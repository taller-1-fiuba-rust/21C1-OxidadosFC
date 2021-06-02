
use crate::logger::Logger;
use crate::request::Request;
use std::path::Path;
use crate::database::{Database};

use std::net::{TcpListener};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

pub struct Server {
    database: Arc<Mutex<Database>>,
    listener: TcpListener,
}

impl Server {
    pub fn new(addr: &str) -> Server {
        let listener = TcpListener::bind(addr).expect("Could not bind");
        let database = Arc::new(Mutex::new(Database::new()));

        Server { database, listener }
    }

    pub fn run(self) {

        let (log_sender, log_rec) = mpsc::channel();
        let path = Path::new("log.txt");
        let mut logger = Logger::new(path, log_rec);

        thread::spawn(move || {
            logger.run();
        });

        for stream in self.listener.incoming() {
            match stream {
                Err(_) => {
                    log_sender.send("Io::Error".to_string());
                }
                Ok(stream) => {
                    let database = self.database.clone();
                    let log_sender = log_sender.clone();
                    thread::spawn(move || {
                        let request = Request::parse_request(&stream);
                        log_sender.send(request.to_string());
                        let reponse = request.execute(database);
                        log_sender.send(reponse.to_string());
                        reponse.respond(stream, log_sender);
                    });
                }
            }
        }
    }
}

