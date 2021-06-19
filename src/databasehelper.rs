use core::fmt::{Display, Formatter, Result};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt;
use std::sync::mpsc::Sender;
use std::time::SystemTime;

#[derive(Clone)]
pub enum StorageValue {
    String(String),
    List(Vec<String>),
    Set(HashSet<String>),
}

impl StorageValue {
    pub fn get_type(&self) -> String {
        match self {
            StorageValue::String(_) => "String".to_owned(),
            StorageValue::List(_) => "List".to_owned(),
            StorageValue::Set(_) => "Set".to_owned(),
        }
    }
}

impl Display for StorageValue {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            StorageValue::String(string) => write!(f, "{}", string),
            StorageValue::List(list) => {
                let mut parms_string = String::new();

                for elem in list {
                    parms_string.push_str(elem);
                    parms_string.push(' ');
                }

                write!(f, "{}", parms_string)
            }
            StorageValue::Set(hash_set) => {
                let mut hash_set_string = String::new();

                for elem in hash_set {
                    hash_set_string.push_str(elem);
                    hash_set_string.push(' ');
                }

                write!(f, "{}", hash_set_string)
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum SuccessQuery {
    Success,
    Boolean(bool),
    Integer(i32),
    String(String),
    List(Vec<SuccessQuery>),
    Nil,
}

impl<'a> fmt::Display for SuccessQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SuccessQuery::Success => write!(f, "Ok"),
            SuccessQuery::Boolean(boolean) => write!(f, "(integer) {}", *boolean as i32),
            SuccessQuery::Integer(val) => write!(f, "(integer) {}", val),
            SuccessQuery::String(val) => write!(f, "{}", val),
            SuccessQuery::List(list) => {
                if list.is_empty() {
                    write!(f, "(empty list or set)")
                } else {
                    let mut hash_set_string = String::new();

                    for elem in list {
                        hash_set_string.push_str(&elem.to_string());
                        hash_set_string.push(' ');
                    }

                    write!(f, "{}", hash_set_string)
                }
            }

            SuccessQuery::Nil => write!(f, "(Nil)"),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum DataBaseError {
    NotAString,
    NonExistentKey,
    NotAnInteger,
    KeyAlredyExist,
    NotASet,
    NotAList,
    IndexOutOfRange,
}

impl fmt::Display for DataBaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            DataBaseError::NonExistentKey => write!(f, "Non-existent key"),
            DataBaseError::NotAString => write!(f, "Value isn't a String"),
            DataBaseError::NotAnInteger => write!(f, "Value isn't an Integer"),
            DataBaseError::KeyAlredyExist => write!(f, "the key alredy exist in the database"),
            DataBaseError::NotASet => write!(f, "element of key isn't a Set"),
            DataBaseError::IndexOutOfRange => write!(f, "index out of range"),
            DataBaseError::NotAList => write!(f, "Value isn't a List"),
        }
    }
}

pub enum MessageTtl {
    Expire(KeyTtl),
    Clear(String),
    Transfer(String, String),
    TTL(String, Sender<RespondTtl>),
}

pub enum RespondTtl {
    TTL(SystemTime),
    Persistent,
}

#[derive(Eq, Clone, Debug)]
pub struct KeyTtl {
    pub key: String,
    pub expire_time: SystemTime,
}

impl KeyTtl {
    pub fn new(key: &str, expire_time: SystemTime) -> KeyTtl {
        KeyTtl {
            key: key.to_string(),
            expire_time,
        }
    }
}

impl PartialOrd for KeyTtl {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for KeyTtl {
    fn cmp(&self, other: &Self) -> Ordering {
        self.expire_time.cmp(&other.expire_time)
    }
}

impl PartialEq for KeyTtl {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}
