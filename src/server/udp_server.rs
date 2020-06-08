use super::udp_stream_coder::UdpStreamCoder;
use crate::types::{Handler, Request};
use futures::{SinkExt, StreamExt};
use prometheus::{IntCounter, IntGauge};
use r53::Message;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::channel;
use tokio::time;
use tokio_util::udp::UdpFramed;

lazy_static! {
    static ref QPS_UDP_INT_GAUGE: IntGauge =
        register_int_gauge!("qps", "query per second").unwrap();
    static ref RPS_UDP_INT_GAUGE: IntGauge =
        register_int_gauge!("rps", "response per second").unwrap();
    static ref CHPS_UDP_INT_GAUGE: IntGauge =
        register_int_gauge!("chps", "cache hit per second").unwrap();
    static ref QC_UDP_INT_COUNT: IntCounter =
        register_int_counter!("qc", "query count until now").unwrap();
    static ref RC_UDP_INT_COUNT: IntCounter =
        register_int_counter!("rc", "response count until now").unwrap();
    static ref CHC_UDP_INT_COUNT: IntCounter =
        register_int_counter!("chc", "cache hit count").unwrap();
}

const RESP_BUFFER_LEN: usize = 1024;
const REQ_BUFFER_LEN: usize = 1024;

pub struct UdpServer<H: Handler> {
    handler: H,
}

impl<H: Handler> UdpServer<H> {
    pub fn new(handler: H) -> Self {
        UdpServer { handler }
    }

    pub async fn run(&mut self, addr: SocketAddr) {
        let socket = UdpSocket::bind(addr).await.unwrap();
        let (mut send_stream, mut recv_stream) =
            UdpFramed::new(socket, UdpStreamCoder::new()).split();
        let (rsp_sender, mut resp_receiver) = channel::<(Message, SocketAddr)>(RESP_BUFFER_LEN);
        tokio::spawn(async move {
            loop {
                let response = resp_receiver.next().await.unwrap();
                send_stream.send(response).await.unwrap();
            }
        });

        let (mut req_sender, mut req_receiver) = channel::<Request>(REQ_BUFFER_LEN);
        tokio::spawn(async move {
            loop {
                if let Some(Ok((request, src))) = recv_stream.next().await {
                    QC_UDP_INT_COUNT.inc();
                    if let Err(e) = req_sender.try_send(Request::new(request, src)) {
                        //error!("send response get error:{}", e);
                    }
                }
            }
        });

        tokio::spawn(calculate_qps());

        loop {
            let query = req_receiver.next().await.unwrap();
            let mut rsp_sender_back = rsp_sender.clone();
            let mut handler = self.handler.clone();
            tokio::spawn(async move {
                let src = query.client;
                if let Ok(response) = handler.resolve(query).await {
                    RC_UDP_INT_COUNT.inc();
                    if response.cache_hit {
                        CHC_UDP_INT_COUNT.inc();
                    }
                    if let Err(e) = rsp_sender_back.try_send((response.response, src)) {
                        //error!("handle request get error:{}", e);
                    }
                }
            });
        }
    }
}

async fn calculate_qps() {
    let mut interval = time::interval(Duration::from_secs(1));
    let mut last_qc = 0;
    let mut last_chc = 0;
    let mut last_rc = 0;
    loop {
        interval.tick().await;

        let qc = QC_UDP_INT_COUNT.get() as u64;
        QPS_UDP_INT_GAUGE.set((qc - last_qc) as i64);
        last_qc = qc;

        let rc = RC_UDP_INT_COUNT.get() as u64;
        RPS_UDP_INT_GAUGE.set((rc - last_rc) as i64);
        last_rc = rc;

        let chc = CHC_UDP_INT_COUNT.get() as u64;
        CHPS_UDP_INT_GAUGE.set((chc - last_chc) as i64);
        last_chc = chc;
    }
}
