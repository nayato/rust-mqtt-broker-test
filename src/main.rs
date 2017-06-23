extern crate futures;
extern crate tokio_io;
extern crate mqtt;
extern crate bytes;
extern crate nom;

extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_service;
extern crate net2;
extern crate num_cpus;

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

use std::net::SocketAddr;
use futures::{Stream, Sink};
use futures::future::Then;
use tokio_core::net::{TcpStream, TcpListener};
use tokio_core::reactor::{Core, Handle};

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

// impl<T: AsyncRead + AsyncWrite + 'static> ServerProto<T> for MqttProto {
//     type Request = Option<Packet>;
//     type Response = Option<Packet>;
//     type Transport = Framed<T, ProtoMqttCodec>;
//     type BindTransport = Result<Self::Transport, io::Error>;

//     fn bind_transport(&self, io: T) -> Self::BindTransport {
//         Ok(io.framed(ProtoMqttCodec(codec::MqttCodec)))
//     }
// }

trait NewMqttBroker {
    fn establish<B: MqttBroker>(proto: MqttProto, connect: Packet) -> BoxFuture<B, io::Error>;
}

trait MqttBroker {
    fn publish(packet: Packet);
}

struct DummyService;

impl Service for DummyService {
    type Request = Option<Packet>;
    type Response = Option<Packet>;
    type Error = io::Error;
    type Future = BoxFuture<Self::Response, Self::Error>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let res = match req.expect("Can never be None") {
            Packet::Connect { .. } => Some(Packet::ConnectAck { session_present: false, return_code: ConnectReturnCode::ConnectionAccepted }),
            Packet::Publish { qos, packet_id: Some(pid), ref payload, .. } if qos == QoS::AtLeastOnce => {
                //print!("[{}, {}]", pid, payload.len());
                Some(Packet::PublishAck { packet_id: pid })
            },
            Packet::Subscribe {packet_id, topic_filters} => Some(Packet::SubscribeAck {
                packet_id,
                status: topic_filters.iter().map(|_| SubscribeReturnCode::Success(QoS::AtLeastOnce)).collect()
            }),
            Packet::Unsubscribe {packet_id, ..} => Some(Packet::UnsubscribeAck {packet_id}),
            Packet::PingRequest => Some(Packet::PingResponse),
            _ => None
        };
        future::ok(res).boxed()
    }
}

impl MqttProto {
    pub fn bind<Io: AsyncRead + AsyncWrite + 'static>(&self, handle: &Handle, socket: Io) {
        let framed = socket.framed(codec::MqttCodec);
        let (tx, rx) = framed.split();
        let rex = rx
            .map_err(move |e| {
                println!("error: {:?}", e);
                e
            })
            .filter_map(move |req| {
                match req {
                    Packet::Connect { .. } => Some(Packet::ConnectAck { session_present: false, return_code: ConnectReturnCode::ConnectionAccepted }),
                    Packet::Publish { qos, packet_id: Some(pid), ref payload, .. } if qos == QoS::AtLeastOnce => {
                        println!("[{}, {}]", pid, payload.len());
                        Some(Packet::PublishAck { packet_id: pid })
                    },
                    Packet::Subscribe {packet_id, topic_filters} => Some(Packet::SubscribeAck {
                        packet_id,
                        status: topic_filters.iter().map(|_| SubscribeReturnCode::Success(QoS::AtLeastOnce)).collect()
                    }),
                    Packet::Unsubscribe {packet_id, ..} => Some(Packet::UnsubscribeAck {packet_id}),
                    Packet::PingRequest => Some(Packet::PingResponse),
                    _ => None
                }
            });
        let server = tx.send_all(rex).then(|_| Ok(()));
        handle.spawn(server);
    }
}

fn main() {
    println!("starting");

    let addr = "0.0.0.0:8113".parse().unwrap();
    serve(addr, /*num_cpus::get()*/ 1, |h| MqttProto);
    // let server = TcpServer::new(MqttProto, addr);
    // server.serve(|| Ok(DummyService));


}

// from server:

fn serve<F>(addr: SocketAddr, workers: usize, new_service: F)
    where F: Fn(&Handle) -> MqttProto
{
    let mut core = Core::new().unwrap();
    let handle = core.handle();
    //let new_service = new_service(&handle);
    let listener = listener(&addr, workers, &handle).unwrap();

    let server = listener.incoming().for_each(move |(socket, _)| {
        // Create the service
        let service = new_service(&handle);

        // Bind it!
        service.bind(&handle, socket);

        Ok(())
    });

    core.run(server).unwrap();
}

fn listener(addr: &SocketAddr,
            workers: usize,
            handle: &Handle) -> io::Result<TcpListener> {
    let listener = match *addr {
        SocketAddr::V4(_) => try!(net2::TcpBuilder::new_v4()),
        SocketAddr::V6(_) => try!(net2::TcpBuilder::new_v6()),
    };
    configure_tcp(workers, &listener)?;
    listener.reuse_address(true)?;
    listener.bind(addr)?;
    listener.listen(1024).and_then(|l| {
        TcpListener::from_listener(l, addr, handle)
    })
}

#[cfg(unix)]
fn configure_tcp(workers: usize, tcp: &net2::TcpBuilder) -> io::Result<()> {
    use net2::unix::*;

    if workers > 1 {
        try!(tcp.reuse_port(true));
    }

    Ok(())
}

#[cfg(windows)]
fn configure_tcp(_workers: usize, _tcp: &net2::TcpBuilder) -> io::Result<()> {
    Ok(())
}
