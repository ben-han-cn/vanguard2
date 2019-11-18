use failure;
use futures::Future;
use r53::Message;
use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct Query {
    pub client: SocketAddr,
    pub message: Message,
    pub done: bool,
}

impl Query {
    pub fn new(message: Message, client: SocketAddr) -> Self {
        Query {
            client,
            message,
            done: false,
        }
    }
}

pub trait QueryHandler: Send + Sync {
    type Response: Future<Item = Query, Error = failure::Error> + Send + 'static;
    fn handle_query(&self, query: Query) -> Self::Response;
}
