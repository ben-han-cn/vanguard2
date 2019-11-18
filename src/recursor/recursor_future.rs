use super::{recursor::Recursor, running_query::RunningQuery};
use crate::error::VgError;
use crate::server::Query;
use failure;
use futures::{prelude::*, Future};
use r53::Message;
use std::{
    error::Error,
    net::SocketAddr,
    time::{Duration, Instant},
};
use tokio::timer::Delay;

const DEFAULT_RECURSOR_TIMEOUT: Duration = Duration::from_secs(10);

pub struct RecursorFuture {
    client: SocketAddr,
    inner: RunningQuery,
    delay: Delay,
}

impl RecursorFuture {
    pub fn new(recursor: Recursor, query: Query) -> Self {
        RecursorFuture {
            client: query.client,
            inner: RunningQuery::new(query.message, recursor, 0),
            delay: Delay::new(
                Instant::now()
                    .checked_add(DEFAULT_RECURSOR_TIMEOUT)
                    .unwrap(),
            ),
        }
    }
}

impl Future for RecursorFuture {
    type Item = Query;
    type Error = failure::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            match self.inner.poll() {
                Err(e) => {
                    self.inner.reset();
                }
                Ok(Async::NotReady) => match self.delay.poll() {
                    Err(e) => {
                        return Err(VgError::TimerErr(e.description().to_string()).into());
                    }
                    Ok(Async::Ready(_)) => {
                        return Err(VgError::Timeout("".to_string()).into());
                    }
                    Ok(Async::NotReady) => {
                        return Ok(Async::NotReady);
                    }
                },
                Ok(Async::Ready(resp)) => {
                    return Ok(Async::Ready(Query {
                        client: self.client,
                        message: resp,
                        done: true,
                    }));
                }
            }
        }
    }
}
