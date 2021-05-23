use crate::storagevalue::StorageValue;
use core::fmt::{Display, Formatter, Result};
use std::collections::HashMap;

pub struct Database {
    dictionary: HashMap<String, StorageValue>,
}

impl Database {
    pub fn new() -> Database {
        Database {
            dictionary: HashMap::new(),
        }
    }

    pub fn append(&mut self, key: &str, value: &str) {
        if self.dictionary.contains_key(key) {
            if let Some(StorageValue::String(val)) = self.dictionary.get_mut(key) {
                val.push_str(value);
            } else {
                panic!("Not a String");
            }
        } else {
            self.set(key, value);
            //self.dictionary.insert(String::from(key), StorageValue::String(String::from(value)));
        }
    }

    // deberia retornar algun mensaje de Ok segun el protocolo Redis, o un Mensaje De error, para que haga un write el server.
    pub fn decrby(&mut self, key: &str, number_of_decr: &str) {
        let number_decr = number_of_decr.parse::<i64>();
        if let Some(StorageValue::String(val)) = self.dictionary.get_mut(key) {
            if let Ok(mut number) = val.parse::<i64>() {
                number -= number_decr.unwrap();
                //aqui deberia lllamarse a set del database.
                self.set(key, &number.to_string());
            }
        }
    }

    pub fn incrby(&mut self, key: &str, number_of_incr: &str) {
        let mut number_incr = String::from(number_of_incr);
        number_incr.insert(0, '-');
        self.decrby(key, &number_incr);
    }

    pub fn get(&mut self, key: &str) {
        if let Some(StorageValue::String(val)) = self.dictionary.get(key) {
            println!("{:?}", val);
        } else if let Some(StorageValue::None) = self.dictionary.get(key) {
            println!("{}", StorageValue::None);
        } else {
            println!("{:?}", String::from("nil"));
        }
    }

    pub fn getdel(&mut self, key: &str) {
        self.get(key);
        self.dictionary
            .insert(String::from(key), StorageValue::None);
    }

    // enunciado dice operacion atomica. como podria implementarse atomicamente?
    pub fn getset(&mut self, key: &str, val: &str) {
        self.get(key);
        self.set(key, val);
    }

    pub fn set(&mut self, key: &str, val: &str) {
        self.dictionary
            .insert(String::from(key), StorageValue::String(val.to_string()));
    }
}
impl Display for Database {
    fn fmt(&self, f: &mut Formatter) -> Result {
        for (key, value) in self.dictionary.iter() {
            writeln!(f, "key: {}, value: {}", key, value)?;
        }

        Ok(())
    }
}
