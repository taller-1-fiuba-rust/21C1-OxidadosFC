use crate::database::Database;
use core::fmt::{Display, Formatter, Result};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

pub enum Request {
    Valid(Command),
    Wrong(RequestError),
}

pub enum RequestError {
    NoInputError,
    NotUtf8CharError,
    GeneralError,
}

pub enum Command {
    Append(String, String),
    Incrby(String, String),
    Decrby(String, String),
    Get(String),
    Copy(String, String),
    Del(String),
    Exists(String),
    Keys(String),
    Rename(String, String),
    Getdel(String),
    Getset(String, String),
    Set(String, String),
    Strlen(String),
    Mset(Vec<String>),
    Mget(Vec<String>),
    Sadd(String, String),
    Sismember(String, String),
    Scard(String),
    None,
}

pub enum Reponse {
    Valid(String),
    Error(String),
}

impl Request {
    pub fn parse_request(stream: &mut TcpStream) -> Request {
        let mut buf = [0; 512];

        match stream.read(&mut buf) {
            Ok(bytes_read) if bytes_read == 0 => Request::Wrong(RequestError::NoInputError),
            Ok(bytes_read) => match std::str::from_utf8(&buf[..bytes_read]) {
                Ok(command) => Request::Valid(Command::new(command)),
                Err(_) => Request::Wrong(RequestError::NotUtf8CharError),
            },
            Err(_) => Request::Wrong(RequestError::GeneralError),
        }
    }

    pub fn execute(self, db: &Arc<Mutex<Database>>) -> Reponse {
        match self {
            Request::Valid(command) => {
                let mut db = db.lock().unwrap();

                let result = match command {
                    Command::Append(key, value) => db.append(key, value),
                    Command::Incrby(key, number_of_incr) => db.incrby(key, number_of_incr),
                    Command::Decrby(key, number_of_decr) => db.decrby(key, number_of_decr),
                    Command::Get(key) => db.get(key),
                    Command::Getdel(key) => db.getdel(key),
                    Command::Getset(key, value) => db.getset(key, value),
                    Command::Set(key, value) => db.set(key, value),
                    Command::Copy(key, to_key) => db.copy(key, to_key),
                    Command::Del(key) => db.del(key),
                    Command::Exists(key) => db.exists(key),
                    Command::Keys(pattern) => db.keys(pattern),
                    Command::Rename(old_key, new_key) => db.rename(old_key, new_key),
                    Command::Strlen(key) => db.strlen(&key),
                    Command::Mset(vec_str) => db.mset(&vec_str[1..]),
                    Command::Mget(vec_str) => db.mget(&vec_str[1..]),
                    Command::Sadd(set_key, value) => db.sadd(set_key, value),
                    Command::Sismember(set_key, value) => db.sismember(set_key, value),
                    Command::Scard(set_key) => db.scard(set_key),
                    Command::None => return Reponse::Error("Unknown Command".to_owned()),
                };

                match result {
                    Ok(value) => Reponse::Valid(value),
                    Err(db_error) => Reponse::Error(db_error.to_string()),
                }
            }

            Request::Wrong(error) => Reponse::Error(error.to_string()),
        }
    }
}

impl Reponse {
    pub fn respond(self, stream: &mut TcpStream, log_sender: &Sender<String>) {
        match self {
            Reponse::Valid(message) => {
                if writeln!(stream, "{}\n", message).is_err() {
                    log_sender
                        .send("response could not be sent".to_string())
                        .unwrap();
                }
            }
            Reponse::Error(message) => {
                if writeln!(stream, "Error: {}\n", message).is_err() {
                    log_sender
                        .send("response could not be sent".to_string())
                        .unwrap();
                }
            }
        };
    }
}

impl Command {
    pub fn new(command: &str) -> Command {
        let command: Vec<&str> = command.trim().split_whitespace().collect();

        match command[..] {
            ["append", key, value] => Command::Append(key.to_owned(), value.to_owned()),
            ["incrby", key, number_of_incr] => {
                Command::Incrby(key.to_owned(), number_of_incr.to_owned())
            }
            ["decrby", key, number_of_decr] => {
                Command::Decrby(key.to_owned(), number_of_decr.to_owned())
            }
            ["get", key] => Command::Get(key.to_owned()),
            ["getdel", key] => Command::Getdel(key.to_owned()),
            ["getset", key, value] => Command::Getset(key.to_owned(), value.to_owned()),
            ["set", key, value] => Command::Set(key.to_owned(), value.to_owned()),
            ["copy", key, to_key] => Command::Copy(key.to_owned(), to_key.to_owned()),
            ["del", key] => Command::Del(key.to_owned()),
            ["exists", key] => Command::Exists(key.to_owned()),
            ["keys", pattern] => Command::Keys(pattern.to_owned()),
            ["rename", old_key, new_key] => Command::Rename(old_key.to_owned(), new_key.to_owned()),
            ["strlen", key] => Command::Strlen(key.to_owned()),
            ["mset", ..] => Command::Mset(command.iter().map(|x| x.to_string()).collect()),
            ["mget", ..] => Command::Mget(command.iter().map(|x| x.to_string()).collect()),
            ["sadd", key_set, element] => Command::Sadd(key_set.to_owned(), element.to_owned()),
            ["sismember", key_set, element] => {
                Command::Sismember(key_set.to_owned(), element.to_owned())
            }
            ["scard", key_set] => Command::Scard(key_set.to_owned()),
            _ => Command::None,
        }
    }
}

impl Display for Command {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            Command::Append(key, value) => write!(f, "Append {} to key {}", value, key),
            Command::Incrby(key, value) => write!(f, "Incrby {} the value with key {}", value, key),
            Command::Decrby(key, value) => write!(f, "Decrby {} the value with key {}", value, key),
            Command::Get(key) => write!(f, "Get the value with key {}", key),
            Command::Getdel(key) => write!(f, "Get the value with key {} and delete", key),
            Command::Getset(key, value) => write!(
                f,
                "Get the value with key {} and set the new value {}",
                key, value
            ),
            Command::Set(key, value) => write!(f, "Set the value {} with key {}", value, key),
            Command::Copy(key, to_key) => {
                write!(f, "Copy the value in key {} to the key {} ", key, to_key)
            }
            Command::Del(key) => write!(f, "Delete the key {}", key),
            Command::Exists(key) => write!(f, "Is the key {} present?", key),
            Command::Keys(pattern) => write!(
                f,
                "Get the keys that match the following pattern {}",
                pattern
            ),
            Command::Rename(old_key, new_key) => write!(
                f,
                "Rename the key with name {} to name {}",
                old_key, new_key
            ),
            Command::Strlen(key) => write!(f, "Get length of the value with key {}", key),
            Command::Mget(params) => write!(f, "Mget {:?}", params),
            Command::Mset(params) => write!(f, "Mset {:?}", params),
            Command::Sadd(key_set, element) => {
                write!(f, "Sadd to key_set: {} and element: {}", key_set, element)
            }
            Command::Sismember(key_set, element) => write!(
                f,
                "Sismember to key_set: {} and element: {}",
                key_set, element
            ),
            Command::Scard(key_set) => write!(f, "Scard to key_set: {} ", key_set),
            Command::None => write!(f, "Wrong Command"),
        }
    }
}

impl<'a> Display for Request {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            Request::Valid(command) => writeln!(f, "Request: {}", command),
            Request::Wrong(error) => writeln!(f, "Request: Error: {}", error),
        }
    }
}

impl<'a> Display for Reponse {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            Reponse::Valid(message) => writeln!(f, "Reponse: {}", message),
            Reponse::Error(error) => writeln!(f, "Reponse: Error: {}", error),
        }
    }
}

impl<'a> Display for RequestError {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            RequestError::NoInputError => write!(f, "Empty request"),
            RequestError::NotUtf8CharError => write!(f, "Not utf8 string"),
            RequestError::GeneralError => write!(f, "Unknown Request"),
        }
    }
}
