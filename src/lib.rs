#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate prometheus;
#[macro_use]
extern crate slog;
#[macro_use]
extern crate slog_scope;

//mod auth;
pub mod config;
//pub mod controller;
pub mod logger;
pub mod metrics;
mod msgbuf_pool;
pub mod resolver;
mod types;
