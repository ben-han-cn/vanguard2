#[macro_use]
extern crate futures;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate prometheus;

pub mod auth;
pub mod config;
mod error;
pub mod recursor;
pub mod server;
