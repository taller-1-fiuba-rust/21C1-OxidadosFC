use crate::channels::{Channels, LOGGER, MONITOR};
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
    channels: Channels,
}

impl Server {
    pub fn new(config_file: &str) -> Result<Server, String> {
        let config = ServerConf::new(config_file)?;
        let listener = TcpListener::bind(config.addr()).expect("Could not bind");
        let database = Database::new();

        listener
            .set_nonblocking(true)
            .expect("Cannot set non-blocking");

        let channels = Channels::new();

        Ok(Server {
            database,
            listener,
            config,
            channels,
        })
    }

    pub fn run(mut self) {
        let mut logger = Logger::new(&self.config.logfile());
        let log_sender = logger.run();
        let mut list_logger = Vec::new();
        list_logger.push(log_sender);

        self.channels.try_add_channel(LOGGER, list_logger);

        loop {
            if let Ok((stream, _)) = self.listener.accept() {
                let mut database = self.database.clone();
                let mut config = self.config.clone();
                let mut channels = self.channels.clone();

                let mut stream = stream;

                thread::spawn(move || {
                    let database = &mut database;
                    let config = &mut config;
                    let channels = &mut channels;
                    
                    loop {
                        let request = request::parse_request(&mut stream);
                        let request = Request::new(&request);

                        channels.send(LOGGER, &request.to_string());
                        channels.send(MONITOR, &request.to_string());

                        let respond = match request {
                            Request::DataBase(query) => query.exec_query(database),
                            Request::Server(request) => request.exec_request(config),
                            Request::Suscriber(request) => request.execute(&mut stream, channels),
                            Request::Invalid(err) => Reponse::Error(err.to_string()),
                        };

                        channels.send(LOGGER, &respond.to_string());

                        respond.respond(&mut stream);
                    }
                });
            }

            if changed_port(&self.listener, &self.config) {
                self.listener = on_changed_port(&self.config);
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
