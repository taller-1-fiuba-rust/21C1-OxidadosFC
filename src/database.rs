

use std::fmt::Formatter;
use std::fmt;
use crate::storagevalue::StorageValue;
use std::collections::HashMap;

pub struct Database {
    dictionary: HashMap<String, StorageValue>,
}

#[derive(Debug)]
pub enum MensajeErroresDataBase{
    ValueNotIsAnString,
    KeyNotExistsInDatabase,
}

impl Database {
    pub fn new() -> Database {
        Database {
            dictionary: HashMap::new(),
        }
    }

    pub fn append(&mut self, key: &str, value: &str) -> Result<String, MensajeErroresDataBase>{
        if self.dictionary.contains_key(key) {
            if let Some(StorageValue::String(val)) = self.dictionary.get_mut(key) {
                val.push_str(value);
                Ok(String::from("(integer) <len del string resultante>"))
            } else {
                Err(MensajeErroresDataBase::ValueNotIsAnString)
            }
        } else {
            self.set(key, value);
            Ok(String::from("(integer) <len del string resultante>"))
            //self.dictionary.insert(String::from(key), StorageValue::String(String::from(value)));
        }
    }

    pub fn get(&mut self, key: &str) -> Result<String, MensajeErroresDataBase> {
        if let Some(StorageValue::String(val)) = self.dictionary.get(key) {
            Ok(val.to_string())
            //println!("{:?}", val);
        }
        else{
            Err(MensajeErroresDataBase::KeyNotExistsInDatabase)
            //println!("{:?}", String::from("nil"));
        }
    }

    pub fn set(&mut self, key: &str, val: &str) -> Result<String, MensajeErroresDataBase>{
        self.dictionary.insert(String::from(key), StorageValue::String(val.to_string()));
        Ok(String::from("OK"))
    }

/*
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
    */



/*
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
*/
}

impl fmt::Display for Database {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        for (key, value) in self.dictionary.iter() {
            writeln!(f, "key: {}, value: {}", key, value)?;
        }

        Ok(())
    }
}


#[cfg(test)]
mod commandtest{

    #[test]
    fn test() {
        assert_eq!(1+2, 3);
    }
}
