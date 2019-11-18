use std::{
    io::{self, Read, Write},
    mem,
    net::SocketAddr,
    time::Duration,
};

use super::handler::{Query, QueryHandler};
use failure;
use futures::{future, stream::Stream, Async, Future, Poll};
use r53::{Message, MessageRender};
use std::sync::Arc;
use tokio::{
    executor::spawn,
    net::{TcpListener, TcpStream},
};
use tokio_io::try_nb;
use tokio_timer::Timeout;

const DEFAULT_RECV_TIMEOUT: Duration = Duration::from_secs(3); //3 secs

pub struct TcpServer<S: QueryHandler> {
    addr: SocketAddr,
    handler: Arc<S>,
}

impl<S: QueryHandler + 'static> TcpServer<S> {
    pub fn new(addr: SocketAddr, handler: Arc<S>) -> Self {
        TcpServer { addr, handler }
    }

    pub fn into_future(self) -> impl Future<Item = (), Error = ()> + Send + 'static {
        let listener = TcpListener::bind(&self.addr).unwrap();
        listener
            .incoming()
            .for_each(move |tcp_stream| {
                let peer = tcp_stream.peer_addr().unwrap();
                let stream = TcpStreamWrapper::from_stream(tcp_stream, peer, self.handler.clone());
                let stream = Timeout::new(stream, DEFAULT_RECV_TIMEOUT);
                spawn(
                    stream
                        .for_each(|_| future::ok(()))
                        .map_err(|e| println!("get error {:?}", e)),
                );

                Ok(())
            })
            .map_err(|e| panic!("error in inbound tcp_stream: {}", e))
    }
}

enum WriteTcpState {
    LenBytes {
        pos: usize,
        length: [u8; 2],
        bytes: Vec<u8>,
    },
    Bytes {
        pos: usize,
        bytes: Vec<u8>,
    },
    Flushing,
}

pub enum ReadTcpState {
    LenBytes { pos: usize, bytes: [u8; 2] },
    Bytes { pos: usize, bytes: Vec<u8> },
}

pub struct TcpStreamWrapper<S: QueryHandler> {
    socket: TcpStream,
    send_state: Option<WriteTcpState>,
    read_state: Option<ReadTcpState>,
    wait_for_handling: Option<S::Response>,
    peer_addr: SocketAddr,
    handler: Arc<S>,
    render: MessageRender,
}

impl<S: QueryHandler> TcpStreamWrapper<S> {
    pub fn from_stream(stream: TcpStream, peer_addr: SocketAddr, handler: Arc<S>) -> Self {
        TcpStreamWrapper {
            socket: stream,
            send_state: None,
            read_state: Some(ReadTcpState::LenBytes {
                pos: 0,
                bytes: [0u8; 2],
            }),
            wait_for_handling: None,
            peer_addr,
            handler,
            render: MessageRender::new(),
        }
    }

    fn try_read(&mut self) -> Poll<Option<()>, io::Error> {
        loop {
            match self.read_state.as_mut().unwrap() {
                ReadTcpState::LenBytes {
                    ref mut pos,
                    ref mut bytes,
                } => {
                    let read = try_nb!(self.socket.read(&mut bytes[*pos..]));
                    if read == 0 {
                        if *pos == 0 {
                            return Ok(Async::Ready(None));
                        } else {
                            return Err(io::Error::new(
                                io::ErrorKind::BrokenPipe,
                                "closed while reading length",
                            ));
                        }
                    }
                    *pos += read;

                    if *pos == bytes.len() {
                        let length =
                            u16::from(bytes[0]) << 8 & 0xFF00 | u16::from(bytes[1]) & 0x00FF;
                        let mut bytes = vec![0; length as usize];
                        bytes.resize(length as usize, 0);
                        self.read_state = Some(ReadTcpState::Bytes { pos: 0, bytes });
                    }
                }
                ReadTcpState::Bytes {
                    ref mut pos,
                    ref mut bytes,
                } => {
                    let read = try_nb!(self.socket.read(&mut bytes[*pos..]));
                    if read == 0 {
                        return Err(io::Error::new(
                            io::ErrorKind::BrokenPipe,
                            "closed while reading message",
                        ));
                    }

                    *pos += read;
                    if *pos == bytes.len() {
                        let query = Message::from_wire(bytes.as_ref());
                        if query.is_ok() {
                            self.read_state = None;
                            let query = Query::new(query.unwrap(), self.peer_addr);
                            self.wait_for_handling = Some(self.handler.handle_query(query));
                            return Ok(Async::Ready(Some(())));
                        } else {
                            return Ok(Async::Ready(None));
                        }
                    }
                }
            };
        }
    }

    fn try_send(&mut self) -> Poll<(), io::Error> {
        loop {
            match self.send_state {
                Some(WriteTcpState::LenBytes {
                    ref mut pos,
                    ref length,
                    ..
                }) => {
                    let wrote = try_nb!(self.socket.write(&length[*pos..]));
                    *pos += wrote;
                }
                Some(WriteTcpState::Bytes {
                    ref mut pos,
                    ref bytes,
                }) => {
                    let wrote = try_nb!(self.socket.write(&bytes[*pos..]));
                    *pos += wrote;
                }
                Some(WriteTcpState::Flushing) => {
                    try_nb!(self.socket.flush());
                }
                None => {
                    self.read_state = Some(ReadTcpState::LenBytes {
                        pos: 0,
                        bytes: [0u8; 2],
                    });
                    return Ok(Async::Ready(()));
                }
            }

            let current_state = mem::replace(&mut self.send_state, None);
            match current_state {
                Some(WriteTcpState::LenBytes { pos, length, bytes }) => {
                    if pos < length.len() {
                        self.send_state = Some(WriteTcpState::LenBytes { pos, length, bytes });
                    } else {
                        self.send_state = Some(WriteTcpState::Bytes { pos: 0, bytes });
                    }
                }
                Some(WriteTcpState::Bytes { pos, bytes }) => {
                    if pos < bytes.len() {
                        self.send_state = Some(WriteTcpState::Bytes { pos, bytes });
                    } else {
                        self.send_state = Some(WriteTcpState::Flushing);
                    }
                }
                Some(WriteTcpState::Flushing) => {
                    self.send_state = None;
                }
                None => {
                    panic!("this shouldn't happend");
                }
            };
        }
    }

    fn handle_query(&mut self) -> Poll<(), io::Error> {
        let mut handler_fut = mem::replace(&mut self.wait_for_handling, None).unwrap();
        match handler_fut.poll() {
            Ok(Async::Ready(query)) => {
                if query.done {
                    query.message.rend(&mut self.render);
                    let buffer = self.render.take_data();
                    let len: [u8; 2] = [
                        (buffer.len() >> 8 & 0xFF) as u8,
                        (buffer.len() & 0xFF) as u8,
                    ];
                    self.send_state = Some(WriteTcpState::LenBytes {
                        pos: 0,
                        length: len,
                        bytes: buffer,
                    });
                }
                return Ok(Async::Ready(()));
            }
            Ok(Async::NotReady) => {
                self.wait_for_handling = Some(handler_fut);
                return Ok(Async::NotReady);
            }
            Err(_) => {
                self.read_state = Some(ReadTcpState::LenBytes {
                    pos: 0,
                    bytes: [0u8; 2],
                });
                return Ok(Async::Ready(()));
            }
        }
    }
}

impl<S: QueryHandler> Stream for TcpStreamWrapper<S> {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            if self.read_state.is_some() {
                match try_ready!(self.try_read()) {
                    None => {
                        return Ok(Async::Ready(None));
                    }
                    _ => (),
                }
            }

            if self.wait_for_handling.is_some() {
                try_ready!(self.handle_query());
            }

            if self.send_state.is_some() {
                try_ready!(self.try_send());
                return Ok(Async::Ready(Some(())));
            }
        }
    }
}
