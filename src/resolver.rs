use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::{io, thread};

use anyhow;
use crossbeam::channel::bounded;
use mio::net::UdpSocket;
use mio::{Events, Interest, Poll, Token};

use crate::auth::AuthServer;
use crate::config::VanguardConfig;
use crate::msgbuf_pool::{MessageBuf, MessageBufPool};
use crate::types::{Request, Response};

const UDP_SOCKET: Token = Token(0);

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
        let (req_sender, resp_receiver) = bounded::<(MessageBuf, SocketAddr)>(1024);
        let mut poll = Poll::new().unwrap();
        let mut events = Events::with_capacity(1);
        let addr = "0.0.0.0:53".parse().unwrap();
        let mut socket = UdpSocket::bind(addr).unwrap();
        poll.registry()
            .register(&mut socket, UDP_SOCKET, Interest::READABLE)
            .unwrap();
        let msgbuf_pool = Arc::new(Mutex::new(MessageBufPool::new(1024)));
        let cpus = num_cpus::get();
        let worker_thread_count = if cpus > 2 { cpus - 2 } else { 1 };
        let socket = Arc::new(socket);
        for i in (0..worker_thread_count) {
            thread::spawn({
                let msgbuf_pool = msgbuf_pool.clone();
                let socket_sender = socket.clone();
                let resp_receiver = resp_receiver.clone();
                move || loop {
                    if let Ok((buf, addr)) = resp_receiver.recv() {
                        socket_sender.send_to(&buf.data[..buf.len], addr);
                        msgbuf_pool.lock().unwrap().release(buf);
                    }
                }
            });
        }

        loop {
            poll.poll(&mut events, None).unwrap();
            let mut drop = [0; 512];
            for event in events.iter() {
                match event.token() {
                    UDP_SOCKET => loop {
                        let maybe_buf = msgbuf_pool.lock().unwrap().allocate();
                        if let Some(mut buf) = maybe_buf {
                            if let Ok((len, addr)) = socket.recv_from(&mut buf.data) {
                                buf.len = len;
                                req_sender.try_send((buf, addr));
                            } else {
                                msgbuf_pool.lock().unwrap().release(buf);
                                break;
                            }
                        } else {
                            if let Ok((len, addr)) = socket.recv_from(&mut drop) {
                            } else {
                                break;
                            }
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
