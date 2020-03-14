use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;

use r53::{question::Question, Message};

#[derive(Debug, Clone)]
pub struct Request {
    pub client: SocketAddr,
    pub request: Message,
}

pub struct Response {
    pub cache_hit: bool,
    pub response: Message,
}

impl Request {
    pub fn new(request: Message, client: SocketAddr) -> Self {
        Self {
            client,
            request: request,
        }
    }

    pub fn question(&self) -> &Question {
        self.request.question.as_ref().unwrap()
    }
}

impl Response {
    pub fn new(response: Message) -> Self {
        Self {
            cache_hit: false,
            response: response,
        }
    }
}

pub trait Handler: Send + Clone + 'static {
    fn resolve(
        &mut self,
        req: Request,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Response>> + Send + '_>>;
}
