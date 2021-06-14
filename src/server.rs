use crate::database::Database;
use crate::logger::Logger;
use crate::request::{self, Request};
use crate::server_conf::ServerConf;
use std::path::Path;

use std::net::TcpListener;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

pub struct Server {
    database: Arc<Mutex<Database>>,
    listener: TcpListener,
    config: Arc<Mutex<ServerConf>>,
}

impl Server {
    pub fn new(config_file: &str) -> Result<Server, String> {
        let config = ServerConf::new(config_file)?;
        let addr = config.addr();
        let listener = TcpListener::bind(addr).expect("Could not bind");
        listener
            .set_nonblocking(true)
            .expect("Cannot set non-blocking");
        let database = Arc::new(Mutex::new(Database::new()));
        let config = Arc::new(Mutex::new(config));

        Ok(Server {
            database,
            listener,
            config,
        })
    }

    pub fn run(mut self) {
        let (log_sender, log_rec) = mpsc::channel();
        let path = Path::new("log.txt");
        let mut logger = Logger::new(path, log_rec);

        thread::spawn(move || {
            logger.run();
        });

        loop {
            if let Ok((stream, _)) = self.listener.accept() {
                let database = self.database.clone();
                let config = self.config.clone();
                let log_sender = log_sender.clone();
                let mut stream = stream;

                thread::spawn(move || {
                    let log_sender = &log_sender;
                    let database = &database;
                    let config = &config;
                    loop {
                        match request::parse_request(&mut stream) {
                            Ok(request) => {
                                let request = Request::new(&request);
                                log_sender.send(request.to_string()).unwrap();
                                let reponse = request.execute(database, config);
                                log_sender.send(reponse.to_string()).unwrap();
                                reponse.respond(&mut stream, log_sender);
                            }
                            Err(err) => {
                                let request = Request::invalid_request(err);
                                log_sender.send(request.to_string()).unwrap();
                                let reponse = request.execute(database, config);
                                log_sender.send(reponse.to_string()).unwrap();
                                reponse.respond(&mut stream, log_sender);
                            }
                        }
                    }
                });
            }

            let addr = self.listener.local_addr().unwrap().to_string();
            let config = self.config.clone();
            let config = config.lock().unwrap();
            let new_addr = config.addr();
            if addr != new_addr {
                self.listener = TcpListener::bind(new_addr).expect("Could not bind");
                self.listener
                    .set_nonblocking(true)
                    .expect("Cannot set non-blocking");
            }
        }
    }
}
