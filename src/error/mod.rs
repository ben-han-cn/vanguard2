use failure::Fail;
use serde_yaml;
use std::{io, net::IpAddr};

#[derive(Debug, Fail)]
pub enum VgError {
    #[fail(display = "IO error: {}", _0)]
    IoError(#[fail(cause)] io::Error),

    #[fail(display = "query {} timed out", _0)]
    Timeout(String),

    #[fail(display = "timer error {}", _0)]
    TimerErr(String),

    #[fail(display = "yaml format error: {}", _0)]
    YamlError(#[fail(cause)] serde_yaml::Error),

    #[fail(display = "no name server is found")]
    NoNameserver,

    #[fail(display = "query get loop")]
    LoopedQuery,
}

impl From<io::Error> for VgError {
    fn from(e: io::Error) -> Self {
        VgError::IoError(e)
    }
}

impl From<serde_yaml::Error> for VgError {
    fn from(e: serde_yaml::Error) -> Self {
        VgError::YamlError(e)
    }
}

mod test {
    use super::*;

    #[test]
    fn test_err() {
        let err: failure::Error = VgError::TimerErr("good".to_string()).into();
        assert_eq!(format!("{:?}", err), "TimerErr(\"good\")".to_string());
    }
}
