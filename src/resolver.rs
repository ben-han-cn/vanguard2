use failure;
use futures::{prelude::*, Future};
use std::mem;

use vanguard2::{
    auth::{AuthFuture, AuthServer},
    config::VanguardConfig,
    recursor::{Recursor, RecursorFuture},
    server::{Query, QueryHandler},
};

#[derive(Clone)]
pub struct Resolver {
    auth: AuthServer,
    recursor: Recursor,
}

impl Resolver {
    pub fn new(auth: AuthServer, conf: &VanguardConfig) -> Self {
        Resolver {
            auth: auth,
            recursor: Recursor::new(&conf.recursor, &conf.forwarder),
        }
    }
}

impl QueryHandler for Resolver {
    type Response = ResolverFuture;
    fn handle_query(&self, query: Query) -> Self::Response {
        ResolverFuture::new(self.clone(), query)
    }
}

enum State {
    Auth(AuthFuture),
    Recursor(RecursorFuture),
    Poisoned,
}

pub struct ResolverFuture {
    resolver: Resolver,
    state: State,
}

impl ResolverFuture {
    pub fn new(resolver: Resolver, query: Query) -> Self {
        let state = State::Auth(resolver.auth.handle_query(query));
        ResolverFuture {
            resolver: resolver,
            state,
        }
    }
}

impl Future for ResolverFuture {
    type Item = Query;
    type Error = failure::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            match mem::replace(&mut self.state, State::Poisoned) {
                State::Auth(mut fut) => match fut.poll() {
                    Err(_) | Ok(Async::NotReady) => {
                        unreachable!();
                    }
                    Ok(Async::Ready(query)) => {
                        if query.done {
                            return Ok(Async::Ready(query));
                        } else {
                            self.state =
                                State::Recursor(self.resolver.recursor.handle_query(query));
                        }
                    }
                },
                State::Recursor(mut fut) => match fut.poll() {
                    Err(e) => {
                        return Err(e);
                    }
                    Ok(Async::NotReady) => {
                        self.state = State::Recursor(fut);
                        return Ok(Async::NotReady);
                    }
                    Ok(Async::Ready(query)) => {
                        return Ok(Async::Ready(query));
                    }
                },
                State::Poisoned => {
                    panic!("resolver future crashed");
                }
            }
        }
    }
}
