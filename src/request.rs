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
    Lindex(String, String),
    Llen(String),
    Lpop(String),
    Lpush(String, String),
    Lpushx(String, String),
    Lrange(String, String, String),
    Lrem(String, String, String),
    Lset(String, String, String),
    Rpop(String),
    Rpush(String, String),
    Rpushx(String, String),
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
                    Command::Append(key, value) => db.append(&key, value),
                    Command::Incrby(key, number_of_incr) => db.incrby(&key, number_of_incr),
                    Command::Decrby(key, number_of_decr) => db.decrby(&key, number_of_decr),
                    Command::Get(key) => db.get(&key),
                    Command::Getdel(key) => db.getdel(&key),
                    Command::Getset(key, value) => db.getset(&key, value),
                    Command::Set(key, value) => db.set(&key, value),
                    Command::Copy(key, to_key) => db.copy(&key, &to_key),
                    Command::Del(key) => db.del(&key),
                    Command::Exists(key) => db.exists(&key),
                    Command::Keys(pattern) => db.keys(pattern),
                    Command::Rename(old_key, new_key) => db.rename(&old_key, &new_key),
                    Command::Strlen(key) => db.strlen(&key),
                    Command::Mset(vec_str) => db.mset(&vec_str[1..]),
                    Command::Mget(vec_str) => db.mget(&vec_str[1..]),
                    Command::Lindex(key, indx) => db.lindex(&key, indx),
                    Command::Llen(key) => db.llen(&key),
                    Command::Lpop(key) => db.lpop(&key),
                    Command::Lpush(key, value) => db.lpush(&key, value),
                    Command::Lpushx(key, value) => db.lpushx(&key, value),
                    Command::Lrange(key, beg, end) => db.lrange(&key, beg, end),
                    Command::Lrem(key, rem, value) => db.lrem(&key, rem, value),
                    Command::Lset(key, index, value) => db.lset(&key, &index, &value),
                    Command::Rpop(key) => db.rpop(&key),
                    Command::Rpush(key, value) => db.rpush(&key, value),
                    Command::Rpushx(key, value) => db.rpushx(&key, value),
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
            ["incrby", key, n] => Command::Incrby(key.to_owned(), n.to_owned()),
            ["decrby", key, n] => Command::Decrby(key.to_owned(), n.to_owned()),
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
            ["lindex", key, value] => Command::Lindex(key.to_owned(), value.to_owned()),
            ["llen", key] => Command::Llen(key.to_owned()),
            ["lpop", key] => Command::Lpop(key.to_owned()),
            ["lpush", key, value] => Command::Lpush(key.to_owned(), value.to_owned()),
            ["lpushx", key, value] => Command::Lpushx(key.to_owned(), value.to_owned()),
            ["lrange", key, beg, end] => {
                Command::Lrange(key.to_owned(), beg.to_owned(), end.to_owned())
            }
            ["lrem", key, rem, value] => {
                Command::Lrem(key.to_owned(), rem.to_owned(), value.to_owned())
            }
            ["lset", key, index, value] => {
                Command::Lset(key.to_owned(), index.to_owned(), value.to_owned())
            }
            ["rpop", key] => Command::Rpop(key.to_owned()),
            ["rpush", key, value] => Command::Rpush(key.to_owned(), value.to_owned()),
            ["rpushx", key, value] => Command::Rpushx(key.to_owned(), value.to_owned()),
            ["sadd", key, element] => Command::Sadd(key.to_owned(), element.to_owned()),
            ["sismember", key, element] => Command::Sismember(key.to_owned(), element.to_owned()),
            ["scard", key] => Command::Scard(key.to_owned()),
            _ => Command::None,
        }
    }
}

impl Display for Command {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            Command::Append(key, value) => write!(
                f,
                "CommandString::Append - Key: {} - Value: {} ",
                key, value
            ),
            Command::Incrby(key, incr) => write!(
                f,
                "CommandString::Incrby - Key: {} - Increment: {}",
                key, incr
            ),
            Command::Decrby(key, incr) => write!(
                f,
                "CommandString::Decrby - Key: {} - Increment: {}",
                key, incr
            ),
            Command::Get(key) => write!(f, "CommandString::Get - Key: {}", key),
            Command::Getdel(key) => write!(f, "CommandString::Getdel - Key: {}", key),
            Command::Getset(key, value) => {
                write!(f, "CommandString::Getset - Key: {} - Value: {}", key, value)
            }

            Command::Mget(params) => {
                let mut parms_string = String::new();

                for elem in params {
                    parms_string.push_str(elem);
                    parms_string.push(' ');
                }

                write!(f, "CommandString::Mget Keys: {}", parms_string)
            }
            Command::Mset(params) => {
                let mut parms_string = String::new();

                for elem in params {
                    parms_string.push_str(elem);
                    parms_string.push(' ');
                }
                write!(f, "CommandString::Mset pair: {}", parms_string)
            }
            Command::Strlen(key) => write!(f, "CommandString::Strlen - Key: {}", key),
            Command::Set(key, value) => {
                write!(f, "CommandString::Set - Key: {} - Value: {}", key, value)
            }
            Command::Copy(key, to_key) => {
                write!(f, "CommandKeys::Copy - Key: {} - To_Key: {}", key, to_key)
            }
            Command::Del(key) => write!(f, "CommandKeys::Del - Key: {}", key),
            Command::Exists(key) => write!(f, "CommandKeys::Exists - Key: {}", key),
            Command::Keys(pattern) => write!(f, "CommandKeys::Keys - Pattern: {}", pattern),
            Command::Rename(old_key, new_key) => write!(
                f,
                "CommandKeys::Rename - Old_Key {} - New_Key {}",
                old_key, new_key
            ),
            Command::Lindex(key, indx) => {
                write!(f, "CommandList::Lindex - Key: {} - Index: {}", key, indx)
            }
            Command::Llen(key) => write!(f, "CommandList::Llen - Key {}", key),
            Command::Lpop(key) => write!(f, "CommandList::Lpop - Key {}", key),
            Command::Lpush(key, value) => {
                write!(f, "CommandList::Lpush - Key: {} - Value: {}", key, value)
            }
            Command::Lpushx(key, value) => {
                write!(f, "CommandList::Lpushx - Key: {} - Value: {}", key, value)
            }
            Command::Lrange(key, beg, end) => write!(
                f,
                "CommandList::Lrange - Key: {} - Begining: {} - End {}",
                key, beg, end
            ),
            Command::Lrem(key, rem, value) => write!(
                f,
                "CommandList::Lrem - Key: {} - Rem: {} - Value: {}",
                key, rem, value
            ),
            Command::Lset(key, index, value) => write!(
                f,
                "CommandList::Lset - Key: {} - Index: {} - Value: {}",
                key, index, value
            ),
            Command::Rpop(key) => write!(f, "CommandList::Rpop - Key: {}", key),
            Command::Rpush(key, value) => {
                write!(f, "CommandList::Rpush - Key: {} - Value: {} ", key, value)
            }
            Command::Rpushx(key, value) => {
                write!(f, "CommandList::Rpushx - Key: {} - Value: {} ", key, value)
            }
            Command::Sadd(key, element) => {
                write!(f, "CommandSet::Sadd - Key: {} - Element: {}", key, element)
            }
            Command::Sismember(key, element) => write!(
                f,
                "CommandSet::Sismember - Key: {} - Element: {}",
                key, element
            ),
            Command::Scard(key) => write!(f, "CommandSet::Sismember - Key: {}", key),
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
