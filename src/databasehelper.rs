use std::cmp::Ordering;
use core::fmt::{Display, Formatter, Result};
use std::collections::HashSet;
use std::fmt;
use std::time::SystemTime;

#[derive(Clone)]
pub enum StorageValue {
    String(String),
    List(Vec<String>),
    Set(HashSet<String>),
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

#[derive(Eq, Clone, Debug)]
pub struct TTlPair {
    pub key: String,
    pub death_time: SystemTime,
}

impl TTlPair {
    pub fn new(key: String, death_time: SystemTime) -> TTlPair{
        TTlPair {
            key,
            death_time,
        }
    }
}

impl PartialOrd for TTlPair {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}


impl Ord for TTlPair {
    fn cmp(&self, other: &Self) -> Ordering {
        self.death_time.cmp(&other.death_time)
    }
}


impl PartialEq for TTlPair {
    fn eq(&self, other: &Self) -> bool {
        self.death_time == other.death_time && self.key == other.key
    }
}