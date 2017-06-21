extern crate futures;
extern crate tokio_io;
extern crate mqtt;
extern crate bytes;
extern crate nom;

extern crate tokio_core;
extern crate tokio_proto;

mod codec;

use std::io::{self, ErrorKind, Write};
use mqtt::Packet;
use tokio_proto::TcpServer;
use tokio_proto::multiplex::{RequestId, ServerProto};
use tokio_core::io::Io;
use tokio_io::codec::{Decoder, Encoder, Framed};
use bytes::BytesMut;

#[derive(Default)]
pub struct ProtoMqttCodec(codec::MqttCodec);

impl Decoder for ProtoMqttCodec {
    type Item = (RequestId, Option<Packet>);
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        self.0.decode(src).map(|p| Some(p))
    }    
}

impl Encoder for ProtoMqttCodec {
    type Item = (RequestId, Option<Packet>);
    type Error = io::Error;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        match item {
            Some(p) => {dst.writer().write_packet(&p.1)?; }
            None => {}
        }
        Ok(())
    }
}

struct MqttProto;

// impl<T: Io + 'static> ServerProto<T> for MqttProto {
//     type Request = Option<Packet>;
//     type Response = Option<Packet>;
//     type Transport = Framed<T, ProtoMqttCodec>;
//     type BindTransport = Result<Self::Transport, io::Error>;

//     fn bind_transport(&self, io: T) -> Self::BindTransport {
//         Ok(io.framed(ProtoMqttCodec(codec::MqttCodec)))
//     }
// }

fn main() {
    println!("Hello, world!");

}
