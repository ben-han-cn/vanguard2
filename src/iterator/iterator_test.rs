use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use super::forwarder::ForwarderManager;
use super::host_selector::{Host, HostSelector, RTTBasedHostSelector};
use super::iterator::Iterator;
use super::nsclient::NameServerClient;
use crate::cache::MessageCache;
use crate::config::{VanguardConfig, ZoneForwarderConfig};
use anyhow::{self, bail};
use async_trait::async_trait;
use r53::{message::SectionType, name::root, Message, MessageBuilder, Name, RRType, RRset, Rcode};
use serde::Deserialize;
use std::{
    fs::{read_dir, File},
    io::prelude::*,
    path::{Path, PathBuf},
};
use tokio::runtime::Runtime;

#[derive(Clone, Eq, PartialEq)]
struct ClientRequest {
    target: Host,
    name: Name,
    typ: RRType,
}

impl Hash for ClientRequest {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.target.hash(state);
        self.name.hash(state);
        state.write_u16(self.typ.to_u16());
    }
}

#[derive(Clone)]
pub struct DumbClient {
    host_selector: Arc<Mutex<RTTBasedHostSelector>>,
    responses: HashMap<ClientRequest, Message>,
}

impl DumbClient {
    pub fn new(selector: Arc<Mutex<RTTBasedHostSelector>>) -> Self {
        Self {
            host_selector: selector,
            responses: HashMap::new(),
        }
    }

    pub fn add_response(&mut self, target: Host, name: &Name, typ: RRType, response: Message) {
        self.responses.insert(
            ClientRequest {
                target,
                name: name.clone(),
                typ,
            },
            response,
        );
    }
}

#[async_trait]
impl NameServerClient for DumbClient {
    async fn query(&self, request: &Message, target: Host) -> anyhow::Result<Message> {
        let question = request.question.as_ref().unwrap();
        match self.responses.get(&ClientRequest {
            target,
            name: question.name.clone(),
            typ: question.typ,
        }) {
            Some(response) => {
                self.host_selector
                    .lock()
                    .unwrap()
                    .set_rtt(target, Duration::from_millis(10));
                return Ok(response.clone());
            }

            None => {
                self.host_selector
                    .lock()
                    .unwrap()
                    .set_timeout(target, Duration::from_secs(3));
                bail!("timeout");
            }
        }
    }
}

fn gen_response(
    name: Name,
    typ: RRType,
    answers: Vec<RRset>,
    auths: Vec<RRset>,
    additionals: Vec<RRset>,
) -> Message {
    let mut response = Message::with_query(name, typ);
    let mut builder = MessageBuilder::new(&mut response);
    for answer in answers {
        builder.add_answer(answer);
    }

    for auth in auths {
        builder.add_auth(auth);
    }

    for additional in additionals {
        builder.add_additional(additional);
    }
    builder.make_response();
    builder.done();
    response
}

fn message_body_eq(m1: &Message, m2: &Message) {
    assert_eq!(
        m1.section(SectionType::Answer),
        m2.section(SectionType::Answer)
    );
    assert_eq!(
        m1.section(SectionType::Authority),
        m2.section(SectionType::Authority)
    );
    assert_eq!(
        m1.section(SectionType::Additional),
        m2.section(SectionType::Additional)
    );
}

#[derive(Debug, Deserialize)]
struct TestCase {
    pub qname: String,
    pub qtype: String,
    pub servers: Vec<NameServer>,
    pub response: Response,
}

#[derive(Debug, Deserialize)]
struct NameServer {
    pub ip: String,
    pub zone: String,
    pub qname: String,
    pub qtype: String,
    pub response: Response,
}

#[derive(Debug, Deserialize)]
struct Response {
    pub answer: Option<Vec<String>>,
    pub authority: Option<Vec<String>>,
    pub additional: Option<Vec<String>>,
}

impl Response {
    pub fn to_message(self, qname: &Name, qtype: RRType) -> Message {
        gen_response(
            qname.clone(),
            qtype,
            self.answer
                .unwrap_or(vec![])
                .iter()
                .map(|s| RRset::from_str(s).unwrap())
                .collect::<Vec<RRset>>(),
            self.authority
                .unwrap_or(vec![])
                .iter()
                .map(|s| RRset::from_str(s).unwrap())
                .collect::<Vec<RRset>>(),
            self.additional
                .unwrap_or(vec![])
                .iter()
                .map(|s| RRset::from_str(s).unwrap())
                .collect::<Vec<RRset>>(),
        )
    }
}

fn run_testcase(conf: &VanguardConfig, case: TestCase) {
    let host_selector = Arc::new(Mutex::new(RTTBasedHostSelector::new(10000)));
    let mut client = DumbClient::new(host_selector.clone());
    let mut cache = MessageCache::new(100000);
    //as a replacement for root hint
    cache.add_response(gen_response(
        root(),
        RRType::NS,
        vec![RRset::from_str(". 3600 IN NS a.root.").unwrap()],
        vec![],
        vec![RRset::from_str("a.root. 3600 IN A 1.1.1.1").unwrap()],
    ));

    for server in case.servers {
        let qname = Name::new(server.qname.as_ref()).unwrap();
        let qtype = RRType::from_str(server.qtype.as_ref()).unwrap();
        client.add_response(
            IpAddr::from_str(server.ip.as_ref()).unwrap(),
            &qname,
            qtype,
            server.response.to_message(&qname, qtype),
        );
    }

    let forwarder = Arc::new(ForwarderManager::new(&conf.forwarder));
    let mut iterator = Iterator::new(
        Arc::new(Mutex::new(cache)),
        host_selector,
        forwarder,
        client,
    );
    let mut rt = Runtime::new().unwrap();

    let qname = Name::new(case.qname.as_ref()).unwrap();
    let qtype = RRType::from_str(case.qtype.as_ref()).unwrap();
    let response = rt
        .block_on(iterator.resolve(Message::with_query(qname.clone(), qtype)))
        .unwrap();
    let desired_response = case.response.to_message(&qname, qtype);
    message_body_eq(&response, &desired_response);
}

#[test]
fn test_iterator() {
    let conf: VanguardConfig = Default::default();
    let mut testdir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    testdir.push("src/iterator/testdata");
    for f in vec![
        "cname_chain.yaml",
        "glue_all_in_zone.yaml",
        "some_nameserver_offline.yaml",
        "glue_out_of_zone.yaml",
    ] {
        let mut testfile_path = testdir.clone();
        testfile_path.push(f);
        let mut file = File::open(testfile_path.as_path()).unwrap();
        let mut content = String::new();
        file.read_to_string(&mut content).unwrap();
        let case: TestCase = serde_yaml::from_str(&content).expect("unmarshal failed");
        run_testcase(&conf, case);
    }

    //test forwarder
    let mut conf: VanguardConfig = Default::default();
    conf.forwarder.forwarders.push(ZoneForwarderConfig {
        zone_name: "cn.".to_string(),
        addresses: vec!["44.44.44.44".to_string()],
    });
    let mut testfile_path = testdir.clone();
    testfile_path.push("forward.yaml");
    let mut file = File::open(testfile_path.as_path()).unwrap();
    let mut content = String::new();
    file.read_to_string(&mut content).unwrap();
    let case: TestCase = serde_yaml::from_str(&content).expect("unmarshal failed");
    run_testcase(&conf, case);
}
