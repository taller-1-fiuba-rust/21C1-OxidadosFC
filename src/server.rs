use crate::channels::Channels;
use crate::client::Client;
use crate::database::Database;
use crate::logger::Logger;
use crate::server_conf::ServerConf;
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::SystemTime;

pub struct Server {
    database: Database,
    listener: TcpListener,
    config: ServerConf,
    next_id: Arc<Mutex<u32>>,
    channels: Channels,
    uptime: SystemTime,
    clients: Arc<Mutex<u64>>,
}

impl Server {
    pub fn new(config_file: &str) -> Result<Server, String> {
        let config = ServerConf::new(config_file)?;
        let listener = TcpListener::bind(config.addr()).expect("Could not bind");
        let database = Database::new();
        let next_id = Arc::new(Mutex::new(1));
        let channels = Channels::new();
        let uptime = SystemTime::now();
        let clients = Arc::new(Mutex::new(0));

        Ok(Server {
            database,
            listener,
            config,
            next_id,
            channels,
            uptime,
            clients,
        })
    }

    fn new_client(&self, stream: TcpStream, id: u32) -> Client {
        let subscriptions = Vec::new();
        let total_clients = self.clients.clone();
        let mut clients = self.clients.lock().unwrap();
        *clients += 1;
        drop(clients);

        Client::new(stream, subscriptions, id, total_clients)
    }

    fn run_logger(&self) -> Sender<String> {
        let mut logger = Logger::new(&self.config.logfile());
        logger.run()
    }

    fn get_next_id(&self) -> u32 {
        let next_id = self.next_id.clone();
        let mut guard = next_id.lock().unwrap();
        let id = *guard;
        *guard = id + 1;
        id
    }

    pub fn run(mut self) {
        let log_sender = self.run_logger();
        self.channels.add_logger(log_sender);

        for stream in self.listener.incoming() {
            match stream {
                Err(e) => eprintln!("failed: {}", e),
                Ok(stream) => {
                    let id = self.get_next_id();
                    let mut client = self.new_client(stream, id);
                    let database = self.database.clone();
                    let channels = self.channels.clone();
                    let uptime = self.uptime;
                    let config = self.config.clone();
                    thread::spawn(move || {
                        client.handle_client(database, channels, uptime, config);
                    });
                }
            }
        }
    }
}
