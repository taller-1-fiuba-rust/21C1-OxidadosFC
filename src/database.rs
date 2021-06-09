use crate::database_errors::DataBaseError;
use crate::storagevalue::StorageValue;
use regex::Regex;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::fmt::Formatter;

pub struct Database {
    dictionary: HashMap<String, StorageValue>,
}

const SUCCES: &str = "Ok";
const INTEGER: &str = "(integer)";
const NIL: &str = "(nil)";
const EMPTY: &str = "(empty list or set)";
const NUMBER_INSERT_SUCCESS: u32 = 1;
const NUMBER_NOT_INSERT: u32 = 0;

impl Database {
    pub fn new() -> Database {
        Database {
            dictionary: HashMap::new(),
        }
    }

    // SERVER
    pub fn flushdb(&mut self) -> Result<String, DataBaseError> {
        self.dictionary.clear();
        Ok(SUCCES.to_string())
    }

    pub fn dbsize(&self) -> Result<String, DataBaseError> {
        Ok(format!("{} {}", INTEGER, self.dictionary.len()))
    }

    // KEYS

    pub fn copy(&mut self, key: &str, to_key: &str) -> Result<String, DataBaseError> {
        if self.dictionary.contains_key(to_key) {
            return Err(DataBaseError::KeyAlredyExist);
        }

        match self.dictionary.get(key) {
            Some(val) => {
                let val = val.clone();
                self.dictionary.insert(to_key.to_owned(), val);
                Ok(String::from(SUCCES))
            }
            None => Err(DataBaseError::NonExistentKey),
        }
    }

    pub fn del(&mut self, key: &str) -> Result<String, DataBaseError> {
        match self.dictionary.remove(key) {
            Some(_) => Ok(SUCCES.to_owned()),
            None => Err(DataBaseError::NonExistentKey),
        }
    }

    pub fn exists(&mut self, key: &str) -> Result<String, DataBaseError> {
        Ok(format!(
            "{} {}",
            INTEGER,
            self.dictionary.contains_key(key) as i8
        ))
    }

    //expire

    //expireat

    pub fn keys(&mut self, pattern: String) -> Result<String, DataBaseError> {
        let patt: String = r"^".to_owned() + &pattern + r"$";
        let patt: String = patt.replace("*", ".*");
        let patt: String = patt.replace("?", ".");
        let result: String = self
            .dictionary
            .keys()
            .filter(|x| match Regex::new(&patt) {
                Ok(re) => re.is_match(x),
                Err(_) => false,
            })
            .map(|x| x.to_string() + "\r\n")
            .collect();

        match result.strip_suffix("\r\n") {
            Some(s) => Ok(s.to_string()),
            None => Err(DataBaseError::NoMatch),
        }
    }

    //persist

    pub fn rename(&mut self, old_key: &str, new_key: &str) -> Result<String, DataBaseError> {
        match self.dictionary.remove(old_key) {
            Some(value) => {
                self.dictionary.insert(new_key.to_owned(), value);
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

    pub fn append(&mut self, key: &str, value: String) -> Result<String, DataBaseError> {
        if self.dictionary.contains_key(key) {
            if let Some(StorageValue::String(val)) = self.dictionary.get_mut(key) {
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

    pub fn decrby(&mut self, key: &str, number_of_decr: String) -> Result<String, DataBaseError> {
        let number_of_decr = match number_of_decr.parse::<i32>() {
            Ok(val) => val,
            Err(_) => return Err(DataBaseError::NotAnInteger),
        };

        if let Some(StorageValue::String(val)) = self.dictionary.get_mut(key) {
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

    pub fn get(&mut self, key: &str) -> Result<String, DataBaseError> {
        match self.dictionary.get(key) {
            Some(StorageValue::String(val)) => Ok(val.to_string()),
            Some(_) => Err(DataBaseError::NotAString),
            None => Err(DataBaseError::NonExistentKey),
        }
    }

    pub fn getdel(&mut self, key: &str) -> Result<String, DataBaseError> {
        match self.dictionary.remove(key) {
            Some(StorageValue::String(val)) => Ok(val),
            Some(_) => Err(DataBaseError::NotAString),
            None => Err(DataBaseError::NonExistentKey),
        }
    }

    pub fn getset(&mut self, key: &str, new_val: String) -> Result<String, DataBaseError> {
        let old_val = match self.dictionary.get(key) {
            Some(StorageValue::String(old_value)) => old_value.to_string(),
            Some(_) => return Err(DataBaseError::NotAString),
            None => return Err(DataBaseError::NonExistentKey),
        };

        let _ = self.set(key, new_val);
        Ok(old_val)
    }

    pub fn incrby(&mut self, key: &str, number_of_incr: String) -> Result<String, DataBaseError> {
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
            match self.get(key) {
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

    pub fn set(&mut self, key: &str, val: String) -> Result<String, DataBaseError> {
        self.dictionary
            .insert(key.to_owned(), StorageValue::String(val));
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

    // Lists

    pub fn lindex(&mut self, key: &str, index: String) -> Result<String, DataBaseError> {
        let index = match index.parse::<i32>() {
            Ok(val) => val,
            Err(_) => return Err(DataBaseError::NotAnInteger),
        };

        match self.dictionary.get(key) {
            Some(StorageValue::List(list)) => {
                let index = if index < 0 {
                    ((list.len() as i32) + index) as usize
                } else {
                    index as usize
                };

                match list.get(index) {
                    Some(val) => Ok(val.to_string()),
                    None => Ok(NIL.to_owned()),
                }
            }
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(NIL.to_owned()),
        }
    }

    pub fn llen(&mut self, key: &str) -> Result<String, DataBaseError> {
        match self.dictionary.get(key) {
            Some(StorageValue::List(list)) => Ok(format!("{} {}", INTEGER, list.len().to_string())),
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(format!("{} 0", INTEGER)),
        }
    }

    pub fn lpop(&mut self, key: &str) -> Result<String, DataBaseError> {
        match self.dictionary.get_mut(key) {
            Some(StorageValue::List(list)) => {
                if list.is_empty() {
                    Ok(NIL.to_owned())
                } else {
                    Ok(list.remove(0))
                }
            }
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(NIL.to_owned()),
        }
    }

    pub fn lpush(&mut self, key: &str, value: String) -> Result<String, DataBaseError> {
        match self.dictionary.get_mut(key) {
            Some(StorageValue::List(list)) => {
                list.insert(0, value);
                Ok(format!("{} {}", INTEGER, list.len().to_string()))
            }
            Some(_) => Err(DataBaseError::NotAList),
            None => {
                let list: Vec<String> = vec![value];
                let len = list.len();
                self.dictionary
                    .insert(key.to_owned(), StorageValue::List(list));
                Ok(format!("{} {}", INTEGER, len.to_string()))
            }
        }
    }

    pub fn lpushx(&mut self, key: &str, value: String) -> Result<String, DataBaseError> {
        match self.dictionary.get_mut(key) {
            Some(StorageValue::List(list)) => {
                list.insert(0, value);
                Ok(format!("{} {}", INTEGER, list.len().to_string()))
            }
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(format!("{} 0", INTEGER)),
        }
    }

    pub fn lrange(&mut self, key: &str, beg: String, end: String) -> Result<String, DataBaseError> {
        let beg = match beg.parse::<i32>() {
            Ok(val) => val,
            Err(_) => return Err(DataBaseError::NotAnInteger),
        };

        let end = match end.parse::<i32>() {
            Ok(val) => val,
            Err(_) => return Err(DataBaseError::NotAnInteger),
        };

        match self.dictionary.get(key) {
            Some(StorageValue::List(list)) => {
                let len: i32 = list.len() as i32;
                let beg = if beg < 0 { len + beg } else { beg };
                let end = if end < 0 { len + end } else { end };

                if beg >= 0 && end <= len && beg <= end {
                    let mut list_srt = String::new();

                    for (i, elem) in list[(beg as usize)..(end as usize + 1)].iter().enumerate() {
                        let row = format!("{}) {}\n", i + 1, elem);
                        list_srt.push_str(&row);
                    }

                    Ok(list_srt)
                } else {
                    Ok(EMPTY.to_owned())
                }
            }
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(EMPTY.to_owned()),
        }
    }

    pub fn lrem(
        &mut self,
        key: &str,
        count: String,
        elem: String,
    ) -> Result<String, DataBaseError> {
        let mut count = match count.parse::<i32>() {
            Ok(val) => val,
            Err(_) => return Err(DataBaseError::NotAnInteger),
        };

        match self.dictionary.get_mut(key) {
            Some(StorageValue::List(list)) => match count.cmp(&0) {
                Ordering::Greater => {
                    let mut exist_elem = true;

                    while count > 0 && exist_elem {
                        if let Some(pos) = list.iter().position(|x| *x == elem) {
                            list.remove(pos);
                            count -= 1;
                        } else {
                            exist_elem = false;
                        }
                    }

                    Ok(format!("{} {}", INTEGER, list.len()))
                }
                Ordering::Less => {
                    let mut exist_elem = true;

                    while count < 0 && exist_elem {
                        if let Some(pos) = list.iter().rposition(|x| *x == elem) {
                            list.remove(pos);
                            count += 1;
                        } else {
                            exist_elem = false;
                        }
                    }

                    Ok(format!("{} {}", INTEGER, list.len()))
                }
                Ordering::Equal => {
                    list.retain(|x| *x != elem);
                    Ok(format!("{} {}", INTEGER, list.len()))
                }
            },
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(format!("{} 0", INTEGER)),
        }
    }

    pub fn lset(&mut self, key: &str, index: &str, value: &str) -> Result<String, DataBaseError> {
        match index.parse::<usize>() {
            Ok(index) => match self.dictionary.get_mut(key) {
                Some(StorageValue::List(list)) => match list.get_mut(index) {
                    Some(val) => {
                        val.clear();
                        val.push_str(value);
                        Ok(SUCCES.to_owned())
                    }
                    None => Err(DataBaseError::IndexOutOfRange),
                },
                Some(_) => Err(DataBaseError::NotAList),
                None => Err(DataBaseError::NonExistentKey),
            },
            Err(_) => Err(DataBaseError::NotAnInteger),
        }
    }

    pub fn rpop(&mut self, key: &str) -> Result<String, DataBaseError> {
        match self.dictionary.get_mut(key) {
            Some(StorageValue::List(list)) => match list.pop() {
                Some(value) => Ok(value),
                None => Ok(NIL.to_owned()),
            },
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(NIL.to_owned()),
        }
    }

    pub fn rpush(&mut self, key: &str, value: String) -> Result<String, DataBaseError> {
        match self.dictionary.get_mut(key) {
            Some(StorageValue::List(list)) => {
                list.push(value);
                Ok(format!("{} {}", INTEGER, list.len().to_string()))
            }
            Some(_) => Err(DataBaseError::NotAList),
            None => {
                let list: Vec<String> = vec![value];
                let len = list.len();
                self.dictionary
                    .insert(key.to_owned(), StorageValue::List(list));
                Ok(format!("{} {}", INTEGER, len.to_string()))
            }
        }
    }

    pub fn rpushx(&mut self, key: &str, value: String) -> Result<String, DataBaseError> {
        match self.dictionary.get_mut(key) {
            Some(StorageValue::List(list)) => {
                list.push(value);
                Ok(format!("{} {}", INTEGER, list.len().to_string()))
            }
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(format!("{} 0", INTEGER)),
        }
    }

    pub fn sismember(&mut self, key: String, value: String) -> Result<String, DataBaseError> {
        match self.dictionary.get_mut(&key) {
            Some(StorageValue::Set(hash_set)) => match hash_set.get(&value) {
                Some(_val) => Ok(format!("{} {}", INTEGER, NUMBER_INSERT_SUCCESS)),
                None => Ok(format!("{} {}", INTEGER, NUMBER_NOT_INSERT)),
            },
            Some(_) => Err(DataBaseError::NotASet),
            None => Ok(format!("{} {}", INTEGER, NUMBER_NOT_INSERT)),
        }
    }

    pub fn scard(&mut self, key: String) -> Result<String, DataBaseError> {
        match self.dictionary.get_mut(&key) {
            Some(StorageValue::Set(hash_set)) => {
                let len = hash_set.len();
                Ok(format!("{} {}", INTEGER, len))
            }
            Some(_) => Err(DataBaseError::NotASet),
            None => Ok(format!("{} {}", INTEGER, NUMBER_NOT_INSERT)),
        }
    }

    pub fn sadd(&mut self, key: String, value: String) -> Result<String, DataBaseError> {
        match self.dictionary.get_mut(&key) {
            Some(StorageValue::Set(hash_set)) => {
                let mut number_response = NUMBER_INSERT_SUCCESS;
                let _ = match hash_set.get(&value) {
                    Some(_val) => number_response = NUMBER_NOT_INSERT,
                    None => {
                        let _ = hash_set.insert(value);
                    }
                };
                Ok(format!("{} {}", INTEGER, number_response))
            }
            Some(_) => Err(DataBaseError::NotASet),
            None => {
                let mut set: HashSet<String> = HashSet::new();
                set.insert(value);
                self.dictionary.insert(key, StorageValue::Set(set));
                Ok(format!("{} {}", INTEGER, NUMBER_INSERT_SUCCESS))
            }
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

    const KEY: &str = "KEY";

    #[test]
    fn test_append_new_key_return_lenght_of_the_value() {
        let mut database = Database::new();

        let result = database.append(KEY, "value".to_string()).unwrap();

        assert_eq!(result, "(integer) 5");
    }

    #[test]
    fn test_append_key_with_valueold_return_lenght_of_the_total_value() {
        let mut database = Database::new();

        let _ = database.append(KEY, "value".to_string());

        let result = database.append(KEY, "_concat".to_string()).unwrap();

        assert_eq!(result, "(integer) 12");
    }

    #[test]
    fn test_get_returns_value_of_key() {
        let mut database = Database::new();

        database.set(KEY, "value".to_string()).unwrap();

        let result = database.get(KEY).unwrap();

        assert_eq!(result, "value");
    }

    #[test]
    fn test_get_returns_error_if_the_key_does_not_exist() {
        let mut database = Database::new();
        let result = database.get(KEY).unwrap_err().to_string();

        assert_eq!(result, "Non-existent key")
    }

    #[test]
    fn test_set_returns_ok() {
        let mut database = Database::new();
        let result = database.set(KEY, "1".to_string()).unwrap();
        let value = database.get(KEY).unwrap();

        assert_eq!(result, "Ok");
        assert_eq!(value, "1");
    }

    #[test]
    fn test_getdel_returns_old_value_of_key_and_after_value_of_key_what_not_exists_in_database_is_nul(
    ) {
        let mut database = Database::new();
        database.set(KEY, "1".to_string()).unwrap();
        let value = database.getdel(KEY).unwrap();

        let current_value_key = database.get(KEY).unwrap_err().to_string();

        assert_eq!(value, "1");
        assert_eq!(current_value_key, "Non-existent key");
    }

    #[test]
    fn test_getdel_returns_nil_value_if_key_not_exist_in_database() {
        let mut database = Database::new();

        let value = database.getdel(KEY).unwrap_err().to_string();

        assert_eq!(value, "Non-existent key");
    }

    #[test]
    fn test_incrby_returns_lenght_of_the_resulting_value_after_increment() {
        let mut database = Database::new();

        let _ = database.set(KEY, "1".to_string());

        let result = database.incrby(KEY, "4".to_string()).unwrap();

        assert_eq!(result, "(integer) 5".to_string());
    }

    #[test]
    fn test_incrby_returns_err_if_the_value_can_not_be_parse_to_int() {
        let mut database = Database::new();

        let _ = database.set(KEY, "1".to_string());

        let result = database
            .incrby(KEY, "4a".to_string())
            .unwrap_err()
            .to_string();

        assert_eq!(result, "Value isn't an Integer");
    }

    #[test]
    fn test_decrby_returns_lenght_of_the_resulting_value_after_increment() {
        let mut database = Database::new();

        let _ = database.set(KEY, "5".to_string());

        let result = database.decrby(KEY, "4".to_string()).unwrap();

        assert_eq!(result, "(integer) 1");
    }

    #[test]
    fn test_decrby_returns_err_if_the_value_can_not_be_parse_to_int() {
        let mut database = Database::new();
        let _ = database.set(KEY, "1".to_string());
        let result = database
            .decrby(KEY, "4a".to_string())
            .unwrap_err()
            .to_string();

        assert_eq!(result, "Value isn't an Integer");
    }

    #[test]
    fn test_strlen_returns_the_lenght_of_value_key() {
        let mut database = Database::new();

        let _ = database.set(KEY, "abcd".to_string());

        let result = database.strlen(KEY).unwrap();

        assert_eq!(result, "4");
    }

    #[test]
    fn test_strlen_returns_zero_if_the_key_not_exists_in_database() {
        let mut database = Database::new();

        let result = database.strlen("no_exists_key").unwrap();

        assert_eq!(result, "0");
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

        let result_get1 = database.get("key1");
        let result_get2 = database.get("key2");

        assert_eq!(result.unwrap().to_string(), "OK");
        assert_eq!(result_get1.unwrap().to_string(), "value1");
        assert_eq!(result_get2.unwrap().to_string(), "value2");
    }

    #[test]
    fn test_mset_returns_err_if_multiple_key_and_value_are_inconsistent_in_number() {
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
        let vect_key_value = vec!["key1".to_string(), "key2".to_string()];

        let mut database = Database::new();

        database.set("key1", "value1".to_string()).unwrap();
        database.set("key2", "value2".to_string()).unwrap();

        let result = database.mget(&vect_key_value);

        assert_eq!(result.unwrap(), "1) value1\n2) value2\n");
    }

    #[test]
    fn test_mget_returns_nil_for_some_key_that_no_exists_in_database() {
        let vect_key_value = vec!["key1".to_string(), "key2".to_string(), "key3".to_string()];

        let mut database = Database::new();

        database.set("key1", "value1".to_string()).unwrap();
        database.set("key3", "value3".to_string()).unwrap();

        let result = database.mget(&vect_key_value);

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

    const KEY: &str = "KEY";

    #[test]
    fn test_copy_set_dolly_sheep_then_copy_to_clone() {
        let mut database = Database::new();
        let key_a = "dolly";
        let key_b = "clone";

        database.set(key_a, "sheep".to_string()).unwrap();

        let result = database.copy(key_a, key_b);
        assert_eq!(result.unwrap(), SUCCES);
        assert_eq!(database.get(key_b).unwrap(), "sheep");
    }

    #[test]
    fn test_copy_set_dolly_sheep_then_copy_to_clone_when_clone_exist() {
        let mut database = Database::new();
        let key_a = "dolly";
        let key_b = "clone";

        database.set(key_a, "sheep".to_string()).unwrap();
        database.set(key_b, "whatever".to_string()).unwrap();

        let result = database.copy(key_a, key_b);

        assert_eq!(
            result.unwrap_err().to_string(),
            "the key alredy exist in the database"
        );
    }

    #[test]
    fn test_copy_try_to_copy_a_key_does_not_exist() {
        let mut database = Database::new();
        let key_a = "dolly";
        let key_b = "clone";

        let result = database.copy(key_a, key_b);

        assert_eq!(result.unwrap_err().to_string(), "Non-existent key");
    }

    #[test]
    fn test_del_key_hello_returns_1() {
        let mut database = Database::new();
        database.set(KEY, "hello".to_string()).unwrap();

        let result = database.del(KEY);

        assert_eq!(result.unwrap(), SUCCES);
        assert_eq!(
            database.get(KEY).unwrap_err().to_string(),
            "Non-existent key"
        );
    }

    #[test]
    fn test_del_key_non_exist_returns_0() {
        let mut database = Database::new();

        let result = database.del(KEY);

        assert_eq!(result.unwrap_err().to_string(), "Non-existent key");
    }

    #[test]
    fn test_exists_key_non_exist_returns_0() {
        let mut database = Database::new();

        let result = database.exists(KEY);
        assert_eq!(result.unwrap(), "(integer) 0");
        assert_eq!(
            database.get(KEY).unwrap_err().to_string(),
            "Non-existent key"
        );
    }

    #[test]
    fn test_exists_key_hello_returns_0() {
        let mut database = Database::new();
        database.set(KEY, "hello".to_string()).unwrap();

        let result = database.exists(KEY);
        assert_eq!(result.unwrap(), "(integer) 1");
        assert_eq!(database.get(KEY).unwrap(), "hello");
    }

    #[cfg(test)]
    mod command_keys {
        use super::*;
        #[test]
        fn test_keys_obtain_keys_with_name() {
            let mut database = Database::new();
            let _ = database.set("firstname", "Alex".to_string());
            let _ = database.set("lastname", "Arbieto".to_string());
            let _ = database.set("age", "22".to_string());

            let result = database.keys("*name".to_string()).unwrap();
            let result: HashSet<_> = result.split("\r\n").collect();
            assert_eq!(result, ["firstname", "lastname"].iter().cloned().collect());
        }

        #[test]
        fn test_keys_obtain_keys_with_four_question_name() {
            let mut database = Database::new();
            let _ = database.set("firstname", "Alex".to_string());
            let _ = database.set("lastname", "Arbieto".to_string());
            let _ = database.set("age", "22".to_string());

            let result = database.keys("????name".to_string()).unwrap();
            let result: HashSet<_> = result.split("\r\n").collect();
            assert_eq!(result, ["lastname"].iter().cloned().collect());
        }

        #[test]
        fn test_keys_obtain_all_keys_with_an_asterisk_in_the_middle() {
            let mut database = Database::new();
            let _ = database.set("key", "val1".to_string());
            let _ = database.set("keeeey", "val2".to_string());
            let _ = database.set("ky", "val3".to_string());
            let _ = database.set("notmatch", "val4".to_string());

            let result = database.keys("k*y".to_string()).unwrap();
            let result: HashSet<_> = result.split("\r\n").collect();
            assert_eq!(result, ["key", "keeeey", "ky"].iter().cloned().collect());
        }

        #[test]
        fn test_keys_obtain_all_keys_with_question_in_the_middle() {
            let mut database = Database::new();
            let _ = database.set("key", "val1".to_string());
            let _ = database.set("keeeey", "val2".to_string());
            let _ = database.set("ky", "val3".to_string());
            let _ = database.set("notmatch", "val4".to_string());

            let result = database.keys("k?y".to_string()).unwrap();
            let result: HashSet<_> = result.split("\r\n").collect();
            assert_eq!(result, ["key"].iter().cloned().collect());
        }

        #[test]
        fn test_keys_obtain_keys_with_h_question_llo_matches_correctly() {
            let mut database = Database::new();
            let _ = database.set("hello", "a".to_string());
            let _ = database.set("hallo", "b".to_string());
            let _ = database.set("hxllo", "c".to_string());
            let _ = database.set("hllo", "d".to_string());
            let _ = database.set(r"h\llo", "d".to_string());
            let _ = database.set("ahllo", "e".to_string());
            let _ = database.set("hallown", "e".to_string());

            let result = database.keys("h?llo".to_string()).unwrap();
            let result: HashSet<_> = result.split("\r\n").collect();
            assert_eq!(
                result,
                ["hello", "hallo", "hxllo", "h\\llo"]
                    .iter()
                    .cloned()
                    .collect()
            );
        }

        #[test]
        fn test_keys_obtain_keys_with_h_asterisk_llo_matches_correctly() {
            let mut database = Database::new();
            let _ = database.set("hello", "a".to_string());
            let _ = database.set("heeeeeello", "a".to_string());
            let _ = database.set("hallo", "b".to_string());
            let _ = database.set("hxllo", "c".to_string());
            let _ = database.set("hllo", "d".to_string());
            let _ = database.set(r"h\llo", "d".to_string());
            let _ = database.set("ahllo", "e".to_string());
            let _ = database.set("hallown", "e".to_string());

            let result = database.keys("h*llo".to_string()).unwrap();
            let result: HashSet<_> = result.split("\r\n").collect();
            assert_eq!(
                result,
                ["hllo", "hello", "heeeeeello", "hallo", "hxllo", "h\\llo"]
                    .iter()
                    .cloned()
                    .collect()
            );
        }

        #[test]
        fn test_keys_obtain_keys_with_nomatch_returns_empty_string() {
            let mut database = Database::new();
            let _ = database.set("firstname", "Alex".to_string());
            let _ = database.set("lastname", "Arbieto".to_string());
            let _ = database.set("age", "22".to_string());

            let result = database.keys("nomatch".to_string());
            assert_eq!(result.unwrap_err().to_string(), "(empty list or set)");
        }
    }

    #[test]
    fn test_rename_key_with_mykey_get_hello() {
        let mut database = Database::new();
        database.set(KEY, "hello".to_string()).unwrap();

        let result = database.rename(KEY, "mykey").unwrap();
        assert_eq!(result, "Ok");

        let result = database.get("mykey").unwrap();
        assert_eq!(result, "hello");

        let result = database.get(KEY).unwrap_err().to_string();
        assert_eq!(result, "Non-existent key");
    }

    #[test]
    fn test_rename_key_non_exists_error() {
        let mut database = Database::new();

        let result = database.rename(KEY, "mykey").unwrap_err().to_string();

        assert_eq!(result, "Non-existent key");
    }
}

#[cfg(test)]
mod group_list {

    const KEY: &str = "KEY";

    use super::*;

    fn database_with_a_list() -> Database {
        let mut database = Database::new();

        database.lpush(KEY, "ValueA".to_owned()).unwrap();
        database.lpush(KEY, "ValueB".to_owned()).unwrap();
        database.lpush(KEY, "ValueC".to_owned()).unwrap();
        database.lpush(KEY, "ValueD".to_owned()).unwrap();

        database
    }

    fn database_with_a_three_repeated_values() -> Database {
        let mut database = Database::new();

        database.lpush(KEY, "ValueA".to_owned()).unwrap();
        database.lpush(KEY, "ValueA".to_owned()).unwrap();
        database.lpush(KEY, "ValueC".to_owned()).unwrap();
        database.lpush(KEY, "ValueA".to_owned()).unwrap();

        database
    }

    fn database_with_a_string() -> Database {
        let mut database = Database::new();
        database.append(KEY, "ValueA".to_owned()).unwrap();
        database
    }

    mod llen_test {

        use super::*;

        #[test]
        fn test_llen_on_a_non_existent_key_gets_len_zero() {
            let mut database = Database::new();

            let result = database.llen(KEY);
            assert_eq!(result.unwrap(), format!("{} 0", INTEGER));
        }

        #[test]
        fn test_llen_on_a_list_with_one_value() {
            let mut database = Database::new();
            database.lpush(KEY, "Value".to_owned()).unwrap();

            let result = database.llen(KEY);

            assert_eq!(result.unwrap(), format!("{} 1", INTEGER));
        }

        #[test]
        fn test_llen_on_a_list_with_more_than_one_value() {
            let mut database = database_with_a_list();

            let result = database.llen(KEY);

            assert_eq!(result.unwrap(), format!("{} 4", INTEGER));
        }

        #[test]
        fn test_llen_on_an_existent_key_that_isnt_a_list() {
            let mut database = Database::new();

            database.append(KEY, "Value".to_owned()).unwrap();
            let result = database.llen(KEY);

            assert_eq!(
                result.unwrap_err().to_string(),
                DataBaseError::NotAList.to_string()
            );
        }
    }

    mod lindex_test {
        use super::*;

        #[test]
        fn test_lindex_on_a_non_existent_key() {
            let mut database = Database::new();

            let result = database.lindex(KEY, "10".to_owned());

            assert_eq!(result.unwrap(), NIL);
        }

        #[test]
        fn test_lindex_with_a_wrong_idex() {
            let mut database = Database::new();

            let result = database.lindex(KEY, "XX".to_owned());

            assert_eq!(
                result.unwrap_err().to_string(),
                DataBaseError::NotAnInteger.to_string()
            );
        }

        #[test]
        fn test_lindex_with_a_list_with_one_value_on_idex_zero() {
            let mut database = Database::new();
            database.lpush(KEY, "val".to_owned()).unwrap();

            let result = database.lindex(KEY, "0".to_owned());

            assert_eq!(result.unwrap(), "val");
        }

        #[test]
        fn test_lindex_with_more_than_one_value() {
            let mut database = database_with_a_list();

            let result = database.lindex(KEY, "3".to_owned());

            assert_eq!(result.unwrap(), "ValueA");
        }

        #[test]
        fn test_lindex_with_more_than_one_value_with_a_negative_index() {
            let mut database = database_with_a_list();

            let result = database.lindex(KEY, "-1".to_owned());

            assert_eq!(result.unwrap(), "ValueA");
        }

        #[test]
        fn test_lindex_on_a_reverse_out_of_bound() {
            let mut database = database_with_a_list();

            let result = database.lindex(KEY, "-5".to_owned());

            assert_eq!(result.unwrap(), NIL);
        }

        #[test]
        fn test_lindex_on_an_out_of_bound() {
            let mut database = database_with_a_list();

            let result = database.lindex(KEY, "1000".to_owned());

            assert_eq!(result.unwrap(), NIL);
        }
    }

    mod lpop_test {
        use super::*;

        #[test]
        fn test_lpop_on_a_non_existent_key() {
            let mut database = Database::new();
            let result = database.lpop(KEY);
            assert_eq!(result.unwrap(), NIL);
        }

        #[test]
        fn test_lpop_with_a_list_with_one_value_on_idex_zero() {
            let mut database = Database::new();
            database.lpush(KEY, "val".to_owned()).unwrap();
            let result = database.lpop(KEY);

            assert_eq!(result.unwrap(), "val");
            let result = database.llen(KEY);
            assert_eq!(result.unwrap(), format!("{} 0", INTEGER))
        }

        #[test]
        fn test_lpop_with_a_list_with_more_than_one_value() {
            let mut database = database_with_a_list();

            let result = database.lpop(KEY);
            assert_eq!(result.unwrap(), "ValueD");

            if let StorageValue::List(list) = database.dictionary.get(KEY).unwrap() {
                assert_eq!(list[0], "ValueC");
            }
        }

        #[test]
        fn test_lpop_on_an_empty_list() {
            let mut database = Database::new();
            database.lpush(KEY, "val".to_owned()).unwrap();

            let result = database.lpop(KEY);
            assert_eq!(result.unwrap(), "val");

            let result = database.lpop(KEY);

            assert_eq!(result.unwrap(), NIL);

            if let StorageValue::List(list) = database.dictionary.get(KEY).unwrap() {
                assert!(list.is_empty());
            }
        }
    }

    mod lpush_test {

        use super::*;

        #[test]
        fn test_lpush_on_a_non_existent_key_creates_a_list_with_new_value() {
            let mut database = Database::new();

            let result = database.lpush(KEY, "Value".to_owned());

            if let StorageValue::List(list) = database.dictionary.get(KEY).unwrap() {
                assert_eq!(list.len(), 1);
                assert_eq!(list[0], "Value");
                assert_eq!(result.unwrap(), format!("{} 1", INTEGER));
            }
        }

        #[test]
        fn test_lpush_on_an_existent_key_is_valid() {
            let mut database = Database::new();
            let result = database.lpush(KEY, "ValueA".to_owned()).unwrap();
            assert_eq!(result, format!("{} 1", INTEGER));
            let result = database.lpush(KEY, "ValueB".to_owned()).unwrap();
            assert_eq!(result, format!("{} 2", INTEGER));

            if let StorageValue::List(list) = database.dictionary.get(KEY).unwrap() {
                assert_eq!(list.len(), 2);
                assert_eq!(list[0], "ValueB");
                assert_eq!(list[1], "ValueA");
            }
        }

        #[test]
        fn test_lpush_on_an_existent_key_that_isnt_a_list() {
            let mut database = Database::new();
            database.append(KEY, "Value".to_owned()).unwrap();
            let result = database.lpush(KEY, "Value".to_owned());
            assert_eq!(
                result.unwrap_err().to_string(),
                DataBaseError::NotAList.to_string()
            );
        }
    }

    mod lrange_test {
        use super::*;

        #[test]
        fn test_lrange_on_zero_zero_range() {
            let mut database = Database::new();
            database.lpush(KEY, "Value".to_owned()).unwrap();

            let result = database.lrange(KEY, "0".to_owned(), "0".to_owned());

            assert_eq!(result.unwrap(), "1) Value\n")
        }

        #[test]
        fn test_lrange_on_zero_two_range_applied_to_a_list_with_four_elements() {
            let mut database = database_with_a_list();

            let result = database.lrange(KEY, "0".to_owned(), "2".to_owned());
            assert_eq!(result.unwrap(), "1) ValueD\n2) ValueC\n3) ValueB\n");
        }

        #[test]
        fn test_lrange_on_one_three_range_applied_to_a_list_with_four_elements() {
            let mut database = database_with_a_list();

            let result = database.lrange(KEY, "1".to_owned(), "3".to_owned());

            assert_eq!(result.unwrap(), "1) ValueC\n2) ValueB\n3) ValueA\n");
        }

        #[test]
        fn test_lrange_on_a_superior_out_of_bound() {
            let mut database = database_with_a_list();

            let result = database.lrange(KEY, "1".to_owned(), "5".to_owned());

            assert_eq!(result.unwrap(), EMPTY);
        }

        #[test]
        fn test_lrange_on_an_inferior_out_of_bound() {
            let mut database = database_with_a_list();

            let result = database.lrange(KEY, "-10".to_owned(), "3".to_owned());

            assert_eq!(result.unwrap(), EMPTY);
        }

        #[test]
        fn test_lrange_on_an_two_way_out_of_bound() {
            let mut database = database_with_a_list();

            let result = database.lrange(KEY, "-10".to_owned(), "10".to_owned());

            assert_eq!(result.unwrap(), EMPTY);
        }

        #[test]
        fn test_lrange_with_valid_negative_bounds() {
            let mut database = database_with_a_list();

            let result = database.lrange(KEY, "-3".to_owned(), "-1".to_owned());
            assert_eq!(result.unwrap(), "1) ValueC\n2) ValueB\n3) ValueA\n");
        }
    }

    mod lrem_test {
        use super::*;

        #[test]
        fn test_lrem_2_on_a_list() {
            let mut database = database_with_a_three_repeated_values();

            let result = database.lrem(KEY, "2".to_owned(), "ValueA".to_owned());
            assert_eq!(result.unwrap(), format!("{} 2", INTEGER));

            if let StorageValue::List(list) = database.dictionary.get(KEY).unwrap() {
                assert_eq!(list[0], "ValueC");
                assert_eq!(list[1], "ValueA");
            }
        }

        #[test]
        fn test_lrem_negative_2_on_a_list() {
            let mut database = database_with_a_three_repeated_values();

            let result = database.lrem(KEY, "-2".to_owned(), "ValueA".to_owned());
            assert_eq!(result.unwrap(), format!("{} 2", INTEGER));

            if let StorageValue::List(list) = database.dictionary.get(KEY).unwrap() {
                assert_eq!(list[0], "ValueA");
                assert_eq!(list[1], "ValueC");
            }
        }

        #[test]
        fn test_lrem_zero_on_a_list() {
            let mut database = database_with_a_three_repeated_values();

            let result = database.lrem(KEY, "0".to_owned(), "ValueA".to_owned());
            assert_eq!(result.unwrap(), format!("{} 1", INTEGER));

            if let StorageValue::List(list) = database.dictionary.get(KEY).unwrap() {
                assert_eq!(list[0], "ValueC");
            }
        }

        #[test]
        fn test_lrem_with_a_value_higher_than_lenght_of_list() {
            let mut database = database_with_a_three_repeated_values();

            let result = database.lrem(KEY, "10".to_owned(), "ValueA".to_owned());
            assert_eq!(result.unwrap(), format!("{} 1", INTEGER));

            if let StorageValue::List(list) = database.dictionary.get(KEY).unwrap() {
                assert_eq!(list[0], "ValueC");
            }
        }
    }

    mod lset_test {
        use super::*;

        #[test]
        fn test_lset_with_an_index_that_isnt_a_number() {
            let mut database = Database::new();

            let result = database.lset(KEY, "XX", "VAL");

            assert_eq!(
                result.unwrap_err().to_string(),
                DataBaseError::NotAnInteger.to_string()
            );
        }

        #[test]
        fn test_lset_with_a_non_existen_key() {
            let mut database = Database::new();

            let result = database.lset(KEY, "0", "VAL");

            assert_eq!(
                result.unwrap_err().to_string(),
                DataBaseError::NonExistentKey.to_string()
            );
        }

        #[test]
        fn test_lset_with_a_value_that_isn_a_list() {
            let mut database = database_with_a_string();

            let result = database.lset(KEY, "0", "VAL");

            assert_eq!(
                result.unwrap_err().to_string(),
                DataBaseError::NotAList.to_string()
            );
        }

        #[test]
        fn test_lset_on_a_list_with_values() {
            let mut database = database_with_a_list();

            let result = database.lset(KEY, "0", "ValueA");
            assert_eq!(result.unwrap(), SUCCES);

            if let StorageValue::List(list) = database.dictionary.get(KEY).unwrap() {
                assert_eq!(list[0], "ValueA");
            }
        }
    }
}

#[cfg(test)]
mod group_set {
    use super::*;

    #[test]
    fn test_sadd_create_new_set_with_element_returns_1_if_key_set_not_exist_in_database() {
        let mut database = Database::new();

        let result = database.sadd("set1".to_string(), "element".to_string());
        let is_member = database.sismember("set1".to_string(), "element".to_string());

        assert_eq!(is_member.unwrap(), "(integer) 1");
        assert_eq!(result.unwrap(), "(integer) 1");
    }

    #[test]
    fn test_sadd_create_set_with_repeating_elements_returns_0() {
        let mut database = Database::new();

        let _ = database.sadd("set1".to_string(), "element".to_string());
        let result = database.sadd("set1".to_string(), "element".to_string());
        let len_set = database.scard("set1".to_string());

        assert_eq!(len_set.unwrap(), "(integer) 1");
        assert_eq!(result.unwrap(), "(integer) 0");
    }

    #[test]
    fn test_sadd_key_with_another_type_of_set_returns_err() {
        let mut database = Database::new();

        let _ = database.set("key", "value1".to_string());
        let result = database.sadd("key".to_string(), "element".to_string());

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "element of key isn't a Set"
        );
    }

    #[test]
    fn test_sadd_add_element_with_set_created_returns_1() {
        let mut database = Database::new();

        let _ = database.sadd("set1".to_string(), "element".to_string());
        let result = database.sadd("set1".to_string(), "element2".to_string());
        let len_set = database.scard("set1".to_string());

        assert_eq!(len_set.unwrap(), "(integer) 2");
        assert_eq!(result.unwrap(), "(integer) 1");
    }

    #[test]
    fn test_sismember_set_with_element_returns_1() {
        let mut database = Database::new();

        let _ = database.sadd("set1".to_string(), "element".to_string());
        let is_member = database.sismember("set1".to_string(), "element".to_string());

        assert_eq!(is_member.unwrap(), "(integer) 1");
    }

    #[test]
    fn test_sismember_set_without_element_returns_0() {
        let mut database = Database::new();

        let _ = database.sadd("set1".to_string(), "element".to_string());
        let result = database.sismember("set1".to_string(), "other_element".to_string());

        assert_eq!(result.unwrap(), "(integer) 0");
    }

    #[test]
    fn test_sismember_key_with_another_type_of_set_returns_err() {
        let mut database = Database::new();

        let _ = database.set("key", "value1".to_string());
        let result = database.sismember("key".to_string(), "element".to_string());

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "element of key isn't a Set"
        );
    }

    #[test]
    fn test_sismember_with_non_exist_key_set_returns_0() {
        let mut database = Database::new();

        let result = database.sismember("set1".to_string(), "element".to_string());

        assert_eq!(result.unwrap(), "(integer) 0");
    }

    #[test]
    fn test_scard_set_with_one_element_returns_1() {
        let mut database = Database::new();

        let _ = database.sadd("set1".to_string(), "element".to_string());
        let len_set = database.scard("set1".to_string());

        assert_eq!(len_set.unwrap(), "(integer) 1");
    }

    #[test]
    fn test_scard_create_set_with_multiple_elements_returns_lenght_of_set() {
        let mut database = Database::new();

        for i in 0..10 {
            let _ = database.sadd("set1".to_string(), i.to_string());
        }
        let len_set = database.scard("set1".to_string());

        assert_eq!(len_set.unwrap(), "(integer) 10");
    }

    #[test]
    fn test_scard_key_with_another_type_of_set_returns_err() {
        let mut database = Database::new();

        let _ = database.set("key", "value1".to_string());
        let result = database.scard("key".to_string());

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "element of key isn't a Set"
        );
    }

    #[test]
    fn test_scard_key_set_not_exist_in_database_returns_0() {
        let mut database = Database::new();

        let result = database.scard("set1".to_string());
        assert_eq!(result.unwrap(), "(integer) 0");
    }
}

#[cfg(test)]
mod group_server {
    use super::*;

    #[test]
    fn flushdb_clear_dictionary() {
        let mut db = Database::new();
        let _ = db.mset(&[
            "key1".to_string(),
            "1".to_string(),
            "key2".to_string(),
            "2".to_string(),
        ]);
        let r = db.get("key1").unwrap();
        assert_eq!(r, "1");

        let r = db.flushdb().unwrap();
        assert_eq!(r, "Ok");
        assert!(db.dictionary.is_empty());
    }

    #[test]
    fn dbsize_empty_gets_0() {
        let db = Database::new();

        let r = db.dbsize().unwrap();
        assert_eq!(r, "(integer) 0");
    }

    #[test]
    fn dbsize_with_one_element_gets_1() {
        let mut db = Database::new();
        let _ = db.set("key1", "1".to_string());
        let r = db.get("key1").unwrap();
        assert_eq!(r, "1");

        let r = db.dbsize().unwrap();
        assert_eq!(r, "(integer) 1");
    }

    #[test]
    fn dbsize_with_two_element_gets_2() {
        let mut db = Database::new();
        let _ = db.mset(&[
            "key1".to_string(),
            "1".to_string(),
            "key2".to_string(),
            "2".to_string(),
        ]);
        let r = db.get("key1").unwrap();
        assert_eq!(r, "1");
        let r = db.get("key2").unwrap();
        assert_eq!(r, "2");

        let r = db.dbsize().unwrap();
        assert_eq!(r, "(integer) 2");
    }
}
