use crate::{internal_com::Receiver, traffic_infos};

use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use std::{net::TcpStream, os::fd::AsFd, sync::Mutex, thread, time::Duration, io::Read};


// Nombre maximum de clients connnectes en meme temps
const NB_MAX_CLIENTS: usize = 20;


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
                match Self::process_client_event(&socket) {
                    Err(e) => {
                        log::warn!("Erreur client : {}", e);
                        break;
                    },
                    Ok(()) => ()
                }
            }

            // Traitement d'un evenement de trafic
            if fds[1].any().unwrap() {
                match Self::process_traffic_event(&traffic_receiver) {
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


    fn process_client_event(mut socket: &TcpStream) -> anyhow::Result<()> {
        let mut buffer = [0; 100];
        let nb = socket.read(&mut buffer)?;
        if nb == 0 {
            return Err(anyhow::anyhow!("Connexion fermee par le distant"));
        }
        Ok(())
    }


    fn process_traffic_event(mut traffic_receiver: &Receiver) -> anyhow::Result<()> {
        let traffic_infos = traffic_receiver.recv()?;
        Ok(())
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
