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
}

impl fmt::Display for DataBaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            DataBaseError::NonExistentKey => write!(f, "Non-existent key"),
            DataBaseError::NotAString => write!(f, "Value isn't a String"),
            DataBaseError::NotAnInteger => write!(f, "Value isn't an Integer"),
        }
    }
}

impl Database {
    pub fn new() -> Database {
        Database {
            dictionary: HashMap::new(),
        }
    }

    pub fn append(&mut self, key: String, value: String) -> Result<String, DataBaseError> {
        let len_value = value.len();
        if self.dictionary.contains_key(&key) {
            if let Some(StorageValue::String(val)) = self.dictionary.get_mut(&key) {
                val.push_str(&value);
                let len_result = val.len();
                Ok(format!("(integer) {}", len_result))
            } else {
                Err(DataBaseError::NotAString)
            }
        } else {
            self.set(key, value).expect("error");
            Ok(format!("(integer) {}", len_value))
        }
    }

    pub fn get(&mut self, key: String) -> Result<String, DataBaseError> {
        if let Some(StorageValue::String(val)) = self.dictionary.get(&key) {
            Ok(val.to_string())
        } else {
            Err(DataBaseError::NonExistentKey)
        }
    }

    pub fn set(&mut self, key: String, val: String) -> Result<String, DataBaseError> {
        self.dictionary
            .insert(key, StorageValue::String(val));
        Ok(String::from("OK"))
    }

    pub fn decrby(&mut self, key: String, number_of_decr: String) -> Result<String, DataBaseError> {
        if let Some(StorageValue::String(val)) = self.dictionary.get_mut(&key) {
            if let Ok(number_of_decr) = number_of_decr.parse::<i64>() {
                if let Ok(mut number) = val.parse::<i64>() {
                    number -= number_of_decr;
                    self.set(key, number.to_string()).expect("error");
                    Ok(format!("(integer) {}", number.to_string()))
                } else {
                    Err(DataBaseError::NotAnInteger)
                }
            } else {
                Err(DataBaseError::NotAnInteger)
            }
        } else {
            Err(DataBaseError::NotAString)
        }
    }

    pub fn incrby(&mut self, key: String, number_of_incr: String) -> Result<String, DataBaseError> {
        let mut number_incr = String::from(number_of_incr);
        number_incr.insert(0, '-');
        self.decrby(key, number_incr)
    }

    pub fn getdel(&mut self, key: String) -> Result<String, DataBaseError> {
        if let Some(StorageValue::String(value)) = self.dictionary.remove(&key) {
            Ok(value)
        } else {
            Err(DataBaseError::NonExistentKey)
        }
    }

    pub fn getset(&mut self, key: String, _new_val: String) -> Result<String, DataBaseError> {
        let mut _copy_key = key.clone();
        if let Some(storage_value) = self.dictionary.get(&_copy_key) {
            match storage_value {
                StorageValue::String(old_value) => Ok(old_value.to_string()),
            }
        } else {
            Err(DataBaseError::NonExistentKey)
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
mod commandtest {
    use crate::database::Database;

    #[test]
    fn test_get_returns_value_of_key() {
        let mut database = Database::new();
        database.set("key".to_owned(), "value".to_owned()).expect("error");
        let result = database.get("key".to_owned());
        assert_eq!(result.unwrap(), String::from("value"));
    }

    #[test]
    fn test_get_returns_error_if_the_key_does_not_exist() {
        let mut database = Database::new();
        let result = database.get("key_no_exist".to_owned());
        assert_eq!(result.unwrap_err().to_string(), "(nil)")
    }

    #[test]
    fn test_set_returns_ok() {
        let mut database = Database::new();
        let result = database.set("key".to_owned(), "1".to_owned());
        let value = database.get("key".to_owned());
        assert_eq!(result.unwrap(), "OK".to_string());
        assert_eq!(value.unwrap(), "1".to_string())
    }

    #[test]
    fn test_getdel_returns_old_value_of_key_and_after_value_of_key_what_not_exists_in_database_is_nul(
    ) {
        let mut database = Database::new();
        database.set("key".to_owned(), "1".to_owned()).expect("error");
        let value = database.getdel("key".to_owned());
        let _current_value_key = database.get("key".to_owned());
        assert_eq!(value.unwrap(), "1".to_string());
        assert_eq!(
            _current_value_key.unwrap_err().to_string(),
            "(nil)".to_string()
        );
    }

    #[test]
    fn test_getdel_returns_nil_value_if_key_not_exist_in_database() {
        let mut database = Database::new();
        let value = database.getdel("key".to_owned());
        assert_eq!(value.unwrap_err().to_string(), "(nil)".to_string())
    }

    #[test]
    fn test_incrby_returns_lenght_of_the_resulting_value_after_increment() {
        let mut database = Database::new();
        database.set("key".to_owned(), "1".to_owned()).expect("error");
        let result = database.incrby("key".to_owned(), "4".to_owned());
        assert_eq!(result.unwrap(), "(integer) 5".to_string());
    }

    #[test]
    fn test_incrby_returns_err_if_the_value_can_not_be_parse_to_int() {
        let mut database = Database::new();
        database.set("key".to_owned(), "1".to_owned()).expect("error");
        let result = database.incrby("key".to_owned(), "4a".to_owned());
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "the value cannot be parsed to int".to_string()
        );
    }

    #[test]
    fn test_decrby_returns_lenght_of_the_resulting_value_after_increment() {
        let mut database = Database::new();
        database.set("key".to_owned(), "5".to_owned()).expect("error");
        let result = database.decrby("key".to_owned(), "4".to_owned());
        assert_eq!(result.unwrap(), "(integer) 1".to_string());
    }
}
