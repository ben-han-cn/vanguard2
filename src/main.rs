use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use vanguard2::auth::AuthServer;
use vanguard2::config::VanguardConfig;
use vanguard2::server::{QueryCoder};
use vanguard2::types::{Query, classify_response};
use vanguard2::error::VgError;
use vanguard2::forwarder::ForwarderManager;
use vanguard2::cache::MessageCache;
use r53::{MessageRender, Message};

use clap::{App, Arg};
use tokio::prelude::*;
use tokio::net::UdpSocket;
use tokio_util::udp::UdpFramed;
use tokio::prelude::*;
use failure::Error;
use futures::channel::mpsc::{channel, Receiver, Sender};
use futures::stream::{self, Stream, TryStreamExt};
use futures::{FutureExt, SinkExt, StreamExt};

const QUERY_BUFFER_LEN :usize = 512;
const DEFAULT_MESSAGE_CACHE_SIZE: usize = 10000;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let matches = App::new("auth")
        .arg(
            Arg::with_name("config")
                .help("config file path")
                .long("config")
                .required(false)
                .takes_value(true),
        )
        .get_matches();

    let config_file = matches.value_of("config").unwrap_or("vanguard.conf");
    let config = VanguardConfig::load_config(config_file)?; 
    let auth_server = AuthServer::new(&config.auth);
    let forwarder = ForwarderManager::new(&config.forwarder);
    let cache = Arc::new(Mutex::new(MessageCache::new(DEFAULT_MESSAGE_CACHE_SIZE)));
    let addr = config.server.address;
    let socket = UdpSocket::bind(&addr).await?;
    let (mut send_stream, mut recv_stream) = UdpFramed::new(socket, QueryCoder::new()).split();
    let (sender, mut receiver) = channel::<Query>(QUERY_BUFFER_LEN);
    tokio::spawn(async move {
        loop {
            let resp = receiver.next().await.unwrap();
            send_stream.send((resp.message, resp.client)).await;
        };
    });

    loop {
        if let Some(Ok((message, src))) = recv_stream.next().await {
            let mut query = Query::new(message, src);
            let mut sender_back = sender.clone();
            let auth_server = auth_server.clone();
            let forwarder = forwarder.clone();
            let cache = cache.clone();
            tokio::spawn(async move {
                {
                    let mut cache = cache.lock().unwrap();
                    if cache.gen_response(&mut query.message) {
                        sender_back.try_send(query);
                        return;
                    }
                }

                let resp = auth_server.handle_query(query);
                if resp.done {
                    sender_back.try_send(resp);
                    return;
                }

                match forwarder.handle_query(resp).await {
                    Ok(resp) =>  {
                        let question = resp.message.question.as_ref().unwrap();
                        let response_type = classify_response(&question.name, question.typ, &resp.message);
                        let mut cache = cache.lock().unwrap();
                        cache.add_response(response_type, resp.message.clone());
                        sender_back.try_send(resp);
                        return;
                    }
                    Err(e) => {
                        println!("forward get err {:?}", e);
                        return;
                    }
                }
            });
        }
    }
}
