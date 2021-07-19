use crate::channels::Channels;
use crate::client::Client;
use crate::database::Database;
use crate::logger::Logger;
use crate::server_conf::ServerConf;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::SystemTime;

pub struct Server {
    database: Database,
    listener: TcpListener,
    config: ServerConf,
    next_id: Arc<Mutex<u32>>,
    channels: Channels,
    uptime: SystemTime,
    clients: Arc<Mutex<u64>>,
}

impl Server {
    pub fn new(config_file: &str) -> Result<Server, String> {
        let config = ServerConf::new(config_file)?;
        let listener = TcpListener::bind(config.addr()).expect("Could not bind");
        let database = Database::new(config.dbfilename());
        let next_id = Arc::new(Mutex::new(1));
        let channels = Channels::new();
        let uptime = SystemTime::now();
        let clients = Arc::new(Mutex::new(0));

        Ok(Server {
            database,
            listener,
            config,
            next_id,
            channels,
            uptime,
            clients,
        })
    }

    fn new_client(&self, stream: TcpStream, id: u32, logger_ref: Arc<Mutex<Logger>>) -> Client {
        let total_clients = self.clients.clone();
        let mut clients = self.clients.lock().unwrap();
        *clients += 1;
        drop(clients);

        Client::new(stream, id, total_clients, logger_ref)
    }

    fn get_next_id(&self) -> u32 {
        let next_id = self.next_id.clone();
        let mut guard = next_id.lock().unwrap();
        let id = *guard;
        *guard = id + 1;
        id
    }

    pub fn run(mut self) {
        let mut logger = Logger::new(&self.config.logfile(), self.config.verbose());
        let log_sender = logger.run();
        self.channels.add_logger(log_sender);
        let logger_ref = Arc::new(Mutex::new(logger));

        for stream in self.listener.incoming() {
            let logger_ref = logger_ref.clone();
            match stream {
                Err(e) => eprintln!("failed: {}", e),
                Ok(stream) => {
                    let id = self.get_next_id();
                    let database = self.database.clone();
                    let channels = self.channels.clone();
                    let uptime = self.uptime;
                    let config = self.config.clone();
                    let mut client = self.new_client(stream, id, logger_ref);

                    thread::spawn(move || {
                        client.handle_client(database, channels, uptime, config);
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod server_test {
    use std::io::{BufRead, BufReader, Write};
    use std::str;

    use super::*;

    const ANS_SUCCESS: &str = "Ok\n";

    const SET_KEY_1: &str = "set key 1\n";
    const GET_KEY: &str = "get key\n";
    const DEL_KEY: &str = "del key\n";
    const APPEND_KEY_HOLA: &str = "append key hola\n";
    const APPEND_KEY_ADIOS: &str = "append key adios\n";
    const FLUSHDB: &str = "flushdb\n";

    fn run_server() {
        let server = Server::new("redis.conf").unwrap();
        thread::spawn(move || {
            server.run();
        });
    }

    fn integer_ans(integer: i32) -> String {
        format!("(integer) {}\n", integer)
    }

    fn test_command(client: &mut TcpStream, command: &str, expect: &str) {
        client.write(command.as_bytes()).expect("Failed to write to server");

        let mut buffer: Vec<u8> = Vec::new();
        let mut reader = BufReader::new(client);
        reader
            .read_until(b'\n', &mut buffer)
            .expect("Could not read into buffer");

            assert_eq!(str::from_utf8(&buffer).unwrap(), expect);
    }

    #[test]
    fn test() {
        run_server();
        test_strings_commands();
        test_two_clients();
        test_multiple_clients();
    }

    fn test_strings_commands() {
        let mut client = TcpStream::connect("0.0.0.0:8888").expect("Could not connect to server");
        test_command(&mut client, APPEND_KEY_HOLA, &integer_ans(4));
        test_command(&mut client, SET_KEY_1, ANS_SUCCESS);
        test_command(&mut client, GET_KEY, "1\n");
        test_command(&mut client, APPEND_KEY_ADIOS, &integer_ans(6));
        test_command(&mut client, GET_KEY, "1adios\n");
        test_command(&mut client, DEL_KEY, ANS_SUCCESS);
        test_command(&mut client, APPEND_KEY_HOLA, &integer_ans(4));
        test_command(&mut client, APPEND_KEY_ADIOS, &integer_ans(9));
        test_command(&mut client, GET_KEY, "holaadios\n");
        test_command(&mut client, FLUSHDB, ANS_SUCCESS);
    }

    fn test_two_clients() {
        let mut client1 = TcpStream::connect("0.0.0.0:8888").expect("Could not connect to server");
        let mut client2 = TcpStream::connect("0.0.0.0:8888").expect("Could not connect to server");
        test_command(&mut client1, APPEND_KEY_HOLA, &integer_ans(4));
        test_command(&mut client1, SET_KEY_1, ANS_SUCCESS);
        test_command(&mut client1, GET_KEY, "1\n");
        test_command(&mut client2, APPEND_KEY_ADIOS, &integer_ans(6));
        test_command(&mut client2, GET_KEY, "1adios\n");
        test_command(&mut client1, DEL_KEY, ANS_SUCCESS);
        test_command(&mut client1, APPEND_KEY_HOLA, &integer_ans(4));
        test_command(&mut client1, APPEND_KEY_ADIOS, &integer_ans(9));
        test_command(&mut client2, GET_KEY, "holaadios\n");
        test_command(&mut client1, FLUSHDB, ANS_SUCCESS);
    }

    fn test_multiple_clients() {
        let mut clients = Vec::new();
        for _ in 0..100 {
            clients.push(TcpStream::connect("0.0.0.0:8888").expect("Could not connect to server"));
        }
        
        let mut i = -1;
        for client in &mut clients {
            i += 1;
            let command = format!("set key1 {}", i);
            test_command(client, &command, ANS_SUCCESS);
        }
        
        for client in &mut clients {
            test_command(client, "get key1", &(i.to_string() + "\n"));
        }

        let mut client = TcpStream::connect("0.0.0.0:8888").expect("Could not connect to server");
        test_command(&mut client, APPEND_KEY_HOLA, &integer_ans(4));
        test_command(&mut client, SET_KEY_1, ANS_SUCCESS);
        test_command(&mut client, GET_KEY, "1\n");
        test_command(&mut client, APPEND_KEY_ADIOS, &integer_ans(6));

        for client in &mut clients {
            test_command(client, "get key", "1adios\n");
        }

        test_command(&mut client, DEL_KEY, ANS_SUCCESS);
        test_command(&mut client, APPEND_KEY_HOLA, &integer_ans(4));
        test_command(&mut client, APPEND_KEY_ADIOS, &integer_ans(9));

        for client in &mut clients {
            test_command(client, "get key", "holaadios\n");
        }
    }
}
