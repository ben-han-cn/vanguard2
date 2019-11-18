use std::{io, net::SocketAddr, sync::Arc, time::Duration};

use super::handler::{Query, QueryHandler};
use futures::{
    future::ok,
    stream::{Fuse, Stream},
    sync::mpsc::{channel, Receiver, Sender},
    Async, Future, Poll,
};
use prometheus::{IntCounter, IntGauge};
use r53::{Message, MessageRender};
use tokio::{executor::spawn, net::UdpSocket};
use tokio_timer::Interval;

const QUERY_BUFFER_LEN: usize = 1024;
const MAX_QUERY_MESSAGE_LEN: usize = 1024;

lazy_static! {
    static ref QPS_UDP_INT_GAUGE: IntGauge =
        register_int_gauge!("pqs", "query per second").unwrap();
    static ref QC_UDP_INT_COUNT: IntCounter =
        register_int_counter!("qc", "query count until now").unwrap();
}

pub struct UdpServer<S: QueryHandler> {
    socket: UdpSocket,
    sender: Sender<Query>,
    handler: Arc<S>,
    response_ch: Fuse<Receiver<Query>>,
}

impl<S: QueryHandler> UdpServer<S> {
    pub fn new(addr: SocketAddr, handler: Arc<S>) -> Self {
        let socket = UdpSocket::bind(&addr).unwrap();
        let (sender, response_ch) = channel(QUERY_BUFFER_LEN);
        UdpServer {
            socket,
            sender,
            handler,
            response_ch: response_ch.fuse(),
        }
    }

    fn send_all_response(&mut self, render: &mut MessageRender) -> Poll<(), io::Error> {
        loop {
            match self.response_ch.poll() {
                Ok(Async::Ready(Some(query))) => {
                    query.message.rend(render);
                    try_ready!(self.socket.poll_send_to(render.data(), &query.client));
                    render.clear();
                }
                Ok(Async::Ready(None)) | Ok(Async::NotReady) => return Ok(Async::Ready(())),
                Err(_) => panic!("get error form channel"),
            }
        }
    }
}

impl<S: QueryHandler> Future for UdpServer<S> {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<(), Self::Error> {
        let mut buf = [0u8; MAX_QUERY_MESSAGE_LEN];
        let mut render = MessageRender::new();
        loop {
            try_ready!(self.send_all_response(&mut render));
            let (size, src) = try_ready!(self.socket.poll_recv_from(&mut buf));
            let query = Message::from_wire(&buf[..size]);
            if query.is_err() {
                continue;
            }
            QC_UDP_INT_COUNT.inc();

            let query = Query::new(query.unwrap(), src);
            let mut sender = UdpStreamSender::new(self.sender.clone());
            spawn(
                self.handler
                    .handle_query(query)
                    .map(move |response| {
                        if let Err(e) = sender.send_response(response) {
                            println!("send response get err {}", e);
                        }
                    })
                    .map_err(|err| {
                        println!("query {:?} is dropped", err);
                    }),
            );
        }
    }
}

#[derive(Clone)]
pub struct UdpStreamSender(Sender<Query>);

impl UdpStreamSender {
    fn new(sender: Sender<Query>) -> Self {
        UdpStreamSender(sender)
    }

    fn send_response(&mut self, response: Query) -> io::Result<()> {
        self.0
            .try_send(response)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "full"))
    }
}

pub fn start_qps_calculate() -> impl Future<Item = (), Error = ()> {
    let interval = Interval::new_interval(Duration::new(1, 0));
    let mut last_qc = 0;
    interval
        .for_each(move |_| {
            let qc = QC_UDP_INT_COUNT.get() as u64;
            QPS_UDP_INT_GAUGE.set((qc - last_qc) as i64);
            last_qc = qc;
            ok(())
        })
        .map_err(|e| println!("timer get err {:?}", e))
}
