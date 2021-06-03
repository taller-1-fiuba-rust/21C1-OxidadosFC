use crate::storagevalue::StorageValue;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Formatter;

pub struct Database {
    dictionary: HashMap<String, StorageValue>,
}

pub enum DataBaseError {
    NotAString,
    NonExistentKey,
    NotAnInteger,
    KeyAlredyExist,
    NoMatch,
}

impl fmt::Display for DataBaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            DataBaseError::NonExistentKey => write!(f, "Non-existent key"),
            DataBaseError::NotAString => write!(f, "Value isn't a String"),
            DataBaseError::NotAnInteger => write!(f, "Value isn't an Integer"),
            DataBaseError::KeyAlredyExist => write!(f, "the key alredy exist in the database"),
            DataBaseError::NoMatch => write!(f, "(empty list or set)"),
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
        if self.dictionary.contains_key(&key) {
            if let Some(StorageValue::String(val)) = self.dictionary.get_mut(&key) {
                val.push_str(&value);
                let len_result = val.len();
                Ok(format!("(integer) {}", len_result))
            } else {
                Err(DataBaseError::NotAString)
            }
        } else {
            let val_len = value.len();
            match self.set(key, value) {
                Ok(_) => Ok(format!("(integer) {}", val_len)),
                Err(err) => Err(err),
            }
        }
    }

    pub fn rename(&mut self, old_key: String, new_key: String) -> Result<String, DataBaseError> {
        match self.dictionary.remove(&old_key) {
            Some(value) => {
                self.dictionary.insert(new_key, value);
                Ok("Ok".to_string())
            }
            None => Err(DataBaseError::NonExistentKey),
        }
    }

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

    pub fn exists(&mut self, key: String) -> Result<String, DataBaseError> {
        Ok((self.dictionary.contains_key(&key) as i8).to_string())
    }

    pub fn copy(&mut self, key: String, to_key: String) -> Result<String, DataBaseError> {
        let value = match self.dictionary.get(&key) {
            Some(StorageValue::String(val)) => {
                if self.dictionary.contains_key(&to_key) {
                    return Err(DataBaseError::KeyAlredyExist);
                } else {
                    val.clone()
                }
            }
            None => return Err(DataBaseError::NonExistentKey),
        };
        self.dictionary.insert(to_key, StorageValue::String(value));
        Ok(String::from("1"))
    }

    pub fn del(&mut self, key: String) -> Result<String, DataBaseError> {
        match self.dictionary.remove(&key) {
            Some(_) => Ok(String::from("1")),
            None => Ok(String::from("0")),
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
        self.dictionary.insert(key, StorageValue::String(val));
        Ok(String::from("OK"))
    }

    pub fn decrby(&mut self, key: String, number_of_decr: String) -> Result<String, DataBaseError> {
        if let Some(StorageValue::String(val)) = self.dictionary.get_mut(&key) {
            if let Ok(number_of_decr) = number_of_decr.parse::<i64>() {
                if let Ok(mut number) = val.parse::<i64>() {
                    number -= number_of_decr;

                    match self.set(key, number.to_string()) {
                        Ok(_) => Ok(format!("(integer) {}", number.to_string())),
                        Err(err) => Err(err),
                    }
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
        let mut number_incr = number_of_incr;
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
        let mut _copy_key = key;
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

// #[cfg(test)]
// mod commandtest {
//     use crate::database::Database;
//     use std::collections::HashSet;

//     #[test]
//     fn test_get_returns_value_of_key() {
//         let mut database = Database::new();
//         database
//             .set("key".to_owned(), "value".to_owned())
//             .expect("error");
//         let result = database.get("key".to_owned());
//         assert_eq!(result.unwrap(), String::from("value"));
//     }

//     #[test]
//     fn test_get_returns_error_if_the_key_does_not_exist() {
//         let mut database = Database::new();
//         let result = database.get("key_no_exist".to_owned());
//         assert_eq!(result.unwrap_err().to_string(), "(nil)")
//     }

//     #[test]
//     fn test_set_returns_ok() {
//         let mut database = Database::new();
//         let result = database.set("key".to_owned(), "1".to_owned());
//         let value = database.get("key".to_owned());
//         assert_eq!(result.unwrap(), "OK".to_string());
//         assert_eq!(value.unwrap(), "1".to_string())
//     }

//     #[test]
//     fn test_getdel_returns_old_value_of_key_and_after_value_of_key_what_not_exists_in_database_is_nul(
//     ) {
//         let mut database = Database::new();
//         database
//             .set("key".to_owned(), "1".to_owned())
//             .expect("error");
//         let value = database.getdel("key".to_owned());
//         let _current_value_key = database.get("key".to_owned());
//         assert_eq!(value.unwrap(), "1".to_string());
//         assert_eq!(
//             _current_value_key.unwrap_err().to_string(),
//             "(nil)".to_string()
//         );
//     }

//     #[test]
//     fn test_getdel_returns_nil_value_if_key_not_exist_in_database() {
//         let mut database = Database::new();
//         let value = database.getdel("key".to_owned());
//         assert_eq!(value.unwrap_err().to_string(), "(nil)".to_string())
//     }

//     #[test]
//     fn test_incrby_returns_lenght_of_the_resulting_value_after_increment() {
//         let mut database = Database::new();
//         database
//             .set("key".to_owned(), "1".to_owned())
//             .expect("error");
//         let result = database.incrby("key".to_owned(), "4".to_owned());
//         assert_eq!(result.unwrap(), "(integer) 5".to_string());
//     }

//     #[test]
//     fn test_incrby_returns_err_if_the_value_can_not_be_parse_to_int() {
//         let mut database = Database::new();
//         database
//             .set("key".to_owned(), "1".to_owned())
//             .expect("error");
//         let result = database.incrby("key".to_owned(), "4a".to_owned());
//         assert!(result.is_err());
//         assert_eq!(
//             result.unwrap_err().to_string(),
//             "the value cannot be parsed to int".to_string()
//         );
//     }

//     #[test]
//     fn test_decrby_returns_lenght_of_the_resulting_value_after_increment() {
//         let mut database = Database::new();
//         database
//             .set("key".to_owned(), "5".to_owned())
//             .expect("error");
//         let result = database.decrby("key".to_owned(), "4".to_owned());
//         assert_eq!(result.unwrap(), "(integer) 1".to_string());
//     }

//     #[test]
//     fn test15_set_dolly_sheep_then_copy_to_clone() {
//         let mut database = Database::new();
//         let _ = database.set("dolly", "sheep");

//         let result = database.copy("dolly", "clone");
//         assert_eq!(result.unwrap(), "1");
//         assert_eq!(database.get("clone").unwrap(), "sheep");
//     }

//     #[test]
//     fn test16_set_dolly_sheep_then_copy_to_clone_when_clone_exist() {
//         let mut database = Database::new();
//         let _ = database.set("dolly", "sheep");
//         let _ = database.set("clone", "whatever");

//         let result = database.copy("dolly", "clone");
//         assert_eq!(
//             result.unwrap_err().to_string(),
//             "the key alredy exist in the database"
//         );
//     }

//     #[test]
//     fn test16_try_to_copy_a_key_does_not_exist() {
//         let mut database = Database::new();

//         let result = database.copy("dolly", "clone");
//         assert_eq!(result.unwrap_err().to_string(), "(nil)");
//     }

//     #[test]
//     fn test17_del_key_hello_returns_1() {
//         let mut database = Database::new();
//         let _ = database.set("key", "hello");

//         let result = database.del("key");
//         assert_eq!(result.unwrap(), "1");
//         assert_eq!(database.get("key").unwrap_err().to_string(), "(nil)");
//     }

//     #[test]
//     fn test18_del_key_non_exist_returns_0() {
//         let mut database = Database::new();

//         let result = database.del("key");
//         assert_eq!(result.unwrap(), "0");
//         assert_eq!(database.get("key").unwrap_err().to_string(), "(nil)");
//     }

//     #[test]
//     fn test19_exists_key_non_exist_returns_0() {
//         let mut database = Database::new();

//         let result = database.exists("key");
//         assert_eq!(result.unwrap(), "0");
//         assert_eq!(database.get("key").unwrap_err().to_string(), "(nil)");
//     }

//     #[test]
//     fn test19_exists_key_hello_returns_0() {
//         let mut database = Database::new();
//         let _ = database.set("key", "hello");

//         let result = database.exists("key");
//         assert_eq!(result.unwrap(), "1");
//         assert_eq!(database.get("key").unwrap(), "hello");
//     }

//     #[test]
//     fn test20_obtain_keys_with_name() {
//         let mut database = Database::new();
//         let _ = database.set("firstname", "Alex");
//         let _ = database.set("lastname", "Arbieto");
//         let _ = database.set("age", "22");

//         let result = database.keys("name").unwrap();
//         let result: HashSet<_> = result.split("\r\n").collect();
//         assert_eq!(result, ["firstname", "lastname"].iter().cloned().collect());
//     }

//     #[test]
//     fn test21_obtain_keys_with_nomatch_returns_empty_string() {
//         let mut database = Database::new();
//         let _ = database.set("firstname", "Alex");
//         let _ = database.set("lastname", "Arbieto");
//         let _ = database.set("age", "22");

//         let result = database.keys("nomatch");
//         assert_eq!(result.unwrap_err().to_string(), "(empty list or set)");
//     }

//     #[test]
//     fn test22_rename_key_with_mykey_get_hello() {
//         let mut database = Database::new();
//         let _ = database.set("key", "hello");

//         let result = database.rename("key", "mykey").unwrap();
//         assert_eq!(result, "Ok");

//         let result = database.get("mykey").unwrap();
//         assert_eq!(result, "hello");

//         let result = database.get("key").unwrap_err().to_string();
//         assert_eq!(result, "(nil)");
//     }

//     #[test]
//     fn test22_rename_key_non_exists_error() {
//         let mut database = Database::new();

//         let result = database.rename("key", "mykey").unwrap_err().to_string();
//         assert_eq!(result, "ERR ERR no such key");
//     }
// }
