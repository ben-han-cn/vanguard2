use bytes::BytesMut;
use r53::Message;
use std::io;
use tokio_util::codec::{Decoder, Encoder};

pub struct UdpStreamCoder {}

impl UdpStreamCoder {
    pub fn new() -> Self {
        UdpStreamCoder {}
    }
}

impl Encoder for UdpStreamCoder {
    type Item = Vec<u8>;
    type Error = io::Error;

    fn encode(&mut self, raw: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.extend(raw);
        Ok(())
    }
}

impl Decoder for UdpStreamCoder {
    type Item = Message;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        match Message::from_wire(src.as_ref()) {
            Ok(message) => Ok(Some(message)),
            Err(e) => Err(io::Error::new(io::ErrorKind::Other, e.to_string())),
        }
    }
}
