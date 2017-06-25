use mqtt::{Packet, read_packet, WritePacketExt, calc_remaining_length};
use tokio_io::codec::{Decoder, Encoder};
use bytes::{BytesMut, BufMut};
use nom::IError;
use std::io::{Error, ErrorKind};

#[derive(Default)]
pub struct MqttCodec;

impl Decoder for MqttCodec {
    type Item = Packet;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let len: usize;
        let p: Packet;
        match read_packet(src) {
            Ok((rest, packet)) => {
                len = (rest.as_ptr() as usize) - (src.as_ptr() as usize);
                p = packet;
            }
            // todo: derive error
            Err(IError::Error(_)) => return Err(Error::new(ErrorKind::Other, "oops")),
            Err(IError::Incomplete(_)) => return Ok(None),
        };
        src.split_to(len);
        Ok(Some(p))
    }
}

impl Encoder for MqttCodec {
    type Item = Packet;
    type Error = Error;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        //println!("dst len: {:?}", dst.len());
        let content_size = calc_remaining_length(&item);
        dst.reserve(content_size + 5);
        dst.writer().write_packet(&item);
        Ok(())
    }
}
