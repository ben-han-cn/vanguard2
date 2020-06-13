use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::{io, thread};

use anyhow;
use crossbeam::channel::bounded;
use mio::net::UdpSocket;
use mio::{Events, Interest, Poll, Token};
use r53::{Message, MessageRender};

use crate::auth::AuthServer;
use crate::config::VanguardConfig;
use crate::msgbuf_pool::{MessageBuf, MessageBufPool};
use crate::types::{Request, Response};

const UDP_SOCKET: Token = Token(0);
const DEFAULT_REQUEST_QUEUE_LEN: usize = 4096;

#[derive(Clone)]
pub struct Resolver {
    auth_server: AuthServer,
}

impl Resolver {
    pub fn new(config: &VanguardConfig) -> Self {
        let auth_server = AuthServer::new(&config.auth);
        Resolver { auth_server }
    }

    pub fn run(&self) {
        let (req_sender, req_receiver) =
            bounded::<(MessageBuf, SocketAddr)>(DEFAULT_REQUEST_QUEUE_LEN);
        let (resp_sender, resp_receiver) =
            bounded::<(MessageBuf, SocketAddr)>(DEFAULT_REQUEST_QUEUE_LEN);
        let mut poll = Poll::new().unwrap();
        let mut events = Events::with_capacity(1);
        let addr = "0.0.0.0:53".parse().unwrap();
        let mut socket = UdpSocket::bind(addr).unwrap();
        poll.registry()
            .register(&mut socket, UDP_SOCKET, Interest::READABLE)
            .unwrap();
        let msgbuf_pool = Arc::new(Mutex::new(MessageBufPool::new(DEFAULT_REQUEST_QUEUE_LEN)));
        let socket = Arc::new(socket);
        thread::spawn({
            let socket_sender = socket.clone();
            let msgbuf_pool = msgbuf_pool.clone();
            move || loop {
                if let Ok((buf, addr)) = resp_receiver.recv() {
                    socket_sender.send_to(&buf.data[..buf.len], addr);
                    msgbuf_pool.lock().unwrap().release(buf);
                }
            }
        });

        let cpus = num_cpus::get();
        let worker_thread_count = if cpus > 2 { cpus - 2 } else { 1 };
        for i in (0..worker_thread_count) {
            thread::spawn({
                let req_receiver = req_receiver.clone();
                let resp_sender = resp_sender.clone();
                let auth_server = self.auth_server.clone();
                move || loop {
                    if let Ok((mut buf, addr)) = req_receiver.recv() {
                        if let Ok(msg) = Message::from_wire(&buf.data[0..buf.len]) {
                            let req = Request::new(msg, addr);
                            if let Some(response) = auth_server.resolve(&req) {
                                let mut render = MessageRender::new(&mut buf.data);
                                if let Ok(len) = response.to_wire(&mut render) {
                                    buf.len = len;
                                    resp_sender.try_send((buf, addr));
                                }
                            }
                        }
                    }
                }
            });
        }

        let mut buf = [0; 512];
        loop {
            poll.poll(&mut events, None).unwrap();
            for event in events.iter() {
                match event.token() {
                    UDP_SOCKET => loop {
                        if let Ok((len, addr)) = socket.recv_from(&mut buf) {
                            let msg_buf = msgbuf_pool.lock().unwrap().allocate();
                            if let Some(mut msg_buf) = msg_buf {
                                msg_buf.data[0..len].copy_from_slice(&buf[0..len]);
                                msg_buf.len = len;
                                req_sender.try_send((msg_buf, addr));
                            }
                        } else {
                            break;
                        }
                    },
                    _ => {
                        println!("Got event for unexpected token: {:?}", event);
                    }
                }
            }
        }
    }
}
