use crate::channels::Channels;
use crate::database::Database;
use crate::request::{self, Reponse, Request};
use crate::server_conf::ServerConf;
use std::net::TcpStream;

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
        loop {
            if let Some(request) = request::parse_request(&mut self.stream) {
                let request = Request::new(&request);
                let respond = match request {
                    Request::DataBase(query) => {
                        self.emit_request(query.to_string());
                        query.exec_query(&mut self.database)
                    }
                    Request::Server(request) => {
                        self.emit_request(request.to_string());
                        request.exec_request(&mut self.config)
                    }
                    Request::Suscriber(request) => {
                        self.emit_request(request.to_string());
                        request.execute(
                            &mut self.stream,
                            &mut self.channels,
                            &mut self.subscriptions,
                            self.id,
                        )
                    }
                    Request::Invalid(_, _) => Reponse::Error(request.to_string()),
                };

                self.emit_reponse(respond.to_string());
                respond.respond(&mut self.stream);
            }
        }
    }

    fn emit_request(&mut self, request: String) {
        self.channels.send_logger(self.id, &request);
        self.channels.send_monitor(self.id, &request);
    }

    fn emit_reponse(&mut self, respond: String) {
        self.channels.send_logger(self.id, &respond);
    }
}
