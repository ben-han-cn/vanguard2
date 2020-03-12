mod delegation_point;
mod forwarder;
mod host_selector;
mod iter_event;
mod iterator;
mod nsclient;
mod roothint;

pub use iterator::{new_iterator, Iterator};

#[cfg(test)]
mod iterator_test;
