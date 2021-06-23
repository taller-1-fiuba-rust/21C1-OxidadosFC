use crate::channels::Channels;
use crate::database::Database;
use crate::logger::Logger;
use crate::request::{self, Reponse, Request};
use crate::server_conf::ServerConf;
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;

pub struct Server {
    database: Database,
    listener: TcpListener,
    config: ServerConf,
    // event_comm: EventCommunication,
    next_id: Arc<Mutex<u32>>,
    channels: Channels,
}

impl Server {
    pub fn new(config_file: &str) -> Result<Server, String> {
        let config = ServerConf::new(config_file)?;
        let listener = TcpListener::bind(config.addr()).expect("Could not bind");
        let database = Database::new();
        // let event_comm = EventCommunication::new();
        let next_id = Arc::new(Mutex::new(1));
        let channels = Channels::new();

        Ok(Server {
            database,
            listener,
            config,
            // event_comm,
            next_id,
            channels,
        })
    }

    fn new_client(&self, stream: TcpStream, id: u32) -> Client {
        let database = self.database.clone();
        let config = self.config.clone();
        // let channels = self.event_comm.channels.clone();
        let channels = self.channels.clone();
        let stream = stream;
        let subscriptions = Vec::new();

        Client {
            stream,
            database,
            // msg_sender,
            channels,
            subscriptions,
            config,
            id,
        }
    }

    // fn run_message_handler(&self, log_sender: Sender<(String, String)>) -> Sender<EventMsg> {
    //     self.event_comm.run_event_communication_handler(log_sender)
    // }

    // fn run_logger(&self) -> Sender<(String, String)> {
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
        // let msg_sender = self.run_message_handler(log_sender);
        self.channels.add_logger(log_sender);

        for stream in self.listener.incoming() {
            match stream {
                Err(e) => eprintln!("failed: {}", e),
                Ok(stream) => {
                    let id = self.get_next_id();
                    // let mut client = self.new_client(msg_sender.clone(), stream, id);
                    let mut client = self.new_client(stream, id);

                    thread::spawn(move || {
                        client.handle_client();
                    });
                }
            }
        }
    }
}
// pub fn run(mut self) {
//     let mut logger = Logger::new(&self.config.logfile());
//     let log_sender = logger.run();
//     let mut list_logger = Vec::new();
//     list_logger.push(log_sender);

//     self.channels.try_add_channel(LOGGER, list_logger);

pub struct Client {
    stream: TcpStream,
    database: Database,
    // msg_sender: Sender<EventMsg>,
    // channels: Arc<Mutex<HashMap<String, Vec<Sender<(String, String)>>>>>,
    channels: Channels,
    subscriptions: Vec<String>,
    config: ServerConf,
    id: u32,
}

impl Client {
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
        // let event = EventMsg::new(
        //     [LOGGER.to_string(), MONITOR.to_string()].to_vec(),
        //     self.id,
        //     request,
        // );
        // self.msg_sender.send(event).unwrap();
        self.channels.send_logger(self.id, &request);
        self.channels.send_monitor(self.id, &request);
    }

    fn emit_reponse(&mut self, respond: String) {
        // let event = EventMsg::new([LOGGER.to_string()].to_vec(), self.id, respond);
        // self.msg_sender.send(event).unwrap();
        self.channels.send_logger(self.id, &respond);
    }
}

// pub struct EventMsg {
//     lisenerts: Vec<String>,
//     client_id: u32,
//     event: String,
// }

// impl EventMsg {
//     pub fn new(lisenerts: Vec<String>, client_id: u32, event: String) -> EventMsg {
//         EventMsg {
//             lisenerts,
//             client_id,
//             event,
//         }
//     }
// }

// pub struct EventCommunication {
//     channels: Arc<Mutex<HashMap<String, Vec<Sender<(String, String)>>>>>,
// }

// impl EventCommunication {
//     pub fn new() -> EventCommunication {
//         let channels = Arc::new(Mutex::new(HashMap::new()));
//         let mut guard = channels.lock().unwrap();
//         guard.insert(MONITOR.to_string(), Vec::new());
//         drop(guard);
//         EventCommunication { channels }
//     }

//     pub fn run_event_communication_handler(
//         &self,
//         log_sender: Sender<(String, String)>,
//     ) -> Sender<EventMsg> {
//         let (sender, reci): (Sender<EventMsg>, Receiver<EventMsg>) = channel();

//         let mut guard = self.channels.lock().unwrap();
//         guard.insert(LOGGER.to_string(), vec![log_sender]);
//         drop(guard);

//         let channels = self.channels.clone();

//         thread::spawn(move || {
//             for rec in reci.iter() {
//                 let list = rec.lisenerts;
//                 let id = &rec.client_id;
//                 let msg = &rec.event;
//                 for elem in list {
//                     let guard = channels.lock().unwrap();
//                     let listeners = guard.get(&elem).unwrap();
//                     listeners.iter().for_each(|x| {
//                         x.send((id.to_string(), msg.to_string())).unwrap();
//                     });

//                     drop(guard);
//                 }
//             }
//         });

//         sender
//     }
// }
