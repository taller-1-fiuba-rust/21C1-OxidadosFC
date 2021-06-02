use std::sync::mpsc::Sender;
use core::fmt::{Display, Formatter, Result};
use crate::database::{Database};
use std::sync::{Arc, Mutex};
use std::net::TcpStream;
use std::io::{Read, Write};


pub enum Request<'a> {
    Valid(Command<'a>),
    Wrong(RequestError),

}

pub enum RequestError {
    NoInputError,
    NotUtf8CharError,
    GeneralError,
}

pub enum Command<'a> {
    Append(&'a str, &'a str),
    Incrby(&'a str, &'a str),
    Decrby(&'a str, &'a str),
    Get(&'a str),
    Getdel(&'a str),
    Getset(&'a str, &'a str),
    Set(&'a str, &'a str),
    None,
}

pub enum Reponse<'a> {
    Valid(&'a str),
    Error(&'a str)
}
 
impl<'a> Request<'a> {
    pub fn parse_request(stream: &mut TcpStream) -> Request<'a> {
        let mut buf = [0; 512];
    
        match stream.read(&mut buf) {
            Ok(bytes_read) if bytes_read == 0 => Request::Wrong(RequestError::NoInputError),
            Ok(bytes_read) => match std::str::from_utf8(&buf[..bytes_read]) {
                Ok(command) => Request::Valid(Command::new(&command)),
                Err(_) => Request::Wrong(RequestError::NotUtf8CharError),
            },
            Err(_) => Request::Wrong(RequestError::GeneralError),
        }
    }

    pub fn execute(self, db: Arc<Mutex<Database>>) -> Reponse<'a> {
        match self {
            Request::Valid(command) => self.valid_request(command, db),
            Request::Wrong(error) => Reponse::Error(&error.to_string()),
        }
    }    

    fn valid_request(self, command: Command,  db: Arc<Mutex<Database>>) -> Reponse{
        let mut db = db.lock().unwrap();
        
        let result = match command {
            Command::Append(key, value) => db.append(key, value),
            Command::Incrby(key, number_of_incr) => db.incrby(key, number_of_incr),
            Command::Decrby(key, number_of_decr) => db.decrby(key, number_of_decr),
            Command::Get(key) => db.get(key),
            Command::Getdel(key) => db.getdel(key),
            Command::Getset(key, value) => db.getset(key, value),
            Command::Set(key, value) => db.set(key, value),
            Command::None => return Reponse::Error("Unknown Command"),
        };

        match result {
            Ok(value) => Reponse::Valid(&value),
            Err(dbError) => Reponse::Error(&dbError.to_string()),
        }
    }
}


impl<'a> Reponse<'a> {
    pub fn respond(self, mut stream: TcpStream, log_sender: Sender<String>) {
        match self {
            Reponse::Valid(message) => {
                if let Err(e) = writeln!(stream, "{}\n", message) {
                    log_sender.send("response could not be sent".to_string());
                }
            }
            Reponse::Error(message) => {
                if let Err(e) = writeln!(stream, "Error: {}\n", message) {
                    log_sender.send("response could not be sent".to_string());
                }
            }
        };
    }
}

impl<'a> Command<'a> {
    pub fn new(command: &str) -> Command {
        let command: Vec<&str> = command.trim().split_whitespace().collect();

        match &command[..] {
            ["append", key, value] => Command::Append(key, value),
            ["incrby", key, number_of_incr] => Command::Incrby(key, number_of_incr),
            ["decrby", key, number_of_decr] => Command::Decrby(key, number_of_decr),
            ["get", key] => Command::Get(key),
            ["getdel", key] => Command::Getdel(key),
            ["getset", key, value] => Command::Getset(key, value),
            ["set", key, value] => Command::Set(key, value),
            _ => Command::None,
        }
    }
}


impl<'a> Display for Command<'a> {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            Command::Append(key,value) => write!(f, "Append {} to key {}", value, key),
            Command::Incrby(key,value) => write!(f, "Incrby {} the value with key {}", value, key),
            Command::Decrby(key,value) => write!(f, "Decrby {} the value with key {}", value, key),
            Command::Get(key) => write!(f, "Get the value with key {}", key),
            Command::Getdel(key) => write!(f, "Get the value with key {} and delete", key),
            Command::Getset(key, value) => write!(f, "Get the value with key {} and set the new value {}", key, value),
            Command::Set(key, value) => write!(f, "Set the value {} with key {}", value, key),
            Command::None => write!(f, "Wrong Command"),
        }
    }
}

impl<'a> Display for Request<'a> {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            Request::Valid(command) => writeln!(f, "Request: {}", command ),
            Request::Wrong(error) => writeln!(f, "Request: {}", error ),
        }
    }
}

impl<'a> Display for Reponse<'a> {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            Reponse::Valid(message) => writeln!(f, "Request: {}", message ),
            Reponse::Error(error) => writeln!(f, "Request: {}", error ),
        }
    }
}

impl<'a> Display for RequestError {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            RequestError::NoInputError =>  write!(f, "Empty request"),
            RequestError::NotUtf8CharError =>  write!(f, "Not utf8 string"),
            RequestError::GeneralError =>  write!(f, "Unknown Request"),
        }
    }
}

