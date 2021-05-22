use crate::storagevalue::StorageValue;
use core::fmt::{Display, Formatter, Result};
use std::collections::HashMap;

pub struct Database {
    dictonary: HashMap<String, StorageValue>,
}

impl Database {
    pub fn new() -> Database {
        Database {
            dictonary: HashMap::new(),
        }
    }

    pub fn append(&mut self, key: &str, value: &str) {
        if self.dictonary.contains_key(key) {
            if let Some(StorageValue::String(val)) = self.dictonary.get_mut(key) {
                val.push_str(value);
            } else {
                panic!("Not a String");
            }
        } else {
            self.dictonary
                .insert(String::from(key), StorageValue::String(String::from(value)));
        }
    }
}

impl Display for Database {
    fn fmt(&self, f: &mut Formatter) -> Result {
        for (key, value) in self.dictonary.iter() {
            write!(f, "key: {}, value: {}\n", key, value);
        }

        Ok(())
    }
}
