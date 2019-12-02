use std::{io, net::SocketAddr};

use bytes::BytesMut;
use r53::{Message, MessageRender};
use tokio_util::codec::{Decoder, Encoder};

pub struct QueryCoder {
    render: MessageRender,
}

impl QueryCoder {
    pub fn new() -> Self {
        QueryCoder {
            render: MessageRender::new(),
        }
    }
}

impl Encoder for QueryCoder {
    type Item = Message;
    type Error = io::Error;

    fn encode(&mut self, message: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        message.rend(&mut self.render);
        dst.extend(self.render.data());
        self.render.clear();
        Ok(())
    }
}

impl Decoder for QueryCoder {
    type Item = Message;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        match Message::from_wire(src.as_ref()) {
            Ok(message) => Ok(Some(message)),
            Err(e) => Err(io::Error::new(io::ErrorKind::Other, e.to_string())),
        }
    }
}
