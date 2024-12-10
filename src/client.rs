use crate::{dgramostream, gdl90, internal_com::Receiver, traffic_infos::TrafficInfos};

use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use std::{net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream}, os::fd::AsFd, sync::{atomic::AtomicUsize, atomic::Ordering}, thread, time::Duration};


// Maximum number of clients connected at the same time
const NB_MAX_CLIENTS: usize = 20;

// Current number of connected clients
static NB_CLIENTS: AtomicUsize = AtomicUsize::new(0);


// 2D position
#[derive(Copy, Clone)]
struct Position {
    latitude: f64,
    longitude: f64,
}


pub struct Client {}

impl Client {
    /// Attempting to create a new client
    pub fn create(socket: TcpStream) -> anyhow::Result<()> {
        // We check that the maximum number of connected clients has not already been reached
        if NB_CLIENTS.fetch_add(1, Ordering::Relaxed) >= NB_MAX_CLIENTS {
            NB_CLIENTS.fetch_sub(1, Ordering::Relaxed);
            return Err(anyhow::anyhow!("Maximum number of clients ({}) reached", NB_MAX_CLIENTS));
        }

        // Creating a new client
        thread::spawn(|| {
            Self::work_thread(socket);
        });

        Ok(())
    }


    fn work_thread(socket: TcpStream) {
        // Memorizing the address of the connected client
        let client_addr = socket.peer_addr()
            .unwrap_or(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0));

        // Setting the client socket to be able to detect a connectivity problem as quickly as possible
        Self::set_client_sock_options(&socket);

        // Creation of the receiver of traffic coming from the sources
        let traffic_receiver = Receiver::new();

        // Creation of a datagram receiver on stream to receive client positions
        let mut dgramostream = dgramostream::RecvDgram::new(16);

        // Client's last position
        let mut client_position = None;

        // Events to monitor
        let mut fds = [
            PollFd::new(socket.as_fd(), PollFlags::POLLIN),             // client socket
            PollFd::new(traffic_receiver.as_fd(), PollFlags::POLLIN)    // traffic socket
        ];

        loop {
            // Waiting for an event
            poll(&mut fds, PollTimeout::NONE).unwrap();

            // Processing a client event
            if fds[0].any().unwrap() {
                match Self::process_client_event(&socket, &mut dgramostream) {
                    Err(e) => {
                        log::warn!("Client event error : {}", e);
                        break;
                    },
                    Ok(v) => {
                        match v {
                            None => (),
                            Some(position) => {
                                client_position = Some(position);
                                log::info!("New position received ({}, {}) from client {}",
                                    position.latitude, position.longitude, client_addr);
                            }
                        }
                    }
                }
            }

            // Processing a traffic event
            if fds[1].any().unwrap() {
                if let Err(e) = Self::process_traffic_event(&traffic_receiver, &client_position, &socket) {
                    log::warn!("Traffic event error : {}", e);
                    break;
                }
            }
        }

        // We decrease the number of connected clients
        NB_CLIENTS.fetch_sub(1, Ordering::Relaxed);

        log::info!("Client {} is disconnected", client_addr);
    }


    fn process_client_event(socket: &TcpStream, dgramostream: &mut dgramostream::RecvDgram)
        -> anyhow::Result<Option<Position>> {
        // Reading the position datagram from the client
        match dgramostream.recv(socket)? {
            None => Ok(None),                   // The datagram is not yet reconstituted, nothing to do
            Some(position_dgram) => {    // The datagram is reconstituted, we parse it
                Ok(Some(Self::parse_client_position_msg(position_dgram)?))
            }
        }
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


    fn process_traffic_event(traffic_receiver: &Receiver, client_position: &Option<Position>, socket: &TcpStream)
        -> anyhow::Result<()> {
        // Reading traffic from multicast socket
        let traffic_infos = traffic_receiver.recv()?;

        // Sending traffic information to the client, only if it is nearby
        if Self::traffic_close_to_client(&traffic_infos, client_position) {
            // Preparing the message in GDL90 format
            let mut buffer = [0u8; 100];
            let len = gdl90::make_traffic_report_message(&traffic_infos, &mut buffer).unwrap();

            // Sending the message as a datagram
            dgramostream::send(socket, &buffer[..len])?;
        }

        Ok(())
    }


    fn traffic_close_to_client(traffic_infos: &TrafficInfos, client_position: &Option<Position>) -> bool {
        let mut traffic_close = false;

        match client_position {
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


    fn set_client_sock_options(socket: &TcpStream) {
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


}
