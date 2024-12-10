use crate::client::Client;

use std::net::TcpListener;


/// Listening and processing connections from clients
/// => This method is blocking
pub fn listen_connections() {
    let listener = TcpListener::bind("0.0.0.0:1664").unwrap();
    loop {
        let (socket, addr) = listener.accept().unwrap();
        match Client::create(socket) {
            Err(e) => log::warn!("Unable to connect new client {} : {}", addr, e),
            Ok(()) => log::info!("New client connected : {}", addr)
        }
    }
}