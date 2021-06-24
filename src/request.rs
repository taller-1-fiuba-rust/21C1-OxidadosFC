use crate::database::Database;
use crate::server_conf::ServerConf;
use core::fmt::{self, Display, Formatter};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};

pub enum Request<'a> {
    DataBase(Query<'a>),
    Server(ServerRequest<'a>),
    Suscriber(SuscriberRequest),
    Invalid(RequestError),
}

pub enum RequestError {
    ParseError,
    UnknownRequest,
    InvalidNumberOfArguments,
}

pub enum ServerRequest<'a> {
    ConfigGet(&'a str),
    ConfigSet(&'a str, &'a str),
}

pub enum SuscriberRequest {
    Monitor,
}

impl SuscriberRequest {
    pub fn execute(
        self,
        stream: &mut TcpStream,
        chanels: &mut Arc<Mutex<HashMap<String, Vec<Sender<String>>>>>,
    ) -> Reponse {
        match self {
            Self::Monitor => {
                let (s, r) = channel();

                let mut guard = chanels.lock().unwrap();

                let list = guard.get_mut("Monitor").unwrap();

                list.push(s);

                drop(guard);

                for msg in r.iter() {
                    let respons = Reponse::Valid(msg);
                    respons.respond(stream);
                }

                Reponse::Valid("Ok".to_string())
            }
        }
    }
}

pub enum Query<'a> {
    Flushdb(),
    Dbsize(),
    Copy(&'a str, &'a str),
    Del(&'a str),
    Exists(&'a str),
    Expire(&'a str, i64),
    ExpireAt(&'a str, i64),
    Keys(&'a str),
    Persist(&'a str),
    Rename(&'a str, &'a str),
    Sort(&'a str, i32, i32, i32, i32),
    TTL(&'a str),
    TYPE(&'a str),
    Get(&'a str),
    Append(&'a str, &'a str),
    Incrby(&'a str, i32),
    Decrby(&'a str, i32),
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
    Smembers(&'a str),
    Srem(&'a str, Vec<&'a str>),
}

impl<'a> Request<'a> {
    pub fn new(request: &str) -> Request {
        let request: Vec<&str> = request.split_whitespace().collect();

        match request[..] {
            ["monitor"] => Request::Suscriber(SuscriberRequest::Monitor),
            ["flushdb"] => Request::DataBase(Query::Flushdb()),
            ["config", "get", pattern] => Request::Server(ServerRequest::ConfigGet(pattern)),
            ["config", "set", option, new_value] => {
                Request::Server(ServerRequest::ConfigSet(option, new_value))
            },
            ["dbsize"] => Request::DataBase(Query::Dbsize()),
            ["copy", key, to_key] => Request::DataBase(Query::Copy(key, to_key)),
            ["del", key] => Request::DataBase(Query::Del(key)),
            ["exists", key] => Request::DataBase(Query::Exists(key)),
            ["expire", key, seconds] => match seconds.parse::<i64>() {
                Ok(seconds) => Request::DataBase(Query::Expire(key, seconds)),
                Err(_) => Request::Invalid(RequestError::ParseError),
            },
            ["expireat", key, seconds] => match seconds.parse::<i64>() {
                Ok(seconds) => Request::DataBase(Query::ExpireAt(key, seconds)),
                Err(_) => Request::Invalid(RequestError::ParseError),
            },
            ["keys", pattern] => Request::DataBase(Query::Keys(pattern)),
            ["persist", key] => Request::DataBase(Query::Persist(key)),
            ["rename", old_key, new_key] => Request::DataBase(Query::Rename(old_key, new_key)),
            ["sort", key, ..] => {
                let tail = &request[2..];
                let mut pos_begin_unwrap = 0;
                let mut num_elem_unwrap = 0;
                let mut alpha = 0;
                let mut desc = 0;
                let _ = if let ["alpha",..] = tail[..]{
                    alpha = 1;
                    println!("entre alpha");
                };
                let _ = if let ["desc",..] = tail[..]{
                    desc = 1;
                    println!("entre desc");
                };
                let _ = if let ["limit", pos_begin, num_elems,..] = tail[..]{
                    println!("entre limit");
                    match pos_begin.parse::<i32>(){
                            Ok(begin) => {pos_begin_unwrap = begin;},
                            Err(_) => {Request::Invalid(RequestError::ParseError);}
                    };
                    match num_elems.parse::<i32>(){
                        Ok(num_elems) => {num_elem_unwrap = num_elems;},
                        Err(_) => {Request::Invalid(RequestError::ParseError);}
                    };
                };
                println!("alpha: {} limit{} {} desc:{}", alpha, pos_begin_unwrap, num_elem_unwrap, desc);
                //_ => {Request::Invalid(RequestError::InvalidNumberOfArguments);}
                Request::DataBase(Query::Sort(key, pos_begin_unwrap, num_elem_unwrap, alpha, desc))
            },
            ["ttl", key] => Request::DataBase(Query::TTL(key)),
            ["type", key] => Request::DataBase(Query::TYPE(key)),
            ["append", key, value] => Request::DataBase(Query::Append(key, value)),
            ["decrby", key, decr] => match decr.parse::<i32>() {
                Ok(decr) => Request::DataBase(Query::Decrby(key, decr)),
                Err(_) => Request::Invalid(RequestError::ParseError),
            },
            ["incrby", key, incr] => match incr.parse::<i32>() {
                Ok(incr) => Request::DataBase(Query::Incrby(key, incr)),
                Err(_) => Request::Invalid(RequestError::ParseError),
            },
            ["get", key] => Request::DataBase(Query::Get(key)),
            ["getdel", key] => Request::DataBase(Query::Getdel(key)),
            ["getset", key, value] => Request::DataBase(Query::Getset(key, value)),
            ["set", key, value] => Request::DataBase(Query::Set(key, value)),
            ["strlen", key] => Request::DataBase(Query::Strlen(key)),
            ["mset", ..] => {
                let tail = &request[1..];
                let len = tail.len() as i32;

                if len > 0 && (len % 2 == 0) {
                    Request::DataBase(Query::Mset(tail.to_vec()))
                } else {
                    Request::Invalid(RequestError::InvalidNumberOfArguments)
                }
            }
            ["mget", ..] => {
                let tail = &request[1..];
                Request::DataBase(Query::Mget(tail.to_vec()))
            }
            ["lindex", key, index] => match index.parse::<i32>() {
                Ok(index) => Request::DataBase(Query::Lindex(key, index)),
                Err(_) => Request::Invalid(RequestError::ParseError),
            },
            ["llen", key] => Request::DataBase(Query::Llen(key)),
            ["lpop", key] => Request::DataBase(Query::Lpop(key)),
            ["lpush", key, value] => Request::DataBase(Query::Lpush(key, value)),
            ["lpushx", key, value] => Request::DataBase(Query::Lpushx(key, value)),
            ["lrange", key, ini, end] => match ini.parse::<i32>() {
                Ok(ini) => match end.parse::<i32>() {
                    Ok(end) => Request::DataBase(Query::Lrange(key, ini, end)),
                    Err(_) => Request::Invalid(RequestError::ParseError),
                },
                Err(_) => Request::Invalid(RequestError::ParseError),
            },
            ["lrem", key, count, value] => match count.parse::<i32>() {
                Ok(count) => Request::DataBase(Query::Lrem(key, count, value)),
                Err(_) => Request::Invalid(RequestError::ParseError),
            },
            ["lset", key, index, value] => match index.parse::<usize>() {
                Ok(index) => Request::DataBase(Query::Lset(key, index, value)),
                Err(_) => Request::Invalid(RequestError::ParseError),
            },
            ["rpop", key] => Request::DataBase(Query::Rpop(key)),
            ["rpush", key, value] => Request::DataBase(Query::Rpush(key, value)),
            ["rpushx", key, value] => Request::DataBase(Query::Rpushx(key, value)),
            ["sadd", key, element] => Request::DataBase(Query::Sadd(key, element)),
            ["sismember", key, element] => Request::DataBase(Query::Sismember(key, element)),
            ["scard", key] => Request::DataBase(Query::Scard(key)),
            ["smembers", key] => Request::DataBase(Query::Smembers(key)),
            ["srem", key, ..] => {
                let tail = &request[1..];
                Request::DataBase(Query::Srem(key, tail.to_vec()))
            },
            _ => Request::Invalid(RequestError::UnknownRequest),
        }
    }
}

pub fn parse_request(stream: &mut TcpStream) -> String {
    let mut buf = [0; 512];

    match stream.read(&mut buf) {
        Ok(bytes_read) => match std::str::from_utf8(&buf[..bytes_read]) {
            Ok(value) => value.trim().to_owned(),
            Err(_) => "".to_string(),
        },
        Err(_) => "".to_string(),
    }
}

impl<'a> Display for Query<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Query::Expire(key, seconds) => {
                write!(f, "QueryKeys::Expire - Key: {} - Seconds: {}", key, seconds)
            }
            Query::ExpireAt(key, seconds) => write!(
                f,
                "QueryKeys::ExpireAt - Key: {} - Seconds: {}",
                key, seconds
            ),
            Query::Persist(key) => write!(f, "QueryKeys::Persist - Key: {}", key),
            Query::TYPE(key) => write!(f, "QueryKeys::Type - Key: {}", key),
            Query::TTL(key) => write!(f, "QueryKeys::TTL - Key: {}", key),
            Query::Append(key, value) => {
                write!(f, "QueryKeys::Append - Key: {} - Value: {} ", key, value)
            }
            Query::Incrby(key, incr) => write!(
                f,
                "QueryString::Incrby - Key: {} - Increment: {}",
                key, incr
            ),
            Query::Decrby(key, incr) => write!(
                f,
                "QueryString::Decrby - Key: {} - Increment: {}",
                key, incr
            ),
            Query::Get(key) => write!(f, "QueryString::Get - Key: {}", key),
            Query::Getdel(key) => write!(f, "QueryString::Getdel - Key: {}", key),
            Query::Getset(key, value) => {
                write!(f, "QueryString::Getset - Key: {} - Value: {}", key, value)
            }

            Query::Mget(params) => {
                let mut parms_string = String::new();

                for elem in params {
                    parms_string.push_str(elem);
                    parms_string.push(' ');
                }

                write!(f, "QueryString::Mget Keys: {}", parms_string)
            }
            Query::Mset(params) => {
                let mut parms_string = String::new();

                for elem in params {
                    parms_string.push_str(elem);
                    parms_string.push(' ');
                }
                write!(f, "QueryString::Mset pair: {}", parms_string)
            }
            Query::Strlen(key) => write!(f, "QueryString::Strlen - Key: {}", key),
            Query::Set(key, value) => {
                write!(f, "QueryString::Set - Key: {} - Value: {}", key, value)
            }
            Query::Copy(key, to_key) => {
                write!(f, "QueryKeys::Copy - Key: {} - To_Key: {}", key, to_key)
            }
            Query::Del(key) => write!(f, "QueryKeys::Del - Key: {}", key),
            Query::Exists(key) => write!(f, "QueryKeys::Exists - Key: {}", key),
            Query::Keys(pattern) => write!(f, "QueryKeys::Keys - Pattern: {}", pattern),
            Query::Rename(old_key, new_key) => write!(
                f,
                "QueryKeys::Rename - Old_Key {} - New_Key {}",
                old_key, new_key
            ),
            Query::Sort(key, pos_begin, num_elems, alpha, asc_desc) => {
                write!(f, "QueryKeys::Sort - Key: {} - Limit {} {} - Alpha {} - ASC {}", key, pos_begin, num_elems, alpha , asc_desc)
            }
            Query::Lindex(key, indx) => {
                write!(f, "QueryList::Lindex - Key: {} - Index: {}", key, indx)
            }
            Query::Llen(key) => write!(f, "QueryList::Llen - Key {}", key),
            Query::Lpop(key) => write!(f, "QueryList::Lpop - Key {}", key),
            Query::Lpush(key, value) => {
                write!(f, "QueryList::Lpush - Key: {} - Value: {}", key, value)
            }
            Query::Lpushx(key, value) => {
                write!(f, "QueryList::Lpushx - Key: {} - Value: {}", key, value)
            }
            Query::Lrange(key, beg, end) => write!(
                f,
                "QueryList::Lrange - Key: {} - Begining: {} - End {}",
                key, beg, end
            ),
            Query::Lrem(key, rem, value) => write!(
                f,
                "QueryList::Lrem - Key: {} - Rem: {} - Value: {}",
                key, rem, value
            ),
            Query::Lset(key, index, value) => write!(
                f,
                "QueryList::Lset - Key: {} - Index: {} - Value: {}",
                key, index, value
            ),
            Query::Rpop(key) => write!(f, "QueryList::Rpop - Key: {}", key),
            Query::Rpush(key, value) => {
                write!(f, "QueryList::Rpush - Key: {} - Value: {} ", key, value)
            }
            Query::Rpushx(key, value) => {
                write!(f, "QueryList::Rpushx - Key: {} - Value: {} ", key, value)
            }
            Query::Sadd(key, element) => {
                write!(f, "QuerySet::Sadd - Key: {} - Element: {}", key, element)
            }
            Query::Sismember(key, element) => write!(
                f,
                "QuerySet::Sismember - Key: {} - Element: {}",
                key, element
            ),
            Query::Scard(key) => write!(f, "QuerySet::Sismember - Key: {}", key),
            Query::Flushdb() => write!(f, "QueryServer::Flushdb"),
            Query::Dbsize() => write!(f, "QueryServer::Dbsize"),
            Query::Smembers(key) => write!(f, "QuerySet::Smembers - Key: {}", key),
            Query::Srem(key, vec_str) => {
                let mut members_str = String::new();

                for member in vec_str {
                    members_str.push_str(member);
                    members_str.push(' ');
                }
                write!(
                    f,
                    "QuerySet::Srem - Key: {} - members: {}",
                    key, members_str
                )
            }
        }
    }
}

impl<'a> ServerRequest<'a> {
    pub fn exec_request(self, conf: &mut ServerConf) -> Reponse {
        let result = match self {
            ServerRequest::ConfigGet(option) => conf.get_config(option),
            ServerRequest::ConfigSet(option, value) => conf.set_config(option, value),
        };

        match result {
            Ok(succes) => Reponse::Valid(succes.to_string()),
            Err(err) => Reponse::Error(err.to_string()),
        }
    }
}

impl<'a> Query<'a> {
    pub fn exec_query(self, db: &mut Database) -> Reponse {
        let result = match self {
            Query::ExpireAt(key, seconds) => db.expireat(&key, seconds),
            Query::Expire(key, seconds) => db.expire(&key, seconds),
            Query::Persist(key) => db.persist(&key),
            Query::TTL(key) => db.ttl(&key),
            Query::TYPE(key) => db.get_type(&key),
            Query::Append(key, value) => db.append(&key, value),
            Query::Incrby(key, incr) => db.incrby(&key, incr),
            Query::Decrby(key, decr) => db.decrby(&key, decr),
            Query::Get(key) => db.get(&key),
            Query::Getdel(key) => db.getdel(&key),
            Query::Getset(key, value) => db.getset(&key, value),
            Query::Set(key, value) => db.set(&key, value),
            Query::Copy(key, to_key) => db.copy(&key, &to_key),
            Query::Del(key) => db.del(&key),
            Query::Exists(key) => db.exists(&key),
            Query::Keys(pattern) => db.keys(pattern),
            Query::Rename(old_key, new_key) => db.rename(&old_key, &new_key),
            Query::Sort(key, pos_begin_unwrap, num_elem_unwrap, alpha, desc) => {
                db.sort(&key, &pos_begin_unwrap, &num_elem_unwrap, &alpha, &desc)
            }
            Query::Strlen(key) => db.strlen(&key),
            Query::Mset(vec_str) => db.mset(vec_str),
            Query::Mget(vec_str) => db.mget(vec_str),
            Query::Lindex(key, indx) => db.lindex(&key, indx),
            Query::Llen(key) => db.llen(&key),
            Query::Lpop(key) => db.lpop(&key),
            Query::Lpush(key, value) => db.lpush(&key, value),
            Query::Lpushx(key, value) => db.lpushx(&key, value),
            Query::Lrange(key, ini, end) => db.lrange(&key, ini, end),
            Query::Lrem(key, count, value) => db.lrem(&key, count, value),
            Query::Lset(key, index, value) => db.lset(&key, index, &value),
            Query::Rpop(key) => db.rpop(&key),
            Query::Rpush(key, value) => db.rpush(&key, value),
            Query::Rpushx(key, value) => db.rpushx(&key, value),
            Query::Sadd(set_key, value) => db.sadd(&set_key, value),
            Query::Sismember(set_key, value) => db.sismember(&set_key, value),
            Query::Scard(set_key) => db.scard(&set_key),
            Query::Flushdb() => db.flushdb(),
            Query::Dbsize() => db.dbsize(),
            Query::Smembers(key) => db.smembers(&key),
            Query::Srem(key, vec_str) => db.srem(&key, vec_str),
        };

        match result {
            Ok(succes) => Reponse::Valid(succes.to_string()),
            Err(err) => Reponse::Error(err.to_string()),
        }
    }
}

pub enum Reponse {
    Valid(String),
    Error(String),
}

impl<'a> Display for ServerRequest<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            ServerRequest::ConfigGet(pattern) => {
                write!(f, "ServerRequest::ConfigGet - Pattern: {}", pattern)
            }
            ServerRequest::ConfigSet(option, new_value) => write!(
                f,
                "ServerRequest::ConfigSet - Option: {} - NewValue: {}",
                option, new_value
            ),
        }
    }
}

impl Reponse {
    pub fn respond(self, stream: &mut TcpStream) {
        match self {
            Reponse::Valid(message) => {
                if writeln!(stream, "{}\n", message).is_err() {
                    println!("Error");
                }
            }
            Reponse::Error(message) => {
                if writeln!(stream, "Error: {}\n", message).is_err() {
                    println!("Error");
                }
            }
        };
    }
}

impl<'a> Display for Request<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Request::DataBase(query) => writeln!(f, "Request: {}", query),
            Request::Server(server_request) => writeln!(f, "Request: {}", server_request),
            Request::Invalid(error) => writeln!(f, "Request: Error: {}", error),
            Request::Suscriber(sus_request) => writeln!(f, "Request: {}", sus_request),
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

impl Display for SuscriberRequest {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            SuscriberRequest::Monitor => writeln!(f, "Reponse: Monitor"),
        }
    }
}

impl<'a> Display for RequestError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            RequestError::ParseError => write!(f, "Couldn't Parse number input"),
            RequestError::InvalidNumberOfArguments => write!(f, "Invalid Number of Arguments"),
            RequestError::UnknownRequest => write!(f, "Unknown Request"),
        }
    }
}
