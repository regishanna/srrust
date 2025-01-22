use crate::{client, internal_com};

use nix::sys::epoll::{Epoll, EpollCreateFlags, EpollEvent, EpollFlags};
use std::{net::TcpStream, os::fd::AsFd, sync::{atomic::{AtomicUsize, Ordering}, mpsc, Arc}, thread};


// Maximum number of clients connected at the same time for the pool
const CLIENTS_MAX_NB: usize = 200;


// Event identifier for use in epoll data field

const EVENT_TYPE_CLIENT: u32 = 1;
const EVENT_TYPE_TRAFFIC_RECV: u32 = 2;

struct EventId(u64);

impl EventId {
    fn new(event_type: u32, event_number: u32) -> Self {
        Self(u64::from(event_type) << 32 | u64::from(event_number))
    }

    fn event_type(&self) -> u32 {
        (self.0 >> 32) as u32
    }

    fn event_number(&self) -> u32 {
        (self.0 & 0x00000000_ffffffffu64) as u32
    }
}

impl From<u64> for EventId {
    fn from(event_id: u64) -> EventId {
        Self(event_id)
    }
}

impl From<EventId> for u64 {
    fn from(event_id: EventId) -> u64 {
        event_id.0
    }
}


pub struct ClientPool {
    new_client_tx: mpsc::SyncSender<TcpStream>,
    nb_clients: Arc<AtomicUsize>
}


impl ClientPool {
    /// Creation of the client pool
    pub fn new() -> Self {
        // Creation of the channel to receive new clients
        let (new_client_tx, new_client_rx) = mpsc::sync_channel(0);

        // Initialization of the current number of clients
        let nb_clients = Arc::new(AtomicUsize::new(0));
        let nb_clients_thread = nb_clients.clone();

        // Creation of the thread that will handle the client pool
        thread::spawn(|| {
            Self::work_thread(new_client_rx, nb_clients_thread);
        });

        Self {new_client_tx, nb_clients}
    }


    /// Add a new client to the pool
    pub fn add_new_client(&self, socket: TcpStream) {
        self.new_client_tx.send(socket).unwrap();
    }


    /// Get current number of clients in the pool
    pub fn get_nb_clients(&self) -> usize {
        self.nb_clients.load(Ordering::Relaxed)
    }


    fn work_thread(new_client_rx: mpsc::Receiver<TcpStream>, nb_clients: Arc<AtomicUsize>) {
        // Clients list
        let mut clients = Vec::new();
        let mut free_clients = Vec::new();      // Index of free clients (None) in clients Vec
        let mut clients_to_delete = Vec::new(); // Index of clients to delete in clients Vec

        // Create the epoll instance
        let epoll = Epoll::new(EpollCreateFlags::empty()).unwrap();

        // Create the traffic receiver and register it in epoll
        let traffic_recv = internal_com::Receiver::new(true /* nonblocking */);
        epoll.add(traffic_recv.as_fd(),
        EpollEvent::new(EpollFlags::EPOLLIN,
            EventId::new(EVENT_TYPE_TRAFFIC_RECV, 0).into())).unwrap();

        let mut epoll_events = [EpollEvent::empty(); 100];
        loop {
            // Wait for events
            let nb_events = epoll.wait(&mut epoll_events, 100u16 /* milliseconds */).unwrap();

            // Read the events
            for epoll_event in epoll_events.iter().take(nb_events) {
                let event_id: EventId = epoll_event.data().into();
                match event_id.event_type() {
                    EVENT_TYPE_CLIENT => {
                        // Process the client event
                        let client_index = event_id.event_number() as usize;
                        Self::process_client_event(client_index, &epoll, &mut clients, &mut free_clients, &nb_clients);
                    }

                    EVENT_TYPE_TRAFFIC_RECV => {
                        // Process the traffic receiver event
                        Self::process_traffic_event(&traffic_recv, &epoll, &mut clients, &mut free_clients, &mut clients_to_delete, &nb_clients);
                    }

                    event_type => panic!("Unknown event type : {event_type}"),
                }
            }

            // Check if there are new clients
            Self::check_new_client(&new_client_rx, &epoll, &mut clients, &mut free_clients, &nb_clients);
        }
    }


    fn process_client_event(client_index: usize, epoll: &Epoll, clients: &mut [Option<client::Client>], free_clients: &mut Vec<usize>, nb_clients: &Arc<AtomicUsize>) {
        if let Some(client) = &mut clients[client_index] {
            match client.recv_position() {
                Ok(Some(position)) => {
                    log::info!("New position received ({}, {}) from client {}",
                        position.latitude, position.longitude, client.address());
                }
                Ok(None) => {
                    // Nothing to do
                }
                Err(e) => {
                    // Error while receiving the client position
                    log::warn!("Receive error ({}) from client {}", e, client.address());
                    Self::delete_client(client_index, epoll, clients, free_clients, nb_clients);
                }
            }
        }
    }


    fn process_traffic_event(traffic_recv: &internal_com::Receiver, epoll: &Epoll,
        clients: &mut [Option<client::Client>], free_clients: &mut Vec<usize>, clients_to_delete: &mut Vec<usize>,
        nb_clients: &Arc<AtomicUsize>) {

        // Loop until there is no more traffic information to receive, to optimize the number of epoll.wait calls
        loop {
            match traffic_recv.recv() {
                Err(e) => {
                    // Exit the loop if an error occurs
                    // A WouldBlock error is normal because we are in non-blocking mode
                    // and indicates that there is no more traffic information to receive
                    match e.downcast_ref::<std::io::Error>() {
                        Some(err) if err.kind() == std::io::ErrorKind::WouldBlock => (),
                        Some(_) | None => log::warn!("Traffic receive error : {}", e),
                    }
                    break;
                }
    
                Ok(infos) => {
                    // Send the traffic information to all clients
                    for (i, client_opt) in clients.iter().enumerate() {
                        if let Some(client) = client_opt {
                            if let Err(e) = client.send_traffic(&infos) {
                                log::warn!("Send error ({}) to client {}", e, client.address());
                                // Add the client to the delete list
                                clients_to_delete.push(i);
                            }
                        }
                    }
    
                    // Delete clients that must be deleted
                    while let Some(i) = clients_to_delete.pop() {
                        Self::delete_client(i, epoll, clients, free_clients, nb_clients);
                    }
                }
            }
        }
    }


    fn add_client(client: client::Client, epoll: &Epoll, clients: &mut Vec<Option<client::Client>>, free_clients: &mut Vec<usize>, nb_clients: &Arc<AtomicUsize>) {
        // If the maximum number of clients is reached, we refuse the new client
        let current_nb_clients = nb_clients.fetch_add(1, Ordering::Relaxed);
        if current_nb_clients >= CLIENTS_MAX_NB {
            nb_clients.fetch_sub(1, Ordering::Relaxed);
            log::warn!("Unable to connect new client {} : maximum number of clients ({}) for the pool is reached", client.address(), CLIENTS_MAX_NB);
        }
        else {
            log::info!("New client connected : {}, {}th in the pool", client.address(), current_nb_clients + 1);

            // Add the new client to the list
            let client_index;

            // If there are free clients, we reuse one
            if let Some(i) = free_clients.pop() {
                assert!(clients[i].is_none());
                clients[i] = Some(client);
                client_index = i;
            }
            // Otherwise we add a new client
            else {
                clients.push(Some(client));
                client_index = clients.len() - 1;
            }
    
            // Register the event in epoll
            epoll.add(clients[client_index].as_ref().unwrap().as_fd(),
                EpollEvent::new(EpollFlags::EPOLLIN,
                    EventId::new(EVENT_TYPE_CLIENT, client_index.try_into().unwrap()).into())).unwrap();    
        }
    }


    fn delete_client(client_index: usize, epoll: &Epoll, clients: &mut [Option<client::Client>], free_clients: &mut Vec<usize>, nb_clients: &Arc<AtomicUsize>) {
        let client = &mut clients[client_index];

        log::info!("Client {} is disconnected", client.as_ref().unwrap().address());

        // Unregister the event in epoll
        epoll.delete(client.as_ref().unwrap().as_fd()).unwrap();

        // Free the client
        *client = None;
        free_clients.push(client_index);

        // Decrement the number of clients
        nb_clients.fetch_sub(1, Ordering::Relaxed);
    }


    fn check_new_client(new_client_rx: &mpsc::Receiver<TcpStream>, epoll: &Epoll,
        clients: &mut Vec<Option<client::Client>>, free_clients: &mut Vec<usize>, nb_clients: &Arc<AtomicUsize>) {

        // While there are new clients, we add them to the pool
        while let Ok(socket) = new_client_rx.try_recv() {
            socket.set_nonblocking(true).unwrap();  // We can't block all clients because of one blocking client
            Self::add_client(client::Client::new(socket), epoll, clients, free_clients, nb_clients);
        }
    }


}