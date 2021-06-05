use crate::storagevalue::StorageValue;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Formatter;

pub struct Database {
    dictionary: HashMap<String, StorageValue>,
}

#[derive(Debug)]
pub enum DataBaseError {
    NotAString,
    NonExistentKey,
    NotAnInteger,
    KeyAlredyExist,
    NoMatch,
    NumberOfParamsIsIncorrectly,
}

const SUCCES: &str = "Ok";
const INTEGER: &str = "(integer)";

impl fmt::Display for DataBaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            DataBaseError::NonExistentKey => write!(f, "Non-existent key"),
            DataBaseError::NotAString => write!(f, "Value isn't a String"),
            DataBaseError::NotAnInteger => write!(f, "Value isn't an Integer"),
            DataBaseError::KeyAlredyExist => write!(f, "the key alredy exist in the database"),
            DataBaseError::NoMatch => write!(f, "(empty list or set)"),
            DataBaseError::NumberOfParamsIsIncorrectly => {
                write!(f, "number of parameters is incorrectly")
            }
        }
    }
}

impl Database {
    pub fn new() -> Database {
        Database {
            dictionary: HashMap::new(),
        }
    }

    // KEYS

    pub fn copy(&mut self, key: String, to_key: String) -> Result<String, DataBaseError> {
        if self.dictionary.contains_key(&to_key) {
            return Err(DataBaseError::KeyAlredyExist);
        }

        match self.dictionary.get(&key) {
            Some(val) => {
                let val = val.clone();
                self.dictionary.insert(to_key, val);
                Ok(String::from(SUCCES))
            }
            None => Err(DataBaseError::NonExistentKey),
        }
    }

    pub fn del(&mut self, key: String) -> Result<String, DataBaseError> {
        match self.dictionary.remove(&key) {
            Some(_) => Ok(SUCCES.to_owned()),
            None => Err(DataBaseError::NonExistentKey),
        }
    }

    pub fn exists(&mut self, key: String) -> Result<String, DataBaseError> {
        Ok(format!(
            "{} {}",
            INTEGER,
            self.dictionary.contains_key(&key) as i8
        ))
    }

    //expire

    //expireat

    pub fn keys(&mut self, pattern: String) -> Result<String, DataBaseError> {
        let result: String = self
            .dictionary
            .keys()
            .filter(|x| x.contains(&pattern))
            .map(|x| x.to_string() + "\r\n")
            .collect();

        match result.strip_suffix("\r\n") {
            Some(s) => Ok(s.to_string()),
            None => Err(DataBaseError::NoMatch),
        }
    }

    //persist

    pub fn rename(&mut self, old_key: String, new_key: String) -> Result<String, DataBaseError> {
        match self.dictionary.remove(&old_key) {
            Some(value) => {
                self.dictionary.insert(new_key, value);
                Ok(SUCCES.to_owned())
            }
            None => Err(DataBaseError::NonExistentKey),
        }
    }

    //sort

    //touch

    //ttl

    //type

    //STRINGS

    pub fn append(&mut self, key: String, value: String) -> Result<String, DataBaseError> {
        if self.dictionary.contains_key(&key) {
            if let Some(StorageValue::String(val)) = self.dictionary.get_mut(&key) {
                val.push_str(&value);
                let len_result = val.len();
                Ok(format!("{} {}", INTEGER, len_result))
            } else {
                Err(DataBaseError::NotAString)
            }
        } else {
            let val_len = value.len();
            match self.set(key, value) {
                Ok(_) => Ok(format!("{} {}", INTEGER, val_len)),
                Err(err) => Err(err),
            }
        }
    }

    pub fn decrby(&mut self, key: String, number_of_decr: String) -> Result<String, DataBaseError> {
        let number_of_decr = match number_of_decr.parse::<i32>() {
            Ok(val) => val,
            Err(_) => return Err(DataBaseError::NotAnInteger),
        };

        if let Some(StorageValue::String(val)) = self.dictionary.get_mut(&key) {
            let val = match val.parse::<i32>() {
                Ok(val) => val - number_of_decr,
                Err(_) => return Err(DataBaseError::NotAnInteger),
            };

            match self.set(key, val.to_string()) {
                Ok(_) => Ok(format!("{} {}", INTEGER, val.to_string())),
                Err(err) => Err(err),
            }
        } else {
            Err(DataBaseError::NotAnInteger)
        }
    }

    pub fn get(&mut self, key: String) -> Result<String, DataBaseError> {
        match self.dictionary.get(&key) {
            Some(StorageValue::String(val)) => Ok(val.to_string()),
            None => Err(DataBaseError::NonExistentKey),
        }
    }

    pub fn getdel(&mut self, key: String) -> Result<String, DataBaseError> {
        match self.dictionary.remove(&key) {
            Some(StorageValue::String(val)) => Ok(val),
            None => Err(DataBaseError::NonExistentKey),
        }
    }

    pub fn getset(&mut self, key: String, new_val: String) -> Result<String, DataBaseError> {
        let old_val = match self.dictionary.get(&key) {
            Some(StorageValue::String(old_value)) => old_value.to_string(),
            None => return Err(DataBaseError::NonExistentKey),
        };

        let _ = self.set(key, new_val);
        Ok(old_val)
    }

    pub fn incrby(&mut self, key: String, number_of_incr: String) -> Result<String, DataBaseError> {
        let mut number_incr = number_of_incr;
        number_incr.insert(0, '-');
        self.decrby(key, number_incr)
    }

    pub fn mget(&mut self, params: &[String]) -> Result<String, DataBaseError> {
        let mut result = String::from("");
        let mut count = 0;
        for key in params {
            count += 1;
            result.push_str((count.to_string() + &") ".to_string()).as_str());
            match self.get(key.to_string()) {
                Ok(result_ok) => {
                    result.push_str(&result_ok);
                }
                Err(error_database) => {
                    result.push_str(&error_database.to_string());
                }
            }
            result.push_str(&'\n'.to_string());
        }
        Ok(result)
    }

    pub fn mset(&mut self, params: &[String]) -> Result<String, DataBaseError> {
        for i in (0..params.len()).step_by(2) {
            let key = match params.get(i) {
                Some(val) => val,
                None => return Err(DataBaseError::NumberOfParamsIsIncorrectly),
            };

            let value = match params.get(i + 1) {
                Some(val) => val,
                None => return Err(DataBaseError::NumberOfParamsIsIncorrectly),
            };

            self.dictionary
                .insert(key.to_string(), StorageValue::String(value.to_string()));
        }
        Ok("OK".to_string())
    }

    pub fn set(&mut self, key: String, val: String) -> Result<String, DataBaseError> {
        self.dictionary.insert(key, StorageValue::String(val));
        Ok(String::from(SUCCES))
    }

    pub fn strlen(&mut self, key: &str) -> Result<String, DataBaseError> {
        if self.dictionary.contains_key(key) {
            if let Some(StorageValue::String(val)) = self.dictionary.get_mut(key) {
                Ok(format!("{}", val.len()))
            } else {
                Err(DataBaseError::NotAString)
            }
        } else {
            Ok("0".to_string())
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
mod group_string {

    use super::*;

    #[test]
    fn test_append_new_key_return_lenght_of_the_value() {
        let mut database = Database::new();

        let result = database
            .append("key".to_string(), "value".to_string())
            .unwrap();

        assert_eq!(result, "(integer) 5");
    }

    #[test]
    fn test_append_key_with_valueold_return_lenght_of_the_total_value() {
        let mut database = Database::new();

        let _ = database.append("key".to_string(), "value".to_string());

        let result = database
            .append("key".to_string(), "_concat".to_string())
            .unwrap();

        assert_eq!(result, "(integer) 12");
    }

    #[test]
    fn test_append_return_err_if_value_is_not_an_string() {
        //no se puede probar dado que append, recibe un string y deberia recibir un Tipo generico.
        /*let mut database = Database::new();
        let vector = vec![1;2;3];

        let result = database.append("key", );

        assert_eq!(result.unwrap() , String::from("(integer) 12"));
        */
    }

    #[test]
    fn test_get_returns_value_of_key() {
        let mut database = Database::new();

        let _ = database.set("key".to_string(), "value".to_string());

        let result = database.get("key".to_string()).unwrap();

        assert_eq!(result, "value");
    }

    #[test]
    fn test_get_returns_error_if_the_key_does_not_exist() {
        let mut database = Database::new();
        let result = database
            .get("key_no_exist".to_string())
            .unwrap_err()
            .to_string();

        assert_eq!(result, "Non-existent key")
    }

    #[test]
    fn test_set_returns_ok() {
        let mut database = Database::new();
        let result = database.set("key".to_string(), "1".to_string()).unwrap();
        let value = database.get("key".to_string()).unwrap();

        assert_eq!(result, "Ok");
        assert_eq!(value, "1");
    }

    #[test]
    fn test_getdel_returns_old_value_of_key_and_after_value_of_key_what_not_exists_in_database_is_nul(
    ) {
        let mut database = Database::new();
        let _ = database.set("key".to_string(), "1".to_string());
        let value = database.getdel("key".to_string()).unwrap();

        //now key, no exists in database.
        let current_value_key = database.get("key".to_string()).unwrap_err().to_string();

        assert_eq!(value, "1");
        assert_eq!(current_value_key, "Non-existent key");
    }

    #[test]
    fn test_getdel_returns_nil_value_if_key_not_exist_in_database() {
        let mut database = Database::new();

        let value = database.getdel("key".to_string()).unwrap_err().to_string();

        assert_eq!(value, "Non-existent key");
    }

    #[test]
    fn test_incrby_returns_lenght_of_the_resulting_value_after_increment() {
        let mut database = Database::new();

        let _ = database.set("key".to_string(), "1".to_string());

        let result = database.incrby("key".to_string(), "4".to_string()).unwrap();

        assert_eq!(result, "(integer) 5".to_string());
    }

    #[test]
    fn test_incrby_returns_err_if_the_value_can_not_be_parse_to_int() {
        let mut database = Database::new();

        let _ = database.set("key".to_string(), "1".to_string());

        let result = database
            .incrby("key".to_string(), "4a".to_string())
            .unwrap_err()
            .to_string();

        assert_eq!(result, "Value isn't an Integer");
    }

    #[test]
    fn test_incrby_returns_err_if_the_value_is_not_an_string() {
        /*let mut database = Database::new();

        database.rpush("key", "1").expect("error");

        let result = database.incrby("key", "4");

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "value of the key not is an String".to_string());
        */
    }

    #[test]
    fn test_decrby_returns_lenght_of_the_resulting_value_after_increment() {
        let mut database = Database::new();

        let _ = database.set("key".to_string(), "5".to_string());

        let result = database.decrby("key".to_string(), "4".to_string()).unwrap();

        assert_eq!(result, "(integer) 1");
    }

    #[test]
    fn test_decrby_returns_err_if_the_value_can_not_be_parse_to_int() {
        let mut database = Database::new();
        let _ = database.set("key".to_string(), "1".to_string());
        let result = database
            .decrby("key".to_string(), "4a".to_string())
            .unwrap_err()
            .to_string();

        assert_eq!(result, "Value isn't an Integer");
    }

    #[test]
    fn test_decrby_returns_err_if_the_value_of_key_is_not_an_string() {
        /*let mut database = Database::new();

        //database.rpush("key", "1").expect("error");

        let result = database.decrby("key", "4");

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "value of the key not is an String".to_string());
        */
    }

    #[test]
    fn test_strlen_returns_the_lenght_of_value_key() {
        let mut database = Database::new();

        let _ = database.set("key".to_string(), "abcd".to_string());

        let result = database.strlen("key").unwrap();

        assert_eq!(result, "4");
    }

    #[test]
    fn test_strlen_returns_zero_if_the_key_not_exists_in_database() {
        let mut database = Database::new();

        let result = database.strlen("no_exists_key").unwrap();

        assert_eq!(result, "0");
    }

    #[test]
    fn test_strlen_returns_err_if_value_of_key_is_not_an_string() {
        /*let mut database = Database::new();

        database.rpush("key", "abcd").expect("error");

        let result = database.strlen("key");

        assert_eq!(result.unwrap_err().to_string(), String::from("value of the key not is an String"));
        */
    }

    #[test]
    fn test_mset_set_multiple_key_and_value_ok() {
        let vect_key_value = vec![
            "key1".to_string(),
            "value1".to_string(),
            "key2".to_string(),
            "value2".to_string(),
        ];
        let mut database = Database::new();

        let result = database.mset(&vect_key_value);

        let result_get1 = database.get("key1".to_string());
        let result_get2 = database.get("key2".to_string());

        assert_eq!(result.unwrap().to_string(), "OK");
        assert_eq!(result_get1.unwrap().to_string(), "value1");
        assert_eq!(result_get2.unwrap().to_string(), "value2");
    }

    #[test]
    fn test_mset_returns_err_if_multiple_key_and_value_are_inconsistent_in_number() {
        //without value for key2
        let vect_key_value = vec!["key1".to_string(), "value1".to_string(), "key2".to_string()];

        let mut database = Database::new();

        let result = database.mset(&vect_key_value);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            String::from("number of parameters is incorrectly")
        );
    }

    #[test]
    fn test_mget_return_all_values_of_keys_if_all_keys_are_in_database() {
        //without value for key2
        let vect_key_value = vec!["key1".to_string(), "key2".to_string()];

        let mut database = Database::new();

        let _ = database.set("key1".to_string(), "value1".to_string());
        let _ = database.set("key2".to_string(), "value2".to_string());

        let result = database.mget(&vect_key_value);

        //refactorizar para que devuelva sin el \n al final del string
        assert_eq!(result.unwrap(), "1) value1\n2) value2\n");
    }

    #[test]
    fn test_mget_returns_nil_for_some_key_that_no_exists_in_database() {
        //without value for key2
        let vect_key_value = vec!["key1".to_string(), "key2".to_string(), "key3".to_string()];

        let mut database = Database::new();

        let _ = database.set("key1".to_string(), "value1".to_string());
        let _ = database.set("key3".to_string(), "value3".to_string());

        let result = database.mget(&vect_key_value);

        //refactorizar para que devuelva sin el \n al final del string
        assert_eq!(
            result.unwrap(),
            "1) value1\n2) Non-existent key\n3) value3\n"
        );
    }
}

#[cfg(test)]
mod group_keys {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_copy_set_dolly_sheep_then_copy_to_clone() {
        let mut database = Database::new();
        let _ = database.set("dolly".to_string(), "sheep".to_string());

        let result = database.copy("dolly".to_string(), "clone".to_string());
        assert_eq!(result.unwrap(), SUCCES);
        assert_eq!(database.get("clone".to_string()).unwrap(), "sheep");
    }

    #[test]
    fn test_copy_set_dolly_sheep_then_copy_to_clone_when_clone_exist() {
        let mut database = Database::new();
        let _ = database.set("dolly".to_string(), "sheep".to_string());
        let _ = database.set("clone".to_string(), "whatever".to_string());

        let result = database.copy("dolly".to_string(), "clone".to_string());
        assert_eq!(
            result.unwrap_err().to_string(),
            "the key alredy exist in the database"
        );
    }

    #[test]
    fn test_copy_try_to_copy_a_key_does_not_exist() {
        let mut database = Database::new();

        let result = database.copy("dolly".to_string(), "clone".to_string());
        assert_eq!(result.unwrap_err().to_string(), "Non-existent key");
    }

    #[test]
    fn test_del_key_hello_returns_1() {
        let mut database = Database::new();
        let _ = database.set("key".to_string(), "hello".to_string());

        let result = database.del("key".to_string());
        assert_eq!(result.unwrap(), SUCCES);
        assert_eq!(
            database.get("key".to_string()).unwrap_err().to_string(),
            "Non-existent key"
        );
    }

    #[test]
    fn test_del_key_non_exist_returns_0() {
        let mut database = Database::new();

        let result = database.del("key".to_string());
        assert_eq!(result.unwrap_err().to_string(), "Non-existent key");
    }

    #[test]
    fn test_exists_key_non_exist_returns_0() {
        let mut database = Database::new();

        let result = database.exists("key".to_string());
        assert_eq!(result.unwrap(), "(integer) 0");
        assert_eq!(
            database.get("key".to_string()).unwrap_err().to_string(),
            "Non-existent key"
        );
    }

    #[test]
    fn test_exists_key_hello_returns_0() {
        let mut database = Database::new();
        let _ = database.set("key".to_string(), "hello".to_string());

        let result = database.exists("key".to_string());
        assert_eq!(result.unwrap(), "(integer) 1");
        assert_eq!(database.get("key".to_string()).unwrap(), "hello");
    }

    #[test]
    fn test_keys_obtain_keys_with_name() {
        let mut database = Database::new();
        let _ = database.set("firstname".to_string(), "Alex".to_string());
        let _ = database.set("lastname".to_string(), "Arbieto".to_string());
        let _ = database.set("age".to_string(), "22".to_string());

        let result = database.keys("name".to_string()).unwrap();
        let result: HashSet<_> = result.split("\r\n").collect();
        assert_eq!(result, ["firstname", "lastname"].iter().cloned().collect());
    }

    #[test]
    fn test_keys_obtain_keys_with_nomatch_returns_empty_string() {
        let mut database = Database::new();
        let _ = database.set("firstname".to_string(), "Alex".to_string());
        let _ = database.set("lastname".to_string(), "Arbieto".to_string());
        let _ = database.set("age".to_string(), "22".to_string());

        let result = database.keys("nomatch".to_string());
        assert_eq!(result.unwrap_err().to_string(), "(empty list or set)");
    }

    #[test]
    fn test_rename_key_with_mykey_get_hello() {
        let mut database = Database::new();
        let _ = database.set("key".to_string(), "hello".to_string());

        let result = database
            .rename("key".to_string(), "mykey".to_string())
            .unwrap();
        assert_eq!(result, "Ok");

        let result = database.get("mykey".to_string()).unwrap();
        assert_eq!(result, "hello");

        let result = database.get("key".to_string()).unwrap_err().to_string();
        assert_eq!(result, "Non-existent key");
    }

    #[test]
    fn test_rename_key_non_exists_error() {
        let mut database = Database::new();

        let result = database
            .rename("key".to_string(), "mykey".to_string())
            .unwrap_err()
            .to_string();
        assert_eq!(result, "Non-existent key");
    }
}
