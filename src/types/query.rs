use r53::{question::Question, Message};
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;

#[derive(Debug, Clone)]
pub struct Query {
    client: SocketAddr,
    request: Message,
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

pub trait QueryHandler: Send + Clone + 'static {
    fn handle_query(
        self,
        query: &Query,
    ) -> Pin<Box<dyn Future<Output = Option<Message>> + Send + '_>>;
}
