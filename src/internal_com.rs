use crate::traffic_infos::TrafficInfos;

use socket2::{Socket, Domain, Type};
use std::{net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket}, os::fd::AsFd};


// Multicast address and port to use
const MULTICAST_ADDR_V4: Ipv4Addr = Ipv4Addr::new(224,0,0,64);
const MULTICAST_PORT: u16 = 1665;


/// Receiving traffic information from sources
pub struct Receiver {
    socket: UdpSocket,
}

impl Receiver {
    pub fn new(nonblocking: bool) -> Self {
        // We use the socket2 crate because UdpSocket does not allow setting the SO_REUSEPORT option
        // necessary to have several receivers listening on the same multicast port
        let sock = Socket::new(Domain::IPV4, Type::DGRAM, None).unwrap();
        sock.set_reuse_port(true).unwrap();

        // Bind the socket to the listening multicast address and port
        sock.bind(&SocketAddrV4::new(MULTICAST_ADDR_V4, MULTICAST_PORT).into()).unwrap();

        // Now we can convert to UdpSocket
        let socket: UdpSocket = sock.into();

        // We set the socket to non-blocking mode if asked
        socket.set_nonblocking(nonblocking).unwrap();

        // We subscribe to the local multicast address
        socket.join_multicast_v4(&MULTICAST_ADDR_V4, &Ipv4Addr::LOCALHOST).unwrap();

        // We expect to receive local frames but no filtering on the remote port
        socket.connect(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).unwrap();

        Self {socket}
    }

    /// Reading of traffic information from sources
    pub fn recv(&self) -> anyhow::Result<TrafficInfos> {
        // Reading on multicast socket
        let mut buf = [0; 100];
        let recv_size = self.socket.recv(&mut buf)?;

        // Deserialization to reconstruct traffic information
        let traffic_infos: TrafficInfos = bincode::deserialize(&buf[..recv_size])?;
        Ok(traffic_infos)
    }

}

impl AsFd for Receiver {
    fn as_fd(&self) -> std::os::unix::prelude::BorrowedFd<'_> {
        self.socket.as_fd()
    }
}


/// Transmission of traffic information to all clients
pub struct Sender {
    socket: UdpSocket,
}

impl Sender {
    pub fn new() -> Self {
        // Bind the socket to the local address without imposing a transmission port
        let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();

        // We will send the frames to the address and the multicast port
        socket.connect(SocketAddr::from((MULTICAST_ADDR_V4, MULTICAST_PORT))).unwrap();

        Sender {socket}
    }

    /// Sending information on traffic to all clients
    pub fn send(&self, traffic_infos: &TrafficInfos) {
        // Serialization of traffic information in a buffer to be able to send it
        let buf = bincode::serialize(traffic_infos).unwrap();

        // Sending the buffer on the multicast socket
        self.socket.send(&buf).unwrap();
    }

}
