use std::mem;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::{io, thread};

use anyhow;
use crossbeam::channel::{bounded, Receiver, Sender, TrySendError};
use mio::net::UdpSocket;
use mio::{Events, Interest, Poll, Token};
use r53::{Message, MessageRender};

use crate::auth::AuthServer;
use crate::config::VanguardConfig;
use crate::msgbuf_pool::{MessageBuf, MessageBufPool};
use crate::types::{Request, Response};

const UDP_SOCKET: Token = Token(0);
const DEFAULT_REQUEST_QUEUE_LEN: usize = 2048;

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
        let mut poll = Poll::new().unwrap();
        let mut events = Events::with_capacity(1);
        let addr = "0.0.0.0:53".parse().unwrap();
        let mut socket = UdpSocket::bind(addr).unwrap();
        poll.registry()
            .register(&mut socket, UDP_SOCKET, Interest::READABLE)
            .unwrap();

        let cpus = num_cpus::get();
        let worker_thread_count = if cpus > 2 { cpus - 2 } else { 1 };
        let pools = (0..worker_thread_count).fold(Vec::new(), |mut pools, i| {
            let pool = Arc::new(Mutex::new(MessageBufPool::new(
                i as u8,
                DEFAULT_REQUEST_QUEUE_LEN,
            )));
            pools.push(pool);
            pools
        });
        println!("create {} worker thread", worker_thread_count);
        let (resp_sender, resp_receiver) =
            bounded::<(MessageBuf, SocketAddr)>(worker_thread_count * DEFAULT_REQUEST_QUEUE_LEN);

        let socket = Arc::new(socket);
        thread::spawn({
            let socket_sender = socket.clone();
            let pools = pools.clone();
            move || loop {
                if let Ok((buf, addr)) = resp_receiver.recv() {
                    socket_sender.send_to(&buf.data[..buf.len], addr);
                    pools[buf.pool_id as usize].lock().unwrap().release(buf);
                }
            }
        });

        let mut senders = Vec::with_capacity(worker_thread_count);
        for i in (0..worker_thread_count) {
            let (req_sender, req_receiver) =
                bounded::<(MessageBuf, SocketAddr)>(DEFAULT_REQUEST_QUEUE_LEN);
            senders.push(req_sender);
            let pool = pools[i].clone();
            thread::spawn({
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
                                    if let Err(TrySendError::Full((buf, _))) =
                                        resp_sender.try_send((buf, addr))
                                    {
                                        pool.lock().unwrap().release(buf);
                                    }
                                }
                            }
                        } else {
                            pool.lock().unwrap().release(buf);
                        }
                    }
                }
            });
        }

        let mut buf = [0; 512];
        let mut handler_index = 0;
        loop {
            poll.poll(&mut events, None).unwrap();
            for event in events.iter() {
                match event.token() {
                    UDP_SOCKET => loop {
                        match socket.recv_from(&mut buf) {
                            Ok((len, addr)) => {
                                let mut retry_count = 0;
                                let mut req_handled = false;
                                loop {
                                    if let Some(mut msg_buf) =
                                        pools[handler_index].lock().unwrap().allocate()
                                    {
                                        msg_buf.data[0..len].copy_from_slice(&buf[0..len]);
                                        msg_buf.len = len;
                                        if let Err(TrySendError::Full((buf, _))) =
                                            senders[handler_index].try_send((msg_buf, addr))
                                        {
                                            pools[handler_index].lock().unwrap().release(buf);
                                        } else {
                                            req_handled = true;
                                        }
                                    }
                                    handler_index = (handler_index + 1) % worker_thread_count;
                                    if req_handled {
                                        break;
                                    }
                                    retry_count += 1;
                                    if retry_count == worker_thread_count {
                                        break;
                                    }
                                }
                            }
                            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                                break;
                            }
                            Err(e) => {
                                panic!("get unexpected error");
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
