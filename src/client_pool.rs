use crate::{client, internal_com};

use nix::sys::epoll::{Epoll, EpollCreateFlags, EpollEvent, EpollFlags};
use std::{net::TcpStream, os::fd::{AsFd, BorrowedFd}, sync::{atomic::{AtomicUsize, Ordering}, mpsc, Arc}, thread};


// Maximum number of clients connected at the same time for the pool
const CLIENTS_MAX_NB: usize = 200;

enum EventType {
    Client(client::Client),
    TrafficRecv(internal_com::Receiver),
}

impl AsFd for EventType {
    fn as_fd(&self) -> BorrowedFd<'_> {
        match self {
            EventType::Client(client) => client.as_fd(),
            EventType::TrafficRecv(traffic_recv) => traffic_recv.as_fd(),
        }
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


    /// Adding a new client to the pool
    pub fn add_new_client(&self, socket: TcpStream) {
        self.new_client_tx.send(socket).unwrap();
    }


    /// Get current number of clients in the pool
    pub fn get_nb_clients(&self) -> usize {
        self.nb_clients.load(Ordering::Relaxed)
    }


    fn work_thread(new_client_rx: mpsc::Receiver<TcpStream>, nb_clients: Arc<AtomicUsize>) {
        // Event list
        let mut events = Vec::new();
        let mut free_events = Vec::new();      // Index of free events (EventType::None) in events Vec

        // Create the epoll instance
        let epoll = Epoll::new(EpollCreateFlags::empty()).unwrap();

        // Add the traffic receiver event
        Self::add_event(&epoll, &mut events, &mut free_events, EventType::TrafficRecv(internal_com::Receiver::new(true /* nonblocking */)));

        let mut epoll_events = [EpollEvent::empty(); 100];
        loop {
            // Wait for events
            let nb_events = epoll.wait(&mut epoll_events, 100u16 /* milliseconds */).unwrap();

            // Read the events
            for epoll_event in epoll_events.iter().take(nb_events) {
                let event_index = usize::try_from(epoll_event.data()).unwrap();
                match &mut events[event_index] {
                    Some(EventType::Client(client)) => {
                        // Process the client event
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
                                Self::delete_client(&mut events[event_index], event_index, &epoll, &mut free_events, &nb_clients);
                            }
                        }
                    }

                    Some(EventType::TrafficRecv(traffic_recv)) => {
                        // Process the traffic receiver event
                        let traffic_infos = traffic_recv.recv();
                        match traffic_infos {
                            Err(e) => log::warn!("Traffic receive error : {}", e),
                            Ok(infos) => {
                                // Send the traffic information to all clients
                                for (i, event) in events.iter_mut().enumerate() {
                                    if let Some(EventType::Client(client)) = event {
                                        if let Err(e) = client.send_traffic(&infos) {
                                            log::warn!("Send error ({}) to client {}", e, client.address());
                                            Self::delete_client(event, i, &epoll, &mut free_events, &nb_clients);
                                        }
                                    }
                                }
                            }
                        }
                    }

                    None => {
                        // An event may have occurred on an entry that has since been deleted
                        // => We ignore it
                    }
                }
            }

            // Check if there are new clients
            Self::check_new_client(&new_client_rx, &epoll, &mut events, &mut free_events, &nb_clients);
        }
    }


    fn add_event(epoll: &Epoll, events: &mut Vec<Option<EventType>>, free_events: &mut Vec<usize>, event: EventType) {
        let event_index;

        // If there are free events, we reuse one
        if let Some(i) = free_events.pop() {
            assert!(events[i].is_none());
            events[i] = Some(event);
            event_index = i;
        }
        // Otherwise we add a new event
        else {
            events.push(Some(event));
            event_index = events.len() - 1;
        }

        // Register the event in epoll
        epoll.add(events[event_index].as_ref().unwrap().as_fd(),
            EpollEvent::new(EpollFlags::EPOLLIN, event_index as u64)).unwrap();
    }


    fn delete_event(event: &mut Option<EventType>, event_index: usize, epoll: &Epoll, free_events: &mut Vec<usize>) {
        // Unregister the event in epoll
        epoll.delete(event.as_ref().unwrap().as_fd()).unwrap();

        // Free the event
        *event = None;
        free_events.push(event_index);
    }


    fn delete_client(event: &mut Option<EventType>, event_index: usize, epoll: &Epoll, free_events: &mut Vec<usize>, nb_clients: &Arc<AtomicUsize>) {
        match event.as_ref().unwrap() {
            EventType::Client(client) => {
                log::info!("Client {} is disconnected", client.address());
                Self::delete_event(event, event_index, epoll, free_events);
                nb_clients.fetch_sub(1, Ordering::Relaxed);
            }
            EventType::TrafficRecv(_) => panic!("Event {event_index} is not a client"),
        }
    }


    fn check_new_client(new_client_rx: &mpsc::Receiver<TcpStream>, epoll: &Epoll,
        events: &mut Vec<Option<EventType>>, free_events: &mut Vec<usize>, nb_clients: &Arc<AtomicUsize>) {

        // While there are new clients, we add them to the pool
        while let Ok(socket) = new_client_rx.try_recv() {
            let client_addr = socket.peer_addr().unwrap();

            // If the maximum number of clients is reached, we refuse the new client
            let current_nb_clients = nb_clients.fetch_add(1, Ordering::Relaxed);
            if current_nb_clients >= CLIENTS_MAX_NB {
                nb_clients.fetch_sub(1, Ordering::Relaxed);
                log::warn!("Unable to connect new client {} : maximum number of clients ({}) for the pool is reached", client_addr, CLIENTS_MAX_NB);
            }
            else {
                // Add the new client to the event list
                socket.set_nonblocking(true).unwrap();  // We can't block all clients because of one blocking client
                Self::add_event(epoll, events, free_events, EventType::Client(client::Client::new(socket)));
                log::info!("New client connected : {}, {}th in the pool", client_addr, current_nb_clients + 1);
            }
        }
    }


}