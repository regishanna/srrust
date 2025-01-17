use crate::client_pool::ClientPool;

use std::{net::TcpListener, thread};


pub struct Server {
    client_pools: Vec<ClientPool>
}


impl Server {
    /// Creation of a new server
    pub fn new() -> Self {
        // Create a pool of clients for each CPU
        let nb_cpus = thread::available_parallelism().unwrap().get();
        let mut client_pools = Vec::new();
        for _ in 0..nb_cpus {
            client_pools.push(ClientPool::new());
        }
        log::info!("{} pools of clients created", nb_cpus);

        Self {client_pools}
    }


    /// Listening and processing connections from clients
    /// => This method is blocking
    pub fn listen_connections(&self) {
        let listener = TcpListener::bind("0.0.0.0:1664").unwrap();
        loop {
            // Wait for a new client connection
            let (socket, _) = listener.accept().unwrap();

            // Add the new client to the least populated pool
            self.least_polpulated_pool().add_new_client(socket);
        }
    }


    fn least_polpulated_pool(&self) -> &ClientPool {
        let mut min = self.client_pools[0].get_nb_clients();
        let mut idx = 0;
        for i in 1..self.client_pools.len() {
            let nb_clients = self.client_pools[i].get_nb_clients();
            if nb_clients < min {
                min = nb_clients;
                idx = i;
            }
        }
        &self.client_pools[idx]
    }

}
