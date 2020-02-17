use std::future::Future;
use std::pin::Pin;

use crate::types::Query;
use r53::Message;

pub trait QueryHandler: Send + Clone + 'static {
    fn handle_query(
        &mut self,
        query: Query,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Message>> + Send + '_>>;
}

pub trait Layer<H: QueryHandler> {
    type Output: QueryHandler;

    fn make_handler(&self, handler: H) -> Self::Output;
}
