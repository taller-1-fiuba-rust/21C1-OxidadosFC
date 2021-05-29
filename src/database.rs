

use std::fmt::Formatter;
use std::fmt;
use crate::storagevalue::StorageValue;
use std::collections::HashMap;

pub struct Database {
    dictionary: HashMap<String, StorageValue>,
}

#[derive(Debug)]
pub enum MensajeErroresDataBase{
    ValueNotIsAnString(String),
    KeyNotExistsInDatabase(String),
    KeyValueIsNotString(String),
}

impl Database {
    pub fn new() -> Database {
        Database {
            dictionary: HashMap::new(),
        }
    }

    pub fn append(&mut self, key: &str, value: &str) -> Result<String, MensajeErroresDataBase>{
        let len_value = value.len();
        if self.dictionary.contains_key(key) {
            if let Some(StorageValue::String(val)) = self.dictionary.get_mut(key) {
                val.push_str(value);
                let len_result = val.len();
                Ok(format!("(integer) {}",len_result))
                //Ok(String::from("(integer) ".to_owned() + &len_result.to_string()))
            } else {
                Err(MensajeErroresDataBase::ValueNotIsAnString(String::from("value not is an String")))
            }
        } else {
            self.set(key, value).expect("error");
            Ok(format!("(integer) {}",len_value))
            //Ok(String::from("(integer) ".to_owned() + &value.to_string()))
        }
    }

    pub fn get(&mut self, key: &str) -> Result<String, MensajeErroresDataBase> {
        if let Some(StorageValue::String(val)) = self.dictionary.get(key) {
            Ok(val.to_string())
        }
        else{
            Err(MensajeErroresDataBase::KeyNotExistsInDatabase(String::from("(nil)")))
            //println!("{:?}", String::from("(nil)"));
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




    pub fn getdel(&mut self, key: &str) -> Result< String, MensajeErroresDataBase >{
        //self.get(key);
        if let Some(StorageValue::String(value)) = self.dictionary.remove(key){
            Ok(value.to_string())
        }
        else{
            Err(MensajeErroresDataBase::KeyNotExistsInDatabase(String::from("(nil)")))
        }

    }

    pub fn getset(&mut self, key: &str, _new_val: &str) -> Result <String, MensajeErroresDataBase >{
        let mut _copy_key = String::from(key.clone());
        if let Some(storage_value) = self.dictionary.get(&_copy_key) {
            if let StorageValue::String(old_value) = storage_value{
                //self.dictionary.insert(String::from(key), StorageValue::String(_new_val.to_string()));
                Ok(old_value.to_string())
            }
            else{
                Err(MensajeErroresDataBase::KeyValueIsNotString(String::from("key value is not an String")))
            }
        }
        else{
            Err(MensajeErroresDataBase::KeyNotExistsInDatabase(String::from("(nil)")))
            //println!("{:?}", String::from("(nil)"));
        }
    }

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
    use crate::database::MensajeErroresDataBase;
    use crate::database::Database;

    #[test]
    fn test01_append_new_key_return_lenght_of_the_value(){
        let mut database = Database::new();

        let result = database.append("key","value");

        assert_eq!(result.unwrap() , String::from("(integer) 5"));  
    }

    #[test]
    fn test02_append_key_with_valueold_return_lenght_of_the_total_value(){
        let mut database = Database::new();

        database.append("key","value").expect("falla en pasaje de parametros");

        let result = database.append("key", "_concat");

        assert_eq!(result.unwrap() , String::from("(integer) 12"));
    }

    #[test]
    fn test03_append_key_with_value_in_not_an_string_return_value_not_is_an_string_error(){
        //no se puede probar dado que append, recibe un string y deberia recibir un Tipo generico.
        /*let mut database = Database::new();
        let vector = vec![1;2;3];

        let result = database.append("key", );

        assert_eq!(result.unwrap() , String::from("(integer) 12"));
        */
    }

    #[test]
    fn test04_get_returns_value_of_key(){
        let mut database = Database::new();

        database.set("key","value").expect("error");

        let result = database.get("key");

        assert_eq!(result.unwrap() , String::from("value"));  
    }

    #[test]
    fn test05_get_returns_error_if_the_key_does_not_exist(){
        let mut database = Database::new();
        let result = database.get("key_no_exist");
        assert!(result.is_err());
        //assert_eq!(result.unwrap_err().to_string(), "(nil)")
    }

    #[test]
    fn test06_set_returns_ok(){
        let mut database = Database::new();
        let result = database.set("key", "1");
        assert_eq!(result.unwrap(), "OK".to_string());
    }

}
