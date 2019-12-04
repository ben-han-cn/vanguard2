use failure;
use futures::Future;
use r53::Message;
use std::pin::Pin;

pub trait Resolver: Clone + Send {
    fn handle_query(
        &self,
        request: Message,
    ) -> Pin<Box<dyn Future<Output = Result<Message, failure::Error>> + Send>>;
}
