use std::{net::SocketAddr, time::Duration};

use super::tcp_stream_coder::TcpStreamCoder;
use crate::types::{Query, QueryHandler};
use futures::{SinkExt, StreamExt};
use tokio_codec::Framed;
use tokio_net::tcp::TcpListener;
use tokio_timer::Timeout;

const DEFAULT_RECV_TIMEOUT: Duration = Duration::from_secs(3); //3 secs

pub struct TcpServer<H: QueryHandler> {
    handler: H,
}

impl<H: QueryHandler + Send + Sync> TcpServer<H> {
    pub fn new(handler: H) -> Self {
        TcpServer { handler }
    }

    pub async fn run(self, addr: SocketAddr) {
        let mut listener = TcpListener::bind(&addr).await.unwrap();
        loop {
            let (stream, src) = listener.accept().await.unwrap();
            let handler = self.handler.clone();
            let mut stream = Framed::new(stream, TcpStreamCoder::new());
            tokio::spawn(async move {
                while let Ok(Some(Ok(request))) =
                    Timeout::new(stream.next(), DEFAULT_RECV_TIMEOUT).await
                {
                    let query = Query::new(request, src);
                    if let Some(response) = handler.clone().handle_query(&query).await {
                        stream.send(response).await;
                    }
                }
            });
        }
    }
}
