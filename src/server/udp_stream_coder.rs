use bytes::BytesMut;
use r53::{Message, MessageRender};
use std::io;
use tokio_util::codec::{Decoder, Encoder};

pub struct UdpStreamCoder {
    render: MessageRender,
}

impl UdpStreamCoder {
    pub fn new() -> Self {
        UdpStreamCoder {
            render: MessageRender::new(),
        }
    }
}

impl Encoder for UdpStreamCoder {
    type Item = Message;
    type Error = io::Error;

    fn encode(&mut self, message: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        message.rend(&mut self.render);
        dst.extend(self.render.data());
        self.render.clear();
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
