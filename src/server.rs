use crate::commands::Command;
use crate::database::Database;

use std::io::{Error, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

pub struct Server {
    database: Arc<Mutex<Database>>,
    listener: TcpListener,
}

impl Server {
    pub fn new(addr: &str) -> Server {
        let listener = TcpListener::bind(addr).expect("Could not bind");
        let database = Arc::new(Mutex::new(Database::new()));

        Server { database, listener }
    }

    pub fn run(self) {
        for stream in self.listener.incoming() {
            match stream {
                Err(e) => {
                    eprintln!("failed: {}", e)
                }
                Ok(stream) => {
                    let database = self.database.clone();
                    thread::spawn(move || {
                        handle_client(stream, database)
                            .unwrap_or_else(|error| eprintln!("{:?}", error));
                    });
                }
            }
        }
    }
}

fn handle_client(mut stream: TcpStream, db: Arc<Mutex<Database>>) -> Result<(), Error> {
    let mut buf = [0; 512];
    loop {
        let bytes_read = stream.read(&mut buf)?;

        if bytes_read == 0 {
            return Ok(());
        }

        let command = std::str::from_utf8(&buf[..bytes_read]).unwrap();
        let mut db = db.lock().unwrap();
        let result = match Command::new(&command) {
            Command::Append(key, value) => db.append(key, value),
            Command::Incrby(key, number_of_incr) => db.incrby(key, number_of_incr),
            Command::Decrby(key, number_of_decr) => db.decrby(key, number_of_decr),
            Command::Set(key, value) => db.set(key, value),
            Command::Get(key) => db.get(key),
            Command::Getdel(key) => db.getdel(key),
            Command::Getset(key, value) => db.getset(key, value),
            Command::Strlen(key) => db.strlen(key),
            Command::Mset(vec_str) => db.mset(&vec_str[1..]),
            Command::Mget(vec_str) => db.mget(&vec_str[1..]),
            Command::Print => Ok(format!("{}", db)),
            Command::None => Ok(String::from("Wrong Command")),
        };

        match result {
            Ok(success) => {
                let buffer_success = success.as_bytes();
                stream.write_all(&buffer_success)?;
                stream.write_all("\n".as_bytes())?;
            }
            Err(failure) => {
                let buffer_clone = failure.to_string().clone();
                let buffer_failure = buffer_clone.as_bytes();
                stream.write_all(&buffer_failure)?;
                stream.write_all("\n".as_bytes())?;
            }
        };
    }
}
