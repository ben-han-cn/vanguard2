use bytes::{Buf, BufMut, BytesMut};
use r53::{Message, MessageRender};
use std::io::{self, Cursor};
use tokio_util::codec::{Decoder, Encoder};

pub struct TcpStreamCoder {
    render: MessageRender,
    message_len: Option<u16>,
}

impl TcpStreamCoder {
    pub fn new() -> Self {
        TcpStreamCoder {
            render: MessageRender::new(),
            message_len: None,
        }
    }
}

impl Encoder for TcpStreamCoder {
    type Item = Message;
    type Error = io::Error;

    fn encode(&mut self, message: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        message.to_wire(&mut self.render);
        let buffer = self.render.take_data();
        dst.put_u16(buffer.len() as u16);
        dst.extend(buffer);
        self.render.clear();
        Ok(())
    }
}

impl Decoder for TcpStreamCoder {
    type Item = Message;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if self.message_len.is_none() {
            if src.len() < 2 {
                return Ok(None);
            }
            self.message_len = Some(Cursor::new(&mut *src).get_u16());
            let _ = src.split_to(2);
        }

        let message_len = self.message_len.unwrap();
        if src.len() < (message_len as usize) {
            return Ok(None);
        }
        self.message_len = None;
        match Message::from_wire(&src.as_ref()[0..(message_len as usize)]) {
            Ok(message) => Ok(Some(message)),
            Err(e) => Err(io::Error::new(io::ErrorKind::Other, e.to_string())),
        }
    }
}
