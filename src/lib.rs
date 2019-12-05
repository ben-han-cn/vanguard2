#[macro_use]
extern crate futures;
#[macro_use]
extern crate lazy_static;

mod auth;
mod cache;
pub mod config;
mod error;
mod forwarder;
mod nameserver;
mod recursor;
pub mod resolver;
mod server;
mod types;
