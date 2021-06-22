use crate::channels::Channels;
use crate::database::Database;
use crate::databasehelper::SuccessQuery;
use crate::server_conf::ServerConf;
use core::fmt::{self, Display, Formatter};
use std::io::{Read, Write};
use std::net::TcpStream;

pub enum Request<'a> {
    DataBase(Query<'a>),
    Server(ServerRequest<'a>),
    Suscriber(SuscriberRequest<'a>),
    Invalid(&'a str, RequestError),
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

pub enum SuscriberRequest<'a> {
    Monitor,
    Subscribe(Vec<&'a str>),
    Publish(&'a str, &'a str)
}

impl<'a> SuscriberRequest<'a> {
    pub fn execute(
        self,
        stream: &mut TcpStream,
        channels: &mut Channels,
    ) -> Reponse {
        match self {
            Self::Monitor => {
                let r =channels.add_monitor();

                for msg in r.iter() {
                    let respons = Reponse::Valid(msg);
                    respons.respond(stream);
                }

                // for msg in r.iter() {
                //     let msg = "Client: ".to_string() + &msg.0 + " " + &msg.1;
                //     let respons = Reponse::Valid(msg);
                //     respons.respond(stream);

                Reponse::Valid("Ok".to_string())
            },
            Self::Subscribe(suscriptions) => {
                let (r, subscriptions) = channels.subscribe(suscriptions);
                writeln!(stream, "{}", subscriptions).unwrap();

                for msg in r.iter() {
                    let msg = SuccessQuery::List(
                        vec![
                            SuccessQuery::String("message".to_string()),
                            SuccessQuery::String(msg.to_string()),
                        ]
                    ).to_string();
                    let respons = Reponse::Valid(msg);
                    respons.respond(stream);
                }

                Reponse::Valid("Ok".to_string())
            },
            Self::Publish(chanel, msg) => {
                let message = SuccessQuery::List(
                    vec![
                        SuccessQuery::String(chanel.to_string()),
                        SuccessQuery::String(msg.to_string()),
                    ]
                );
                let subscribers = channels.send(chanel, &message.to_string());

                Reponse::Valid(subscribers.to_string())
            }
        }
    }
}

pub enum Query<'a> {
    Flushdb(),
    Dbsize(),
    Expire(&'a str, i64),
    ExpireAt(&'a str, i64),
    Persist(&'a str),
    TTL(&'a str),
    TYPE(&'a str),
    Get(&'a str),
    Copy(&'a str, &'a str),
    Del(&'a str),
    Exists(&'a str),
    Keys(&'a str),
    Rename(&'a str, &'a str),
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
    pub fn new(request_str: &str) -> Request {
        let request: Vec<&str> = request_str.split_whitespace().collect();

        match request[..] {
            ["expire", key, seconds] => match seconds.parse::<i64>() {
                Ok(seconds) => Request::DataBase(Query::Expire(key, seconds)),
                Err(_) => Request::Invalid(request_str, RequestError::ParseError),
            },
            ["expireat", key, seconds] => match seconds.parse::<i64>() {
                Ok(seconds) => Request::DataBase(Query::ExpireAt(key, seconds)),
                Err(_) => Request::Invalid(request_str, RequestError::ParseError),
            },
            ["ttl", key] => Request::DataBase(Query::TTL(key)),
            ["type", key] => Request::DataBase(Query::TYPE(key)),
            ["persist", key] => Request::DataBase(Query::Persist(key)),
            ["append", key, value] => Request::DataBase(Query::Append(key, value)),
            ["incrby", key, incr] => match incr.parse::<i32>() {
                Ok(incr) => Request::DataBase(Query::Incrby(key, incr)),
                Err(_) => Request::Invalid(request_str, RequestError::ParseError),
            },
            ["decrby", key, decr] => match decr.parse::<i32>() {
                Ok(decr) => Request::DataBase(Query::Decrby(key, decr)),
                Err(_) => Request::Invalid(request_str, RequestError::ParseError),
            },
            ["get", key] => Request::DataBase(Query::Get(key)),
            ["getdel", key] => Request::DataBase(Query::Getdel(key)),
            ["getset", key, value] => Request::DataBase(Query::Getset(key, value)),
            ["set", key, value] => Request::DataBase(Query::Set(key, value)),
            ["copy", key, to_key] => Request::DataBase(Query::Copy(key, to_key)),
            ["del", key] => Request::DataBase(Query::Del(key)),
            ["exists", key] => Request::DataBase(Query::Exists(key)),
            ["keys", pattern] => Request::DataBase(Query::Keys(pattern)),
            ["rename", old_key, new_key] => Request::DataBase(Query::Rename(old_key, new_key)),
            ["strlen", key] => Request::DataBase(Query::Strlen(key)),
            ["mset", ..] => {
                let tail = &request[1..];
                let len = tail.len() as i32;

                if len > 0 && (len % 2 == 0) {
                    Request::DataBase(Query::Mset(tail.to_vec()))
                } else {
                    Request::Invalid(request_str, RequestError::InvalidNumberOfArguments)
                }
            }
            ["mget", ..] => {
                let tail = &request[1..];
                Request::DataBase(Query::Mget(tail.to_vec()))
            }
            ["lindex", key, index] => match index.parse::<i32>() {
                Ok(index) => Request::DataBase(Query::Lindex(key, index)),
                Err(_) => Request::Invalid(request_str, RequestError::ParseError),
            },
            ["llen", key] => Request::DataBase(Query::Llen(key)),
            ["lpop", key] => Request::DataBase(Query::Lpop(key)),
            ["lpush", key, value] => Request::DataBase(Query::Lpush(key, value)),
            ["lpushx", key, value] => Request::DataBase(Query::Lpushx(key, value)),
            ["lrange", key, ini, end] => match ini.parse::<i32>() {
                Ok(ini) => match end.parse::<i32>() {
                    Ok(end) => Request::DataBase(Query::Lrange(key, ini, end)),
                    Err(_) => Request::Invalid(request_str, RequestError::ParseError),
                },
                Err(_) => Request::Invalid(request_str, RequestError::ParseError),
            },
            ["lrem", key, count, value] => match count.parse::<i32>() {
                Ok(count) => Request::DataBase(Query::Lrem(key, count, value)),
                Err(_) => Request::Invalid(request_str, RequestError::ParseError),
            },
            ["lset", key, index, value] => match index.parse::<usize>() {
                Ok(index) => Request::DataBase(Query::Lset(key, index, value)),
                Err(_) => Request::Invalid(request_str, RequestError::ParseError),
            },
            ["rpop", key] => Request::DataBase(Query::Rpop(key)),
            ["rpush", key, value] => Request::DataBase(Query::Rpush(key, value)),
            ["rpushx", key, value] => Request::DataBase(Query::Rpushx(key, value)),
            ["sadd", key, element] => Request::DataBase(Query::Sadd(key, element)),
            ["sismember", key, element] => Request::DataBase(Query::Sismember(key, element)),
            ["scard", key] => Request::DataBase(Query::Scard(key)),
            ["flushdb"] => Request::DataBase(Query::Flushdb()),
            ["dbsize"] => Request::DataBase(Query::Dbsize()),
            ["config", "get", pattern] => Request::Server(ServerRequest::ConfigGet(pattern)),
            ["config", "set", option, new_value] => {
                Request::Server(ServerRequest::ConfigSet(option, new_value))
            }
            ["smembers", key] => Request::DataBase(Query::Smembers(key)),
            ["srem", key, ..] => {
                let tail = &request[1..];
                Request::DataBase(Query::Srem(key, tail.to_vec()))
            }
            ["monitor"] => Request::Suscriber(SuscriberRequest::Monitor),
            ["subscribe", ..] => {
                let tail = &request[1..];
                Request::Suscriber(SuscriberRequest::Subscribe(tail.to_vec()))
            }
            ["publish", chanel, msg] => Request::Suscriber(SuscriberRequest::Publish(chanel, msg)),
            _ => Request::Invalid(request_str, RequestError::UnknownRequest),
        }
    }
}

pub fn parse_request(stream: &mut TcpStream) -> Option<String> {
    let mut buf = [0; 512];

    match stream.read(&mut buf) {
        Ok(bytes_read) => match std::str::from_utf8(&buf[..bytes_read]) {
            Ok(value) if value.trim().len() > 0 => Some(value.trim().to_owned()),
            _ => None,
        },
        Err(_) => None,
    }
}

impl<'a> Display for Query<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Query::Expire(key, seconds) => {
                write!(f, "Expire - Key: {} - Seconds: {}", key, seconds)
            }
            Query::ExpireAt(key, seconds) => {
                write!(f, "ExpireAt - Key: {} - Seconds: {}", key, seconds)
            }
            Query::Persist(key) => write!(f, "Persist - Key: {}", key),
            Query::TYPE(key) => write!(f, "Type - Key: {}", key),
            Query::TTL(key) => write!(f, "TTL - Key: {}", key),
            Query::Append(key, value) => {
                write!(f, "Append - Key: {} - Value: {} ", key, value)
            }
            Query::Incrby(key, incr) => write!(f, "Incrby - Key: {} - Increment: {}", key, incr),
            Query::Decrby(key, incr) => write!(f, "Decrby - Key: {} - Increment: {}", key, incr),
            Query::Get(key) => write!(f, "Get - Key: {}", key),
            Query::Getdel(key) => write!(f, "Getdel - Key: {}", key),
            Query::Getset(key, value) => {
                write!(f, "Getset - Key: {} - Value: {}", key, value)
            }

            Query::Mget(params) => {
                let mut parms_string = String::new();

                for elem in params {
                    parms_string.push_str(elem);
                    parms_string.push(' ');
                }

                write!(f, "Mget Keys: {}", parms_string)
            }
            Query::Mset(params) => {
                let mut parms_string = String::new();

                for elem in params {
                    parms_string.push_str(elem);
                    parms_string.push(' ');
                }
                write!(f, "Mset pair: {}", parms_string)
            }
            Query::Strlen(key) => write!(f, "Strlen - Key: {}", key),
            Query::Set(key, value) => {
                write!(f, "Set - Key: {} - Value: {}", key, value)
            }
            Query::Copy(key, to_key) => {
                write!(f, "Copy - Key: {} - To_Key: {}", key, to_key)
            }
            Query::Del(key) => write!(f, "Del - Key: {}", key),
            Query::Exists(key) => write!(f, "Exists - Key: {}", key),
            Query::Keys(pattern) => write!(f, "Keys - Pattern: {}", pattern),
            Query::Rename(old_key, new_key) => {
                write!(f, "Rename - Old_Key {} - New_Key {}", old_key, new_key)
            }
            Query::Lindex(key, indx) => {
                write!(f, "Lindex - Key: {} - Index: {}", key, indx)
            }
            Query::Llen(key) => write!(f, "Llen - Key {}", key),
            Query::Lpop(key) => write!(f, "Lpop - Key {}", key),
            Query::Lpush(key, value) => {
                write!(f, "Lpush - Key: {} - Value: {}", key, value)
            }
            Query::Lpushx(key, value) => {
                write!(f, "Lpushx - Key: {} - Value: {}", key, value)
            }
            Query::Lrange(key, beg, end) => {
                write!(f, "Lrange - Key: {} - Begining: {} - End {}", key, beg, end)
            }
            Query::Lrem(key, rem, value) => {
                write!(f, "Lrem - Key: {} - Rem: {} - Value: {}", key, rem, value)
            }
            Query::Lset(key, index, value) => write!(
                f,
                "Lset - Key: {} - Index: {} - Value: {}",
                key, index, value
            ),
            Query::Rpop(key) => write!(f, "Rpop - Key: {}", key),
            Query::Rpush(key, value) => {
                write!(f, "Rpush - Key: {} - Value: {} ", key, value)
            }
            Query::Rpushx(key, value) => {
                write!(f, "Rpushx - Key: {} - Value: {} ", key, value)
            }
            Query::Sadd(key, element) => {
                write!(f, "Sadd - Key: {} - Element: {}", key, element)
            }
            Query::Sismember(key, element) => {
                write!(f, "Sismember - Key: {} - Element: {}", key, element)
            }
            Query::Scard(key) => write!(f, "Sismember - Key: {}", key),
            Query::Flushdb() => write!(f, "Flushdb"),
            Query::Dbsize() => write!(f, "Dbsize"),
            Query::Smembers(key) => write!(f, "Smembers - Key: {}", key),
            Query::Srem(key, vec_str) => {
                let mut members_str = String::new();

                for member in vec_str {
                    members_str.push_str(member);
                    members_str.push(' ');
                }
                write!(f, "Srem - Key: {} - members: {}", key, members_str)
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
                if writeln!(stream, "{}", message).is_err() {
                    println!("Error");
                }
            }
            Reponse::Error(message) => {
                if writeln!(stream, "Error: {}", message).is_err() {
                    println!("Error");
                }
            }
        };
    }
}

impl<'a> Display for Request<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Request::DataBase(query) => write!(f, "{}", query),
            Request::Server(server_request) => write!(f, "{}", server_request),
            Request::Invalid(request_str, error) => write!(f, "{} On: {}", error, request_str),
            Request::Suscriber(sus_request) => write!(f, "{}", sus_request),
        }
    }
}

impl<'a> Display for Reponse {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Reponse::Valid(message) => write!(f, "{}", message),
            Reponse::Error(error) => write!(f, "Error: {}", error),
        }
    }
}

impl<'a> Display for SuscriberRequest<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            SuscriberRequest::Monitor => write!(f, "Monitor"),
            SuscriberRequest::Subscribe(suscriptions) => {
                let mut suscriptions_string = String::new();

                for elem in suscriptions {
                    suscriptions_string.push_str(elem);
                    suscriptions_string.push(' ');
                }

                write!(f, "Subscribe channels: {}", suscriptions_string)
            },
            SuscriberRequest::Publish(chanel, msg) => writeln!(f, "publish - channel: {} - message: {}", chanel, msg),
        }
    }
}

impl<'a> Display for RequestError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            RequestError::ParseError => write!(f, "Couldn't Parse number input"),
            RequestError::InvalidNumberOfArguments => write!(f, "Invalid Number of Arguments"),
            RequestError::UnknownRequest => write!(f, "Non existent Request"),
        }
    }
}
