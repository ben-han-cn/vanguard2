mod nameserver_store;
mod sender;

pub use self::nameserver_store::{Nameserver, NameserverStore};
pub use self::sender::send_query;
