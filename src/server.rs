use crate::database::Database;
use crate::logger::Logger;
use crate::request::{self, Reponse, Request};
use crate::server_conf::ServerConf;
use std::net::TcpListener;
use std::thread;

pub struct Server {
    database: Database,
    listener: TcpListener,
    config: ServerConf,
}

impl Server {
    pub fn new(config_file: &str) -> Result<Server, String> {
        let config = ServerConf::new(config_file)?;
        let listener = TcpListener::bind(config.addr()).expect("Could not bind");
        let database = Database::new();

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
        let mut logger = Logger::new(&self.config.logfile());
        let log_sender = logger.run();

        loop {
            if let Ok((stream, _)) = self.listener.accept() {
                
                let mut database = self.database.clone();
                let mut config = self.config.clone();
                let log_sender = log_sender.clone();
                let mut stream = stream;

                thread::spawn(move || {
                    let log_sender = &log_sender;
                    let database = &mut database;
                    let config = &mut config;

                    loop {
                        let request = request::parse_request(&mut stream);
                        let request = Request::new(&request);

                        log_sender.send(request.to_string()).unwrap();

                        let respond = match request {
                            Request::DataBase(query) => query.exec_query(database),
                            Request::Server(request) => request.exec_request(config),
                            Request::Invalid(err) => Reponse::Error(err.to_string()),
                        };

                        respond.respond(&mut stream, &log_sender);
                    }
                });
            }

            if changed_port(&self.listener, &self.config) {
                self.listener = on_changed_port( &self.config);
            }
        }
    }
}


fn changed_port(listener: &TcpListener, config: &ServerConf) -> bool {
    let addr = listener.local_addr().unwrap().to_string();
    let new_addr = config.addr();
    addr != new_addr 
}


fn on_changed_port(config: &ServerConf) -> TcpListener {
    let new_addr = config.addr();
    let listener = TcpListener::bind(new_addr).expect("Could not bind");
    listener
        .set_nonblocking(true)
        .expect("Cannot set non-blocking");

    listener
}