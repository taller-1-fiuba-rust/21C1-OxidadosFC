use crate::database::Database;
use crate::logger::Logger;
use crate::request::{self, Reponse, Request};
use crate::server_conf::ServerConf;
use std::collections::HashMap;
use std::net::TcpListener;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

pub struct Server {
    database: Database,
    listener: TcpListener,
    config: ServerConf,
    channels: Arc<Mutex<HashMap<String, Vec<Sender<String>>>>>,
}

impl Server {
    pub fn new(config_file: &str) -> Result<Server, String> {
        let config = ServerConf::new(config_file)?;
        let listener = TcpListener::bind(config.addr()).expect("Could not bind");
        let database = Database::new();

        listener
            .set_nonblocking(true)
            .expect("Cannot set non-blocking");

        let channels = Arc::new(Mutex::new(HashMap::new()));
        let mut guard = channels.lock().unwrap();

        guard.insert("Monitor".to_string(), Vec::new());

        drop(guard);

        Ok(Server {
            database,
            listener,
            config,
            channels,
        })
    }

    pub fn run_message_handler(&self) -> Sender<(Vec<String>, String)> {
        let (sender, reci): (
            Sender<(Vec<String>, String)>,
            Receiver<(Vec<String>, String)>,
        ) = channel();
        let channels = self.channels.clone();

        thread::spawn(move || {
            for rec in reci.iter() {
                let list = rec.0;
                let msg = &rec.1;
                for elem in list {
                    let guard = channels.lock().unwrap();
                    let listeners = guard.get(&elem).unwrap();
                    listeners.iter().for_each(|x| {
                        x.send(msg.to_string()).unwrap();
                    });
                }
            }
        });

        sender
    }

    pub fn run(mut self) {
        let mut logger = Logger::new(&self.config.logfile());
        let log_sender = logger.run();
        let mut list_logger = Vec::new();
        list_logger.push(log_sender);

        let mut guard = self.channels.lock().unwrap();
        guard.insert("Logger".to_string(), list_logger);
        drop(guard);

        let sender = self.run_message_handler();

        loop {
            if let Ok((stream, _)) = self.listener.accept() {
                let mut database = self.database.clone();
                let mut config = self.config.clone();
                let mut channels = self.channels.clone();
                let msg_sender = sender.clone();

                let mut stream = stream;

                thread::spawn(move || {
                    let database = &mut database;
                    let config = &mut config;
                    let channels = &mut channels;
                    let msg_sender = &msg_sender;
                    
                    loop {
                        let request = request::parse_request(&mut stream);
                        let request = Request::new(&request);
                        let list = ["Logger".to_string(), "Monitor".to_string()].to_vec();

                        msg_sender.send((list, request.to_string())).unwrap();

                        let respond = match request {
                            Request::DataBase(query) => query.exec_query(database),
                            Request::Server(request) => request.exec_request(config),
                            Request::Suscriber(request) => request.execute(&mut stream, channels),
                            Request::Invalid(err) => Reponse::Error(err.to_string()),
                        };

                        let list = ["Logger".to_string()].to_vec();
                        msg_sender.send((list, respond.to_string())).unwrap();

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
