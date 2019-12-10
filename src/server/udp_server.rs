use std::net::SocketAddr;
use crate::types::{Query, QueryHandler};
use r53::Message;
use super::udp_stream_coder::UdpStreamCoder;
use tokio_net::udp::{UdpSocket, UdpFramed};
use futures::channel::mpsc::channel;
use futures::{SinkExt, StreamExt};

const QUERY_BUFFER_LEN:usize = 1024;

pub struct UdpServer<H: QueryHandler> {
    handler: H,
}


impl<H:QueryHandler> UdpServer<H> {
    pub fn new(handler: H) -> Self {
        UdpServer {
            handler,
        }
    }

    pub async fn run(&self, addr: SocketAddr) {
        let socket = UdpSocket::bind(addr).await.unwrap();
        let (mut send_stream, mut recv_stream) = UdpFramed::new(socket, UdpStreamCoder::new()).split();
        let (sender, mut receiver) = channel::<(Message, SocketAddr)>(QUERY_BUFFER_LEN);
        tokio::spawn(async move {
            loop {
                let response = receiver.next().await.unwrap();
                send_stream.send(response).await;
            };
        });

        loop {
            if let Some(Ok((request, src))) = recv_stream.next().await {
                let mut sender_back = sender.clone();
                let handler = self.handler.clone();
                tokio::spawn(async move {
                    let query = Query::new(request, src);
                    if let Some(response) = handler.handle_query(&query).await {
                        sender_back.try_send((response, query.client()));
                    }
                });
            }
        }
    }
}
