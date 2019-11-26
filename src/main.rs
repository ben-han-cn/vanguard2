use clap::{App, Arg};
use std::net::SocketAddr;
use tokio::prelude::*;

use vanguard2::auth::AuthServer;
use vanguard2::config::VanguardConfig;
use vanguard2::server::{Query, QueryCoder};
use vanguard2::error::VgError;
use r53::{MessageRender, Message};

use tokio::net::{UdpFramed, UdpSocket};
use tokio::prelude::*;
use tokio::future::FutureExt as TokioFutureExt;
use failure::Error;
use futures::channel::mpsc::{channel, Receiver, Sender};
use futures::stream::{self, Stream, TryStreamExt};
use futures::{FutureExt, SinkExt, StreamExt};

const MAX_QUERY_MESSAGE_LEN :usize = 512;
const QUERY_BUFFER_LEN :usize = 512;

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
            let query = Query::new(message, src);
            let mut sender_back = sender.clone();
            let auth_server = auth_server.clone();
            tokio::spawn(async move {
                let resp = auth_server.handle_query(query);
                if resp.done {
                    sender_back.try_send(resp);
                }
            });
        }
    }
}
