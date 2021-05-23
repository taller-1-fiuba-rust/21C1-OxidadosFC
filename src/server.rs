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

        match Command::new(&command) {
            Command::Append(key, value) => db.append(key, value),
            Command::Print => println!("{}", db),
            Command::None => println!("Wrong Command!"),
        }

        stream.write_all(&buf[..bytes_read])?;
    }
}
