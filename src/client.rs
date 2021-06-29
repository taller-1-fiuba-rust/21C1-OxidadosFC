use crate::channels::Channels;
use crate::database::Database;
use crate::request::{self, Reponse, Request};
use crate::server_conf::ServerConf;
use std::net::TcpStream;
use std::time::Duration;

const SUBSCRIPTION_MODE_ERROR: &str = "Subscription mode doesn't support other commands";

pub struct Client {
    stream: TcpStream,
    database: Database,
    channels: Channels,
    subscriptions: Vec<String>,
    config: ServerConf,
    id: u32,
}

impl Client {
    pub fn new(
        stream: TcpStream,
        database: Database,
        channels: Channels,
        subscriptions: Vec<String>,
        config: ServerConf,
        id: u32,
    ) -> Client {
        Client {
            stream,
            database,
            channels,
            subscriptions,
            config,
            id,
        }
    }

    pub fn handle_client(&mut self) {
        let mut a_live = true;
        let mut subscription_mode = false;

        while a_live {
            let time_out = self.config.time_out();
            let time_out = if time_out > 0 && !subscription_mode {
                Some(Duration::from_secs(time_out))
            } else {
                None
            };

            let verbose = self.config.verbose();
            self.stream.set_read_timeout(time_out).unwrap();
            match request::parse_request(&mut self.stream) {
                Ok(request) if request.is_empty() => {}
                Ok(request) => {
                    let request = Request::new(&request);
                    let respond = match request {
                        Request::DataBase(query) => {
                            if subscription_mode {
                                Reponse::Error(SUBSCRIPTION_MODE_ERROR.to_string())
                            } else {
                                self.emit_request(query.to_string(), verbose);
                                query.exec_query(&mut self.database)
                            }
                        }
                        Request::Server(request) => {
                            if subscription_mode {
                                Reponse::Error(SUBSCRIPTION_MODE_ERROR.to_string())
                            } else {
                                self.emit_request(request.to_string(), verbose);
                                request.exec_request(&mut self.config)
                            }
                        }
                        Request::Publisher(request) => {
                            if subscription_mode {
                                Reponse::Error(SUBSCRIPTION_MODE_ERROR.to_string())
                            } else {
                                self.emit_request(request.to_string(), verbose);
                                request.execute(&mut self.channels)
                            }
                        }
                        Request::Suscriber(request) => {
                            self.emit_request(request.to_string(), verbose);
                            request.execute(
                                &mut self.stream,
                                &mut self.channels,
                                &mut self.subscriptions,
                                self.id,
                                &mut subscription_mode,
                            )
                        }
                        Request::Invalid(_, _) => Reponse::Error(request.to_string()),
                        Request::CloseClient => {
                            a_live = false;
                            Reponse::Valid("OK".to_string())
                        }
                    };
                    self.emit_reponse(respond.to_string(), verbose);
                    respond.respond(&mut self.stream);
                }
                Err(error) => {
                    a_live = false;
                    let response = Reponse::Error(error);
                    response.respond(&mut self.stream);
                }
            }
        }

        for subs in self.subscriptions.iter() {
            self.channels.unsubscribe(&subs, self.id);
        }
    }

    fn emit_request(&mut self, request: String, verbose: bool) {
        self.channels.send_logger(self.id, &request, verbose);
        self.channels.send_monitor(self.id, &request);
    }

    fn emit_reponse(&mut self, respond: String, verbose: bool) {
        self.channels.send_logger(self.id, &respond, verbose);
    }
}
