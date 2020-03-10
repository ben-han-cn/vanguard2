#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate prometheus;

mod auth;
mod cache;
pub mod config;
pub mod controller;
mod forwarder;
mod iterator;
pub mod metrics;
mod nameserver;
pub mod resolver;
pub mod server;
mod types;
