use crate::{dgramostream, internal_com::Receiver, traffic_infos::TrafficInfos, gdl90};

use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use std::{net::TcpStream, os::fd::AsFd, sync::Mutex, thread, time::Duration};


// Nombre maximum de clients connnectes en meme temps
const NB_MAX_CLIENTS: usize = 20;


// Position 2D
struct Position {
    latitude: f64,
    longitude: f64,
}


pub struct Client {}

impl Client {
    /// Tentative de creation d'un nouveau client
    pub fn new(socket: TcpStream) -> anyhow::Result<()> {
        static NB_CLIENTS: Mutex<usize> = Mutex::new(0);

        // On verifie que le nombre max de clients connectes n'est pas deja atteint
        let mut nb_clients = NB_CLIENTS.lock().unwrap();
        if *nb_clients >= NB_MAX_CLIENTS {
            return Err(anyhow::anyhow!("Nombre max de clients ({}) atteint", NB_MAX_CLIENTS));
        }
        else {
            // Creation d'un nouveau client
            *nb_clients += 1;
            thread::spawn(|| {
                Self::work_thread(&NB_CLIENTS, socket);
            });
        }

        Ok(())
    }


    fn work_thread(nb_clients: &Mutex<usize>, socket: TcpStream) {
        // Reglage du socket client pour pouvoir detecter au plus vite un probleme de connectivite
        Self::set_client_sock_options(&socket);

        // Creation du recepteur des trafics provenant des sources
        let traffic_receiver = Receiver::new();

        // Creation d'un recepteur de datagram sur stream pour recevoir les positions des clients
        let mut dgramostream = dgramostream::RecvDgram::new(16);

        // Derniere position du client
        let mut client_position = None;

        // Renseignement des evenements a surveiller
        let mut fds = [
            PollFd::new(socket.as_fd(), PollFlags::POLLIN),             // socket client
            PollFd::new(traffic_receiver.as_fd(), PollFlags::POLLIN)    // socket des trafic
        ];

        loop {
            // Attente d'un evenement
            poll(&mut fds, PollTimeout::NONE).unwrap();

            // Traitement d'un evenement client
            if fds[0].any().unwrap() {
                match Self::process_client_event(&socket, &mut dgramostream) {
                    Err(e) => {
                        log::warn!("Erreur client : {}", e);
                        break;
                    },
                    Ok(v) => {
                        match v {
                            None => (),
                            Some(position) => client_position = Some(position)
                        }
                    }
                }
            }

            // Traitement d'un evenement de trafic
            if fds[1].any().unwrap() {
                match Self::process_traffic_event(&traffic_receiver, &client_position, &socket) {
                    Err(e) => {
                        log::warn!("Erreur evenement de trafic : {}", e);
                        break;
                    },
                    Ok(()) => ()
                }
            }
        }

        // On decremente le nombre de clients connectes
        let mut nb_clients = nb_clients.lock().unwrap();
        *nb_clients -= 1;

        log::info!("Le client {} est deconnecte", socket.peer_addr().unwrap());
    }


    fn process_client_event(socket: &TcpStream, dgramostream: &mut dgramostream::RecvDgram)
        -> anyhow::Result<Option<Position>> {
        // lecture du datagram de position provenant du client
        match dgramostream.recv(socket)? {
            None => Ok(None),                   // le datagram n'est pas encore reconstitue, rien a faire
            Some(position_dgram) => {    // le datagram est reconstitue, on le parse
                Ok(Some(Self::parse_client_position_msg(position_dgram)?))
            }
        }
    }


    fn parse_client_position_msg(msg: &[u8]) -> anyhow::Result<Position> {
        let mut parser = bytes_parser::BytesParser::from(msg);

        let latitude = parser.parse_i32()? as f64 / 1000000.0;
        if latitude > 90.0 || latitude < -90.0 {
            return Err(anyhow::anyhow!("Latitude hors bornes"));
        }

        let longitude = parser.parse_i32()? as f64 / 1000000.0;
        if longitude > 180.0 || longitude < -180.0 {
            return Err(anyhow::anyhow!("Longitude hors bornes"));
        }

        Ok(Position {
            latitude,
            longitude,
        })
    }


    fn process_traffic_event(traffic_receiver: &Receiver, client_position: &Option<Position>, socket: &TcpStream)
        -> anyhow::Result<()> {
        // Lecture du trafic depuis la socket multicast
        let traffic_infos = traffic_receiver.recv()?;

        // Envoi de l'info de trafic au client, uniquement s'il est proche
        if Self::traffic_close_to_client(&traffic_infos, client_position) {
            // Preparation du message au format GDL90
            let mut buffer = [0u8; 100];
            let len = gdl90::make_traffic_report_message(&traffic_infos, &mut buffer).unwrap();

            // Envoi du message sous forme de datagram
            dgramostream::send(socket, &buffer[..len])?;
        }

        Ok(())
    }


    fn traffic_close_to_client(traffic_infos: &TrafficInfos, client_position: &Option<Position>) -> bool {
        let mut traffic_close = false;

        match client_position {
            None => (),         // La position du client n'est pas connue, on considere que le trafic n'est pas proche
            Some(position) => {
                // Le trafic doit etre dans un carre centre sur la position du client pour etre considere proche
                const MAX_DELTA_LATITUDE: f64 = 1.0;     // En degres
                const MAX_DELTA_LONGITUDE: f64 = 1.0;    // En degres
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

        // Reglage du timeout TCP
        sock.set_tcp_user_timeout(Some(Duration::from_secs(10))).unwrap();

        // Reglage du keepalive TCP
        let keepalive = socket2::TcpKeepalive::new()
            .with_time(Duration::from_secs(30))
            .with_interval(Duration::from_secs(5))
            .with_retries(2);
        sock.set_tcp_keepalive(&keepalive).unwrap();
    }


}
