use super::nameserver_store::{Nameserver, NameserverStore};
use crate::error::VgError;
use failure;
use futures::{prelude::*, Future};
use r53::{Message, MessageRender};
use std::{
    error::Error,
    mem,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    net::{udp, UdpSocket},
    timer::Delay,
    util::FutureExt,
};

const DEFAULT_RECV_TIMEOUT: Duration = Duration::from_secs(2); //3 secs
const DEFAULT_RECV_BUF_SIZE: usize = 1024;

enum State {
    Init,
    Send(udp::SendDgram<Vec<u8>>),
    Recv(udp::RecvDgram<Vec<u8>>, Delay, Instant),
    Poisoned,
}

pub struct Sender<S, SS> {
    query: Message,
    nameserver: S,
    nsas: SS,
    state: State,
}

impl<S: Nameserver, SS: NameserverStore<S>> Sender<S, SS> {
    pub fn new(query: Message, nameserver: S, nsas: SS) -> Self {
        Sender {
            query,
            nameserver,
            nsas,
            state: State::Init,
        }
    }
}

impl<S: Nameserver, SS: NameserverStore<S>> Future for Sender<S, SS> {
    type Item = Message;
    type Error = failure::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            match mem::replace(&mut self.state, State::Poisoned) {
                State::Init => {
                    let mut render = MessageRender::new();
                    self.query.rend(&mut render);
                    let socket =
                        UdpSocket::bind(&("0.0.0.0:0".parse::<SocketAddr>().unwrap())).unwrap();
                    let target = self.nameserver.get_addr();
                    self.state = State::Send(socket.send_dgram(render.take_data(), &target));
                }
                State::Send(mut fut) => match fut.poll() {
                    Err(e) => {
                        self.nameserver.set_unreachable();
                        self.nsas.update_nameserver_rtt(&self.nameserver);
                        return Err(VgError::IoError(e).into());
                    }
                    Ok(Async::NotReady) => {
                        self.state = State::Send(fut);
                        return Ok(Async::NotReady);
                    }
                    Ok(Async::Ready((socket, _))) => {
                        let timeout = {
                            let mut rtt = self.nameserver.get_rtt();
                            if rtt.as_millis() == 0 || rtt > DEFAULT_RECV_TIMEOUT {
                                rtt = DEFAULT_RECV_TIMEOUT;
                            }
                            rtt
                        };
                        self.state = State::Recv(
                            socket.recv_dgram(vec![0; DEFAULT_RECV_BUF_SIZE]),
                            Delay::new(Instant::now().checked_add(timeout).unwrap()),
                            Instant::now(),
                        );
                    }
                },
                State::Recv(mut fut, mut delay, send_time) => match fut.poll() {
                    Err(e) => {
                        self.nameserver.set_unreachable();
                        self.nsas.update_nameserver_rtt(&self.nameserver);
                        return Err(VgError::IoError(e).into());
                    }
                    Ok(Async::NotReady) => match delay.poll() {
                        Err(e) => {
                            return Err(VgError::TimerErr(e.description().to_string()).into());
                        }
                        Ok(Async::Ready(_)) => {
                            self.nameserver.set_rtt(DEFAULT_RECV_TIMEOUT);
                            self.nsas.update_nameserver_rtt(&self.nameserver);
                            return Err(VgError::Timeout(
                                self.nameserver.get_addr().ip().to_string(),
                            )
                            .into());
                        }
                        Ok(Async::NotReady) => {
                            self.state = State::Recv(fut, delay, send_time);
                            return Ok(Async::NotReady);
                        }
                    },
                    Ok(Async::Ready((_, buf, size, _))) => {
                        self.nameserver.set_rtt(send_time.elapsed());
                        self.nsas.update_nameserver_rtt(&self.nameserver);
                        match Message::from_wire(&buf[..size]) {
                            Ok(resp) => {
                                return Ok(Async::Ready(resp));
                            }
                            Err(err) => {
                                return Err(err);
                            }
                        }
                    }
                },
                State::Poisoned => panic!("inside sender pool"),
            }
        }
    }
}
