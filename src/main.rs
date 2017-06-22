extern crate futures;
extern crate tokio_io;
extern crate mqtt;
extern crate bytes;
extern crate nom;

extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_service;

mod codec;

use std::io;
use mqtt::{Packet, ConnectReturnCode, QoS, SubscribeReturnCode, WritePacketExt};
use tokio_proto::TcpServer;
use tokio_proto::pipeline::ServerProto;
use tokio_io::codec::{Decoder, Encoder, Framed};
use tokio_io::{AsyncRead, AsyncWrite};
use bytes::{BufMut, BytesMut};
use tokio_service::Service;
use futures::{future, Future, BoxFuture};

#[derive(Default)]
pub struct ProtoMqttCodec(codec::MqttCodec);

impl Decoder for ProtoMqttCodec {
    type Item = Option<Packet>;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let res = self.0.decode(src)?;
        Ok(res.map(Some))
    }    
}

impl Encoder for ProtoMqttCodec {
    type Item = Option<Packet>;
    type Error = io::Error;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        if let Some(p) = item {
            dst.writer().write_packet(&p)?;
        }
        Ok(())
    }
}

struct MqttProto;

impl<T: AsyncRead + AsyncWrite + 'static> ServerProto<T> for MqttProto {
    type Request = Option<Packet>;
    type Response = Option<Packet>;
    type Transport = Framed<T, ProtoMqttCodec>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        Ok(io.framed(ProtoMqttCodec(codec::MqttCodec)))
    }
}

struct DummyService;

impl Service for DummyService {
    type Request = Option<Packet>;
    type Response = Option<Packet>;
    type Error = io::Error;
    type Future = BoxFuture<Self::Response, Self::Error>;

    fn call(&self, req: Self::Request) -> Self::Future {
        println!("{:?}", req);
        let res = match req.expect("Can never be None") {
            Packet::Connect { .. } => Some(Packet::ConnectAck { session_present: false, return_code: ConnectReturnCode::ConnectionAccepted }),
            Packet::Publish { qos, packet_id: Some(pid), .. } if qos == QoS::AtLeastOnce => Some(Packet::PublishAck { packet_id: pid }),
            Packet::Subscribe {packet_id, topic_filters} => Some(Packet::SubscribeAck {
                packet_id,
                status: topic_filters.iter().map(|_| SubscribeReturnCode::Success(QoS::AtLeastOnce)).collect()
            }),
            Packet::Unsubscribe {packet_id, ..} => Some(Packet::UnsubscribeAck {packet_id}),
            _ => None
        };
        future::ok(res).boxed()
    }
}

fn main() {
    println!("starting");

    let addr = "0.0.0.0:8113".parse().unwrap();
    let server = TcpServer::new(MqttProto, addr);
    server.serve(|| Ok(DummyService));
}
