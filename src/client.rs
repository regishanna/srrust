use crate::{dgramostream, gdl90, traffic_infos::TrafficInfos};

use std::{net::{SocketAddr, TcpStream}, os::fd::{AsFd, BorrowedFd}, time::Duration};


// client 2D position
#[derive(Clone)]
pub struct Position {
    pub latitude: f64,
    pub longitude: f64,
}


pub struct Client {
    socket: TcpStream,
    address: SocketAddr,
    position: Option<Position>,
    recv_dgram: dgramostream::RecvDgram,
}


impl Client {
    /// Creation of a new client
    pub fn new(socket: TcpStream) -> Self {
        // Set the client socket to be able to detect a connectivity problem as quickly as possible
        Self::set_sock_options(&socket);

        // Get the address of the connected client
        let address = socket.peer_addr().unwrap();

        Self {
            socket,
            address,
            position: None,
            recv_dgram: dgramostream::RecvDgram::new(16),
        }
    }


    /// Get the address of the client
    pub fn address(&self) -> SocketAddr {
        self.address
    }


    /// Receive the position of the client
    pub fn recv_position(&mut self) -> anyhow::Result<Option<Position>> {
        // Reading the position datagram from the client
        match self.recv_dgram.recv(&self.socket)? {
            None => Ok(None),                   // The datagram is not yet reconstituted, nothing to do
            Some(position_dgram) => {    // The datagram is reconstituted, we parse it
                self.position = Some(Self::parse_client_position_msg(position_dgram)?);
                Ok(self.position.clone())
            }
        }
    }


    /// Send traffic information to the client, only if it is nearby
    pub fn send_traffic(&self, traffic_infos: &TrafficInfos) -> anyhow::Result<()> {
        if self.traffic_close(traffic_infos) {
            // The traffic is close to the client, we send it the information

            // Prepare the message in GDL90 format
            let mut buffer = [0u8; 100];
            let len = gdl90::make_traffic_report_message(traffic_infos, &mut buffer).unwrap();

            // Send the message as a datagram
            dgramostream::send(&self.socket, &buffer[..len])?;
        }

        Ok(())
    }


    fn set_sock_options(socket: &TcpStream) {
        let sock = socket2::SockRef::from(socket);

        // Setting TCP timeout
        sock.set_tcp_user_timeout(Some(Duration::from_secs(10))).unwrap();

        // Setting TCP keepalive
        let keepalive = socket2::TcpKeepalive::new()
            .with_time(Duration::from_secs(30))
            .with_interval(Duration::from_secs(5))
            .with_retries(2);
        sock.set_tcp_keepalive(&keepalive).unwrap();
    }


    fn parse_client_position_msg(msg: &[u8]) -> anyhow::Result<Position> {
        let mut parser = bytes_parser::BytesParser::from(msg);

        let latitude = f64::from(parser.parse_i32()?) / 1_000_000.0;
        anyhow::ensure!((-90.0..=90.0).contains(&latitude), "Latitude out of bounds");

        let longitude = f64::from(parser.parse_i32()?) / 1_000_000.0;
        anyhow::ensure!((-180.0..=180.0).contains(&longitude), "Longitude out of bounds");

        Ok(Position {
            latitude,
            longitude,
        })
    }


    fn traffic_close(&self, traffic_infos: &TrafficInfos) -> bool {
        let mut traffic_close = false;

        match &self.position {
            None => (),         // The client's position is not known, we consider that the traffic is not close
            Some(position) => {
                // Traffic must be in a square centered on the customer's position to be considered close
                const MAX_DELTA_LATITUDE: f64 = 1.0;     // In degrees
                const MAX_DELTA_LONGITUDE: f64 = 1.0;    // In degrees
                if ((traffic_infos.latitude - position.latitude).abs() < MAX_DELTA_LATITUDE) &&
                   ((traffic_infos.longitude - position.longitude).abs() < MAX_DELTA_LONGITUDE) {
                    traffic_close = true;
                   }
            }
        }

        traffic_close
    }


}


impl AsFd for Client {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.socket.as_fd()
    }
}
