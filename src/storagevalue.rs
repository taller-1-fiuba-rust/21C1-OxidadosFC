use core::fmt::{Display, Formatter, Result};
use std::collections::HashSet;

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
