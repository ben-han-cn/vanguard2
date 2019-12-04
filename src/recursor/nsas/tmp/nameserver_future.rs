use super::{
    nameserver_cache::Nameserver, ns_address_store::NSAddressStore, zone_fetcher::ZoneFetcher,
};
use crate::{error::VgError, recursor::util::Sender, recursor::Recursor};
use failure;
use futures::{future, prelude::*, Future};
use r53::{message::SectionType, name, Message, MessageBuilder, Name, RData, RRType, Rcode};
use std::{mem, time::Duration};
use tokio::executor::spawn;

const MAX_QUERY_DEPTH: usize = 10;

enum State {
    HitCache(
        Nameserver,
        Option<Box<dyn Future<Item = (), Error = ()> + Send>>,
    ),
    FetchNameserver(ZoneFetcher<Recursor>),
    Poisoned,
}

pub struct NameserverFuture {
    state: State,
}

impl NameserverFuture {
    pub fn new(
        zone: Name,
        resolver: &Recursor,
        address_store: &NSAddressStore,
        depth: usize,
    ) -> failure::Result<Self> {
        if depth > MAX_QUERY_DEPTH {
            return Err(VgError::LoopedQuery.into());
        }

        let (nameserver, probefut) = address_store.get_nameserver(&zone, resolver);
        let state = if let Some(nameserver) = nameserver {
            State::HitCache(nameserver, probefut)
        } else {
            State::FetchNameserver(address_store.fetch_zone(zone, depth, resolver.clone()))
        };
        Ok(NameserverFuture { state })
    }
}

impl Future for NameserverFuture {
    type Item = Nameserver;
    type Error = failure::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match mem::replace(&mut self.state, State::Poisoned) {
            State::HitCache(nameserver, probefut) => {
                if let Some(probefut) = probefut {
                    spawn(probefut);
                }
                return Ok(Async::Ready(nameserver));
            }
            State::FetchNameserver(mut fetcher) => match fetcher.poll() {
                Err(e) => {
                    return Err(e);
                }
                Ok(Async::NotReady) => {
                    self.state = State::FetchNameserver(fetcher);
                    return Ok(Async::NotReady);
                }
                Ok(Async::Ready(nameserver)) => {
                    return Ok(Async::Ready(nameserver));
                }
            },
            State::Poisoned => {
                panic!("nsas future state is corrupted");
            }
        }
    }
}
