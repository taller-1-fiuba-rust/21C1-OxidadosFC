use crate::database::Database;
use crate::logger::Logger;
use crate::request::{self, Request};
use crate::server_conf::ServerConf;
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::sync::{Arc, Mutex};

pub struct Server {
    database: Database,
    listener: TcpListener,
    config: ServerConf,
}

impl Server {
    pub fn new(config_file: &str) -> Result<Server, String> {
        let config = ServerConf::new(config_file)?;
        let addr = config.addr();
        let listener = TcpListener::bind(addr).expect("Could not bind");
        let (ttl_sender, ttl_rec) = mpsc::channel();

        let database = Database::new(ttl_sender);
        
        let database_ttl = database.clone();
       
        database_ttl.ttl_supervisor_run(ttl_rec);

        listener
            .set_nonblocking(true)
            .expect("Cannot set non-blocking");


        Ok(Server {
            database,
            listener,
            config,
        })
    }

    pub fn run(mut self) {
        let (log_sender, log_rec) = mpsc::channel();
        let lf = self.config.logfile();
        let mut logger = Logger::new(&lf, log_rec);

        thread::spawn(move || {
            logger.run();
        });

        let config_shr = Arc::new(Mutex::new(self.config));

        loop {
            if let Ok((stream, _)) = self.listener.accept() {
                let mut database = self.database.clone();
                let config = config_shr.clone();
                let log_sender = log_sender.clone();
                let mut stream = stream;

                thread::spawn(move || {
                    let log_sender = &log_sender;
                    let database = &mut database;
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
            let config = config_shr.clone();
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
