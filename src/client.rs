use crate::channels::Channels;
use crate::database::Database;
use crate::logger::Logger;
use crate::request::{self, Reponse, Request};
use crate::server_conf::ServerConf;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

pub struct Client {
    stream: TcpStream,
    subscriptions: Vec<String>,
    id: u32,
    total_clients: Arc<Mutex<u64>>,
    logger_ref: Arc<Mutex<Logger>>,
}

impl Client {
    pub fn new(
        stream: TcpStream,
        id: u32,
        total_clients: Arc<Mutex<u64>>,
        logger_ref: Arc<Mutex<Logger>>,
    ) -> Client {
        let subscriptions = Vec::new();
        Client {
            stream,
            subscriptions,
            id,
            total_clients,
            logger_ref,
        }
    }

    pub fn handle_client(
        &mut self,
        mut database: Database,
        mut channels: Channels,
        uptime: SystemTime,
        mut config: ServerConf,
    ) {
        let mut a_live = true;
        let mut subscription_mode = false;

        while a_live {
            let time_out = config.time_out();
            let time_out = if time_out > 0 && !subscription_mode {
                Some(Duration::from_secs(time_out))
            } else {
                None
            };

            self.stream.set_read_timeout(time_out).unwrap();

            let mut logger = self.logger_ref.lock().unwrap();
            logger.set_verbose(config.verbose());
            drop(logger);

            match request::parse_request(&mut self.stream) {
                Ok(request) => {
                    let request = Request::new(&request, subscription_mode);
                    let respond = match request {
                        Request::DataBase(query) => {
                            self.emit_request(query.to_string(), &mut channels);
                            query.exec_query(&mut database)
                        }
                        Request::Server(request) => {
                            self.emit_request(request.to_string(), &mut channels);
                            request.exec_request(&mut config, uptime, self.total_clients.clone())
                        }
                        Request::Publisher(request) => {
                            self.emit_request(request.to_string(), &mut channels);
                            request.execute(&mut channels)
                        }
                        Request::Suscriber(request) => {
                            self.emit_request(request.to_string(), &mut channels);
                            request.execute(
                                &mut self.stream,
                                &mut channels,
                                &mut self.subscriptions,
                                self.id,
                                &mut subscription_mode,
                            )
                        }
                        Request::Touch(key) => {
                            let r = database.touch(key);
                            let (response, time) = match r {
                                Some(time) => ("(Integer) 1".to_string(), time),
                                None => ("(Integer) 0".to_string(), 0),
                            };
                            let msg = format!("{} - Time since last access: {}", request, time);
                            self.emit_request(msg, &mut channels);
                            Reponse::Valid(response)
                        }
                        Request::Invalid(_, _) => Reponse::Error(request.to_string()),
                        Request::CloseClient => {
                            a_live = false;
                            let mut clients = self.total_clients.lock().unwrap();
                            *clients -= 1;
                            Reponse::Valid("OK".to_string())
                        }
                    };
                    if let Reponse::Valid(msg) = &respond {
                        channels.send_logger(self.id, msg);
                    }

                    respond.respond(&mut self.stream);
                }
                Err(error) => {
                    a_live = false;
                    let mut clients = self.total_clients.lock().unwrap();
                    *clients -= 1;
                    if error != "EOF" {
                        let response = Reponse::Error(error);
                        response.respond(&mut self.stream);
                    }
                }
            }
        }

        for subs in self.subscriptions.iter() {
            channels.unsubscribe(&subs, self.id);
        }
    }

    fn emit_request(&mut self, request: String, channels: &mut Channels) {
        channels.send_logger(self.id, &request);
        channels.send_monitor(self.id, &request);
    }
}
