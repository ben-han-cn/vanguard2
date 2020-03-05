use r53::{question::Question, Message};
use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct Query {
    client: SocketAddr,
    pub request: Message,
}

impl Query {
    pub fn new(request: Message, client: SocketAddr) -> Self {
        Query {
            client,
            request: request,
        }
    }

    pub fn client(&self) -> SocketAddr {
        self.client
    }

    pub fn request(&self) -> &Message {
        &self.request
    }

    pub fn question(&self) -> &Question {
        self.request().question.as_ref().unwrap()
    }
}
