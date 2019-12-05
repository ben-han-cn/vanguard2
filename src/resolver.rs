use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use crate::auth::AuthServer;
use crate::config::VanguardConfig;
use crate::server::{QueryCoder};
use crate::types::Query;
use crate::forwarder::ForwarderManager;
use crate::cache::MessageCache;
use crate::recursor::{Recursor};
use r53::Message;

use tokio::prelude::*;
use tokio::net::UdpSocket;
use tokio_util::udp::UdpFramed;
use futures::channel::mpsc::channel;
use futures::{SinkExt, StreamExt};

const QUERY_BUFFER_LEN :usize = 512;
const DEFAULT_MESSAGE_CACHE_SIZE: usize = 10000;

pub struct Resolver {
    auth_server: AuthServer,
    forwarder: ForwarderManager,
    recursor: Recursor,
    cache: Arc<Mutex<MessageCache>>,
    server_addr: String,
}

impl Resolver {
    pub fn new(config: &VanguardConfig) -> Self {
        let auth_server = AuthServer::new(&config.auth);
        let forwarder = ForwarderManager::new(&config.forwarder);
        let cache = Arc::new(Mutex::new(MessageCache::new(DEFAULT_MESSAGE_CACHE_SIZE)));
        let recursor = Recursor::new(&config.recursor, cache.clone());
        let server_addr = config.server.address.clone();
        Resolver {
            auth_server,
            forwarder,
            recursor,
            cache,
            server_addr,
        }
    }

    pub async fn run(self) {
        let socket = UdpSocket::bind(&self.server_addr).await.unwrap();
        let (mut send_stream, mut recv_stream) = UdpFramed::new(socket, QueryCoder::new()).split();
        let (sender, mut receiver) = channel::<(Message, SocketAddr)>(QUERY_BUFFER_LEN);

        tokio::spawn(async move {
            loop {
                let response = receiver.next().await.unwrap();
                send_stream.send(response).await;
            };
        });

        loop {
            if let Some(Ok((request, src))) = recv_stream.next().await {
                let query = Query::new(request, src);
                let mut sender_back = sender.clone();
                let auth_server = self.auth_server.clone();
                let forwarder = self.forwarder.clone();
                let cache = self.cache.clone();
                let recursor = self.recursor.clone();
                tokio::spawn(async move {
                    if let Some(response) = auth_server.handle_query(&query) {
                        sender_back.try_send((response, query.client()));
                        return;
                    }

                    {
                        let mut cache = cache.lock().unwrap();
                        if let Some(response) = cache.gen_response(query.request()) {
                            sender_back.try_send((response, query.client()));
                            return;
                        }
                    }

                    match forwarder.handle_query(&query).await {
                        Ok(Some(response)) => {
                            cache.lock().unwrap().add_response(response.clone());
                            sender_back.try_send((response, query.client()));
                            return;
                        }
                        Ok(None) => {
                        }
                        Err(e) => {
                            println!("forward get err {:?}", e);
                        }
                    }

                    match recursor.handle_query(query.request()).await {
                        Ok(response) => {
                            sender_back.try_send((response, query.client()));
                        }
                        Err(e) => {
                            println!("recursor get err {:?}", e);
                        }
                    }
                });
            }
        }
    }
}

