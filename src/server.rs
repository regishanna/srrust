use crate::client::Client;

use std::net::TcpListener;


/// Ecoute et traitement des connexions provenant des clients
/// => Cette methode est bloquante
pub fn listen_connections() {
    let listener = TcpListener::bind("0.0.0.0:1664").unwrap();
    loop {
        let (socket, addr) = listener.accept().unwrap();
        match Client::new(socket) {
            Err(e) => log::warn!("Impossible de connecter le nouveau client {} : {}", addr, e),
            Ok(()) => log::info!("Nouveau client connecte : {}", addr)
        }
    }
}