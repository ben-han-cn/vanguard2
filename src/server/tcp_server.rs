use std::{net::SocketAddr, time::Duration};

use super::tcp_stream_coder::TcpStreamCoder;
use crate::types::{Handler, Request};
use futures::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_util::codec::Framed;

const DEFAULT_RECV_TIMEOUT: Duration = Duration::from_secs(3); //3 secs

pub struct TcpServer<H> {
    handler: H,
}

impl<H: Handler + Send + Sync> TcpServer<H> {
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
                while let Ok(Some(Ok(request))) = timeout(DEFAULT_RECV_TIMEOUT, stream.next()).await
                {
                    let query = Request::new(request, src);
                    if let Ok(response) = handler.clone().resolve(query).await {
                        stream.send(response.response).await;
                    }
                }
            });
        }
    }
}
