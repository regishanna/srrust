use crate::traffic_infos::TrafficInfos;

use std::net::{UdpSocket, SocketAddr, SocketAddrV4, Ipv4Addr};


// Adresse et port multicast a utiliser
const MULTICAST_ADDR_V4: Ipv4Addr = Ipv4Addr::new(224,0,0,64);
const MULTICAST_PORT: u16 = 1665;


/// Reception des infos de trafic provenant des sources
pub struct Receiver {
    socket: UdpSocket,
}

impl Receiver {
    pub fn new() -> Self {
        // Bind de la socket sur l'adresse et le port d'ecoute multicast
        let socket = UdpSocket::bind(SocketAddrV4::new(MULTICAST_ADDR_V4, MULTICAST_PORT)).unwrap();

        // On s'abonne a l'adresse multicast locale
        socket.join_multicast_v4(&MULTICAST_ADDR_V4, &Ipv4Addr::LOCALHOST).unwrap();

        // On s'attend a recevoir des trames locales mais pas de filtrage sur le port distant
        socket.connect(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).unwrap();

        Receiver {socket}
    }

    /// Lecture bloquante d'infos de trafic provenant des sources
    pub fn recv(&self) -> anyhow::Result<TrafficInfos> {
        // Lecture bloquante sur la socket multicast
        let mut buf = [0; 100];
        let recv_size = self.socket.recv(&mut buf)?;

        // Deserialisation pour reconstituer l'info de trafic
        let traffic_infos: TrafficInfos = bincode::deserialize(&buf[..recv_size])?;
        Ok(traffic_infos)
    }

}


/// Emission des infos de trafic a l'ensemble des clients
pub struct Sender {
    socket: UdpSocket,
}

impl Sender {
    pub fn new() -> Self {
        // Bind de la socket sur l'adresse locale en n'imposant pas de port d'emission
        let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();

        // On va emettre les trames vers l'adresse et le port multicast
        socket.connect(SocketAddr::from((MULTICAST_ADDR_V4, MULTICAST_PORT))).unwrap();

        Sender {socket}
    }

    /// Envoi d'infos sur un trafic a l'ensemble des clients
    pub fn send(&self, traffic_infos: &TrafficInfos) {
        // Serialisation de l'info de trafic dans un buffer pour pouvoir l'envoyer
        let buf = bincode::serialize(traffic_infos).unwrap();

        // Envoi du buffer sur la socket multicast
        self.socket.send(&buf).unwrap();
    }

}
