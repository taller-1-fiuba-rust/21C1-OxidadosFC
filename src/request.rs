use crate::database::Database;
use core::fmt::{self, Display, Formatter};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

pub enum Request<'a> {
    Valid(Command<'a>),
    Invalid(RequestError),
}

pub enum RequestError {
    NoInputError,
    NotUtf8CharError,
    ParseError,
    InvalidCommand,
    InvalidNumberOfArguments,
}

pub enum Command<'a> {
    Append(&'a str, &'a str),
    Incrby(&'a str, i32),
    Decrby(&'a str, i32),
    Get(&'a str),
    Copy(&'a str, &'a str),
    Del(&'a str),
    Exists(&'a str),
    Keys(&'a str),
    Rename(&'a str, &'a str),
    Getdel(&'a str),
    Getset(&'a str, &'a str),
    Set(&'a str, &'a str),
    Strlen(&'a str),
    Mset(Vec<&'a str>),
    Mget(Vec<&'a str>),
    Lindex(&'a str, i32),
    Llen(&'a str),
    Lpop(&'a str),
    Lpush(&'a str, &'a str),
    Lpushx(&'a str, &'a str),
    Lrange(&'a str, i32, i32),
    Lrem(&'a str, i32, &'a str),
    Lset(&'a str, usize, &'a str),
    Rpop(&'a str),
    Rpush(&'a str, &'a str),
    Rpushx(&'a str, &'a str),
    Sadd(&'a str, &'a str),
    Sismember(&'a str, &'a str),
    Scard(&'a str),
    Flushdb(),
    Dbsize(),
}

pub enum Reponse {
    Valid(String),
    Error(String),
}

impl<'a> Request<'a> {
    pub fn new(request: &str) -> Request {
        let request: Vec<&str> = request.split_whitespace().collect();

        match request[..] {
            ["append", key, value] => Request::Valid(Command::Append(key, value)),
            ["incrby", key, incr] => match incr.parse::<i32>() {
                Ok(incr) => Request::Valid(Command::Incrby(key, incr)),
                Err(_) => Request::Invalid(RequestError::ParseError),
            },
            ["decrby", key, decr] => match decr.parse::<i32>() {
                Ok(decr) => Request::Valid(Command::Decrby(key, decr)),
                Err(_) => Request::Invalid(RequestError::ParseError),
            },
            ["get", key] => Request::Valid(Command::Get(key)),
            ["getdel", key] => Request::Valid(Command::Getdel(key)),
            ["getset", key, value] => Request::Valid(Command::Getset(key, value)),
            ["set", key, value] => Request::Valid(Command::Set(key, value)),
            ["copy", key, to_key] => Request::Valid(Command::Copy(key, to_key)),
            ["del", key] => Request::Valid(Command::Del(key)),
            ["exists", key] => Request::Valid(Command::Exists(key)),
            ["keys", pattern] => Request::Valid(Command::Keys(pattern)),
            ["rename", old_key, new_key] => Request::Valid(Command::Rename(old_key, new_key)),
            ["strlen", key] => Request::Valid(Command::Strlen(key)),
            ["mset", ..] => {
                let tail = &request[1..];
                let len = tail.len() as i32;

                if len > 0 && (len % 2 == 0) {
                    Request::Valid(Command::Mset(tail.to_vec()))
                } else {
                    Request::Invalid(RequestError::InvalidNumberOfArguments)
                }
            }
            ["mget", ..] => {
                let tail = &request[1..];
                Request::Valid(Command::Mget(tail.to_vec()))
            }
            ["lindex", key, index] => match index.parse::<i32>() {
                Ok(index) => Request::Valid(Command::Lindex(key, index)),
                Err(_) => Request::Invalid(RequestError::ParseError),
            },
            ["llen", key] => Request::Valid(Command::Llen(key)),
            ["lpop", key] => Request::Valid(Command::Lpop(key)),
            ["lpush", key, value] => Request::Valid(Command::Lpush(key, value)),
            ["lpushx", key, value] => Request::Valid(Command::Lpushx(key, value)),
            ["lrange", key, ini, end] => match ini.parse::<i32>() {
                Ok(ini) => match end.parse::<i32>() {
                    Ok(end) => Request::Valid(Command::Lrange(key, ini, end)),
                    Err(_) => Request::Invalid(RequestError::ParseError),
                },
                Err(_) => Request::Invalid(RequestError::ParseError),
            },
            ["lrem", key, count, value] => match count.parse::<i32>() {
                Ok(count) => Request::Valid(Command::Lrem(key, count, value)),
                Err(_) => Request::Invalid(RequestError::ParseError),
            },
            ["lset", key, index, value] => match index.parse::<usize>() {
                Ok(index) => Request::Valid(Command::Lset(key, index, value)),
                Err(_) => Request::Invalid(RequestError::ParseError),
            },
            ["rpop", key] => Request::Valid(Command::Rpop(key)),
            ["rpush", key, value] => Request::Valid(Command::Rpush(key, value)),
            ["rpushx", key, value] => Request::Valid(Command::Rpushx(key, value)),
            ["sadd", key, element] => Request::Valid(Command::Sadd(key, element)),
            ["sismember", key, element] => Request::Valid(Command::Sismember(key, element)),
            ["scard", key] => Request::Valid(Command::Scard(key)),
            _ => Request::Invalid(RequestError::InvalidCommand),
        }
    }

    pub fn invalid_request(request_error: RequestError) -> Request<'a> {
        Request::Invalid(request_error)
    }

    pub fn execute(self, db: &Arc<Mutex<Database>>) -> Reponse {
        match self {
            Request::Valid(command) => {
                let mut db = db.lock().unwrap();

                let result = match command {
                    Command::Append(key, value) => db.append(&key, value),
                    Command::Incrby(key, incr) => db.incrby(&key, incr),
                    Command::Decrby(key, decr) => db.decrby(&key, decr),
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
                    Command::Mset(vec_str) => db.mset(vec_str),
                    Command::Mget(vec_str) => db.mget(vec_str),
                    Command::Lindex(key, indx) => db.lindex(&key, indx),
                    Command::Llen(key) => db.llen(&key),
                    Command::Lpop(key) => db.lpop(&key),
                    Command::Lpush(key, value) => db.lpush(&key, value),
                    Command::Lpushx(key, value) => db.lpushx(&key, value),
                    Command::Lrange(key, ini, end) => db.lrange(&key, ini, end),
                    Command::Lrem(key, count, value) => db.lrem(&key, count, value),
                    Command::Lset(key, index, value) => db.lset(&key, index, &value),
                    Command::Rpop(key) => db.rpop(&key),
                    Command::Rpush(key, value) => db.rpush(&key, value),
                    Command::Rpushx(key, value) => db.rpushx(&key, value),
                    Command::Sadd(set_key, value) => db.sadd(&set_key, value),
                    Command::Sismember(set_key, value) => db.sismember(&set_key, value),
                    Command::Scard(set_key) => db.scard(&set_key),
                    Command::Flushdb() => db.flushdb(),
                    Command::Dbsize() => db.dbsize(),
                };

                match result {
                    Ok(value) => Reponse::Valid(value.to_string()),
                    Err(db_error) => Reponse::Error(db_error.to_string()),
                }
            }

            Request::Invalid(error) => Reponse::Error(error.to_string()),
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

pub fn parse_request(stream: &mut TcpStream) -> Result<String, RequestError> {
    let mut buf = [0; 512];

    match stream.read(&mut buf) {
        Ok(bytes_read) if bytes_read == 0 => Err(RequestError::NoInputError),
        Ok(bytes_read) => match std::str::from_utf8(&buf[..bytes_read]) {
            Ok(value) => Ok(value.trim().to_owned()),
            Err(_) => Err(RequestError::NotUtf8CharError),
        },
        Err(_) => Err(RequestError::InvalidCommand),
    }
}

impl<'a> Display for Command<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
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
            Command::Flushdb() => write!(f, "CommandServer::Flushdb"),
            Command::Dbsize() => write!(f, "CommandServer::Dbsize"),
            Command::None => write!(f, "Wrong Command"),
        }
    }
}

impl<'a> Display for Request<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Request::Valid(command) => writeln!(f, "Request: {}", command),
            Request::Invalid(error) => writeln!(f, "Request: Error: {}", error),
        }
    }
}

impl<'a> Display for Reponse {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Reponse::Valid(message) => writeln!(f, "Reponse: {}", message),
            Reponse::Error(error) => writeln!(f, "Reponse: Error: {}", error),
        }
    }
}

impl<'a> Display for RequestError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            RequestError::NoInputError => write!(f, "Empty request"),
            RequestError::NotUtf8CharError => write!(f, "Not utf8 string"),
            RequestError::ParseError => write!(f, "Couldn't Parse number input"),
            RequestError::InvalidCommand => write!(f, "Invalid Command"),
            RequestError::InvalidNumberOfArguments => write!(f, "Invalid Number of Arguments"),
        }
    }
}
