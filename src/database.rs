use crate::databasehelper::{DataBaseError, StorageValue, SuccessQuery};
use crate::matcher::matcher;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::fmt::Formatter;

pub struct Database {
    dictionary: HashMap<String, StorageValue>,
}

impl Database {
    pub fn new() -> Database {
        Database {
            dictionary: HashMap::new(),
        }
    }

    // SERVER
    pub fn flushdb(&mut self) -> Result<SuccessQuery, DataBaseError> {
        self.dictionary.clear();
        Ok(SuccessQuery::Success)
    }

    pub fn dbsize(&self) -> Result<SuccessQuery, DataBaseError> {
        Ok(SuccessQuery::Integer(self.dictionary.len() as i32))
    }

    // KEYS

    pub fn copy(&mut self, key: &str, to_key: &str) -> Result<SuccessQuery, DataBaseError> {
        if self.dictionary.contains_key(to_key) {
            return Err(DataBaseError::KeyAlredyExist);
        }

        match self.dictionary.get(key) {
            Some(val) => {
                let val = val.clone();
                self.dictionary.insert(to_key.to_owned(), val);
                Ok(SuccessQuery::Success)
            }
            None => Err(DataBaseError::NonExistentKey),
        }
    }

    pub fn del(&mut self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        match self.dictionary.remove(key) {
            Some(_) => Ok(SuccessQuery::Success),
            None => Err(DataBaseError::NonExistentKey),
        }
    }

    pub fn exists(&mut self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        let bool = self.dictionary.contains_key(key);
        Ok(SuccessQuery::Boolean(bool))
    }

    //expire

    //expireat

    pub fn keys(&mut self, pattern: &str) -> Result<SuccessQuery, DataBaseError> {
        let list: Vec<SuccessQuery> = self
            .dictionary
            .keys()
            .filter(|x| matcher(x, pattern))
            .map(|item| SuccessQuery::String(item.to_owned()))
            .collect::<Vec<SuccessQuery>>();

        Ok(SuccessQuery::List(list))
    }

    //persist

    pub fn rename(&mut self, old_key: &str, new_key: &str) -> Result<SuccessQuery, DataBaseError> {
        match self.dictionary.remove(old_key) {
            Some(value) => {
                self.dictionary.insert(new_key.to_owned(), value);
                Ok(SuccessQuery::Success)
            }
            None => Err(DataBaseError::NonExistentKey),
        }
    }

    //sort

    //touch

    //ttl

    //type

    //STRINGS

    pub fn append(&mut self, key: &str, value: &str) -> Result<SuccessQuery, DataBaseError> {
        if self.dictionary.contains_key(key) {
            if let Some(StorageValue::String(val)) = self.dictionary.get_mut(key) {
                val.push_str(&value);
                let len_result = val.len() as i32;
                Ok(SuccessQuery::Integer(len_result))
            } else {
                Err(DataBaseError::NotAString)
            }
        } else {
            let len_result = value.len() as i32;
            match self.set(key, value) {
                Ok(_) => Ok(SuccessQuery::Integer(len_result)),
                Err(err) => Err(err),
            }
        }
    }

    pub fn decrby(&mut self, key: &str, decr: i32) -> Result<SuccessQuery, DataBaseError> {
        if let Some(StorageValue::String(val)) = self.dictionary.get_mut(key) {
            let val = match val.parse::<i32>() {
                Ok(val) => val - decr,
                Err(_) => return Err(DataBaseError::NotAnInteger),
            };

            match self.set(key, &val.to_string()) {
                Ok(_) => Ok(SuccessQuery::Integer(val)),
                Err(err) => Err(err),
            }
        } else {
            Err(DataBaseError::NotAnInteger)
        }
    }

    pub fn get(&mut self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        match self.dictionary.get(key) {
            Some(StorageValue::String(val)) => Ok(SuccessQuery::String(val.clone())),
            Some(_) => Err(DataBaseError::NotAString),
            None => Err(DataBaseError::NonExistentKey),
        }
    }

    pub fn getdel(&mut self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        match self.dictionary.remove(key) {
            Some(StorageValue::String(val)) => Ok(SuccessQuery::String(val)),
            Some(_) => Err(DataBaseError::NotAString),
            None => Err(DataBaseError::NonExistentKey),
        }
    }

    pub fn getset(&mut self, key: &str, new_val: &str) -> Result<SuccessQuery, DataBaseError> {
        let old_value = match self.dictionary.remove(key) {
            Some(StorageValue::String(old_value)) => old_value,
            Some(_) => return Err(DataBaseError::NotAString),
            None => return Err(DataBaseError::NonExistentKey),
        };

        self.set(key, new_val).unwrap();
        Ok(SuccessQuery::String(old_value))
    }

    pub fn incrby(&mut self, key: &str, incr: i32) -> Result<SuccessQuery, DataBaseError> {
        self.decrby(key, -incr)
    }

    pub fn mget(&self, params: Vec<&str>) -> Result<SuccessQuery, DataBaseError> {
        let mut list: Vec<SuccessQuery> = Vec::new();

        for key in params {
            match self.dictionary.get(key) {
                Some(StorageValue::String(value)) => list.push(SuccessQuery::String(value.clone())),
                Some(_) | None => list.push(SuccessQuery::Nil),
            }
        }

        Ok(SuccessQuery::List(list))
    }

    pub fn mset(&mut self, params: Vec<&str>) -> Result<SuccessQuery, DataBaseError> {
        for i in (0..params.len()).step_by(2) {
            let key = params.get(i).unwrap();
            let value = params.get(i + 1).unwrap();

            self.dictionary
                .insert(key.to_string(), StorageValue::String(value.to_string()));
        }
        Ok(SuccessQuery::Success)
    }

    pub fn set(&mut self, key: &str, val: &str) -> Result<SuccessQuery, DataBaseError> {
        self.dictionary
            .insert(key.to_owned(), StorageValue::String(val.to_owned()));
        Ok(SuccessQuery::Success)
    }

    pub fn strlen(&mut self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        if self.dictionary.contains_key(key) {
            if let Some(StorageValue::String(val)) = self.dictionary.get_mut(key) {
                Ok(SuccessQuery::Integer(val.len() as i32))
            } else {
                Err(DataBaseError::NotAString)
            }
        } else {
            Ok(SuccessQuery::Integer(0))
        }
    }

    // Lists

    pub fn lindex(&mut self, key: &str, index: i32) -> Result<SuccessQuery, DataBaseError> {
        match self.dictionary.get(key) {
            Some(StorageValue::List(list)) => {
                let index = if index < 0 {
                    ((list.len() as i32) + index) as usize
                } else {
                    index as usize
                };

                match list.get(index) {
                    Some(val) => Ok(SuccessQuery::String(val.clone())),
                    None => Ok(SuccessQuery::Nil),
                }
            }
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(SuccessQuery::Nil),
        }
    }

    pub fn llen(&mut self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        match self.dictionary.get(key) {
            Some(StorageValue::List(list)) => Ok(SuccessQuery::Integer(list.len() as i32)),
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(SuccessQuery::Integer(0)),
        }
    }

    pub fn lpop(&mut self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        match self.dictionary.get_mut(key) {
            Some(StorageValue::List(list)) => {
                if list.is_empty() {
                    Ok(SuccessQuery::Nil)
                } else {
                    Ok(SuccessQuery::String(list.remove(0)))
                }
            }
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(SuccessQuery::Nil),
        }
    }

    pub fn lpush(&mut self, key: &str, value: &str) -> Result<SuccessQuery, DataBaseError> {
        match self.dictionary.get_mut(key) {
            Some(StorageValue::List(list)) => {
                list.insert(0, value.to_owned());
                Ok(SuccessQuery::Integer(list.len() as i32))
            }
            Some(_) => Err(DataBaseError::NotAList),
            None => {
                let list: Vec<String> = vec![value.to_owned()];
                let len = list.len();
                self.dictionary
                    .insert(key.to_owned(), StorageValue::List(list));
                Ok(SuccessQuery::Integer(len as i32))
            }
        }
    }

    pub fn lpushx(&mut self, key: &str, value: &str) -> Result<SuccessQuery, DataBaseError> {
        match self.dictionary.get_mut(key) {
            Some(StorageValue::List(list)) => {
                list.insert(0, value.to_owned());
                Ok(SuccessQuery::Integer(list.len() as i32))
            }
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(SuccessQuery::Integer(0)),
        }
    }

    pub fn lrange(&mut self, key: &str, ini: i32, end: i32) -> Result<SuccessQuery, DataBaseError> {
        let mut sub_list: Vec<SuccessQuery> = Vec::new();

        match self.dictionary.get(key) {
            Some(StorageValue::List(list)) => {
                let len = list.len() as i32;
                let ini = if ini < 0 {
                    (len + ini) as usize
                } else {
                    ini as usize
                };
                let end = if end < 0 {
                    (len + end) as usize
                } else {
                    end as usize
                };

                if end < len as usize && ini <= end {
                    for elem in list[ini..(end + 1)].iter() {
                        sub_list.push(SuccessQuery::String(elem.clone()));
                    }

                    Ok(SuccessQuery::List(sub_list))
                } else {
                    Ok(SuccessQuery::List(sub_list))
                }
            }
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(SuccessQuery::List(sub_list)),
        }
    }

    pub fn lrem(
        &mut self,
        key: &str,
        mut count: i32,
        elem: &str,
    ) -> Result<SuccessQuery, DataBaseError> {
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

                    Ok(SuccessQuery::Integer(list.len() as i32))
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

                    Ok(SuccessQuery::Integer(list.len() as i32))
                }
                Ordering::Equal => {
                    list.retain(|x| *x != elem);
                    Ok(SuccessQuery::Integer(list.len() as i32))
                }
            },
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(SuccessQuery::Integer(0)),
        }
    }

    pub fn lset(
        &mut self,
        key: &str,
        index: usize,
        value: &str,
    ) -> Result<SuccessQuery, DataBaseError> {
        match self.dictionary.get_mut(key) {
            Some(StorageValue::List(list)) => match list.get_mut(index) {
                Some(val) => {
                    val.clear();
                    val.push_str(value);
                    Ok(SuccessQuery::Success)
                }
                None => Err(DataBaseError::IndexOutOfRange),
            },
            Some(_) => Err(DataBaseError::NotAList),
            None => Err(DataBaseError::NonExistentKey),
        }
    }

    pub fn rpop(&mut self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        match self.dictionary.get_mut(key) {
            Some(StorageValue::List(list)) => match list.pop() {
                Some(value) => Ok(SuccessQuery::String(value)),
                None => Ok(SuccessQuery::Nil),
            },
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(SuccessQuery::Nil),
        }
    }

    pub fn rpush(&mut self, key: &str, value: &str) -> Result<SuccessQuery, DataBaseError> {
        match self.dictionary.get_mut(key) {
            Some(StorageValue::List(list)) => {
                list.push(value.to_owned());
                Ok(SuccessQuery::Integer(value.len() as i32))
            }
            Some(_) => Err(DataBaseError::NotAList),
            None => {
                let list: Vec<String> = vec![value.to_owned()];
                let len = list.len();
                self.dictionary
                    .insert(key.to_owned(), StorageValue::List(list));
                Ok(SuccessQuery::Integer(len as i32))
            }
        }
    }

    pub fn rpushx(&mut self, key: &str, value: &str) -> Result<SuccessQuery, DataBaseError> {
        match self.dictionary.get_mut(key) {
            Some(StorageValue::List(list)) => {
                list.push(value.to_owned());
                Ok(SuccessQuery::Integer(value.len() as i32))
            }
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(SuccessQuery::Boolean(false)),
        }
    }

    //SETS

    pub fn sismember(&mut self, key: &str, value: &str) -> Result<SuccessQuery, DataBaseError> {
        match self.dictionary.get_mut(key) {
            Some(StorageValue::Set(hash_set)) => match hash_set.get(value) {
                Some(_val) => Ok(SuccessQuery::Boolean(true)),
                None => Ok(SuccessQuery::Boolean(false)),
            },
            Some(_) => Err(DataBaseError::NotASet),
            None => Ok(SuccessQuery::Boolean(false)),
        }
    }

    pub fn scard(&mut self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        match self.dictionary.get_mut(key) {
            Some(StorageValue::Set(hash_set)) => Ok(SuccessQuery::Integer(hash_set.len() as i32)),
            Some(_) => Err(DataBaseError::NotASet),
            None => Ok(SuccessQuery::Boolean(false)),
        }
    }

    pub fn sadd(&mut self, key: &str, value: &str) -> Result<SuccessQuery, DataBaseError> {
        match self.dictionary.get_mut(key) {
            Some(StorageValue::Set(hash_set)) => {
                if hash_set.contains(value) {
                    Ok(SuccessQuery::Boolean(false))
                } else {
                    hash_set.insert(value.to_owned());
                    Ok(SuccessQuery::Boolean(true))
                }
            }
            Some(_) => Err(DataBaseError::NotASet),
            None => {
                let mut set: HashSet<String> = HashSet::new();
                set.insert(value.to_owned());
                self.dictionary
                    .insert(key.to_owned(), StorageValue::Set(set));
                Ok(SuccessQuery::Boolean(true))
            }
        }
    }

    pub fn smembers(&mut self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        let mut result: Vec<SuccessQuery> = Vec::new();
        match self.dictionary.get_mut(key) {
            Some(StorageValue::Set(hash_set)) => {
                for elem in hash_set.iter() {
                    result.push(SuccessQuery::String(elem.clone()));
                }
                Ok(SuccessQuery::List(result))
            }
            Some(_) => Err(DataBaseError::NotASet),
            None => Ok(SuccessQuery::List(result)),
        }
    }

    pub fn srem(
        &mut self,
        key: &str,
        members_to_rmv: Vec<&str>,
    ) -> Result<SuccessQuery, DataBaseError> {
        match self.dictionary.get_mut(key) {
            Some(StorageValue::Set(hash_set)) => {
                let mut count: i32 = 0;
                for member in members_to_rmv {
                    if let Some(_mem) = hash_set.take(member) {
                        count += 1;
                    }
                }
                Ok(SuccessQuery::Integer(count))
            }
            Some(_) => Err(DataBaseError::NotASet),
            None => Ok(SuccessQuery::Boolean(false)),
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
    const VALUE: &str = "VALUE";

    fn create_database_with_string() -> Database {
        let mut database = Database::new();

        database.set(KEY, VALUE).unwrap();

        database
    }

    mod append_test {

        use super::*;

        #[test]
        fn test_append_new_key_return_lenght_of_the_value() {
            let mut database = Database::new();

            if let SuccessQuery::Integer(lenght) = database.append(KEY, VALUE).unwrap() {
                assert_eq!(lenght, 5);
            }
        }

        #[test]
        fn test_append_key_with_old_value_return_lenght_of_the_total_value() {
            let mut database = Database::new();

            if let SuccessQuery::Integer(lenght) = database.append(KEY, VALUE).unwrap() {
                assert_eq!(lenght, 5);
                if let SuccessQuery::Integer(lenght) = database.append(KEY, VALUE).unwrap() {
                    assert_eq!(lenght, 10);
                }
            }
        }
    }

    mod get_test {

        use super::*;

        #[test]
        fn test_get_returns_value_of_key() {
            let mut database = create_database_with_string();

            if let SuccessQuery::String(value) = database.get(KEY).unwrap() {
                assert_eq!(VALUE, value.to_string());
            }
        }

        #[test]
        fn test_get_returns_error_if_the_key_does_not_exist() {
            let mut database = Database::new();

            let result = database.get(KEY).unwrap_err();

            assert_eq!(result, DataBaseError::NonExistentKey);
        }
    }

    mod set_test {
        use super::*;

        #[test]
        fn test_set_returns_ok() {
            let mut database = Database::new();

            let result = database.set(KEY, VALUE).unwrap();
            assert_eq!(SuccessQuery::Success, result);

            if let SuccessQuery::String(value) = database.get(KEY).unwrap() {
                assert_eq!(VALUE, value.to_string());
            }
        }
    }

    mod getdel_test {
        use super::*;

        #[test]
        fn test_getdel_returns_old_value_of_key_and_after_value_of_key_what_not_exists_in_database_is_nul(
        ) {
            let mut database = create_database_with_string();

            if let SuccessQuery::String(value) = database.getdel(KEY).unwrap() {
                assert_eq!(value.to_string(), VALUE);
            }

            let result = database.get(KEY).unwrap_err();
            assert_eq!(result, DataBaseError::NonExistentKey);
        }

        #[test]
        fn test_getdel_returns_nil_value_if_key_not_exist_in_database() {
            let mut database = Database::new();

            let result = database.getdel(KEY).unwrap_err();
            assert_eq!(result, DataBaseError::NonExistentKey);
        }
    }

    mod incrby_test {
        use super::*;

        #[test]
        fn test_incrby_returns_lenght_of_the_resulting_value_after_increment() {
            let mut database = Database::new();
            database.set(KEY, "1").unwrap();

            let result = database.incrby(KEY, 4).unwrap();

            assert_eq!(result, SuccessQuery::Integer(5));
        }
    }

    mod decrby_test {
        use super::*;

        #[test]
        fn test_decrby_returns_lenght_of_the_resulting_value_after_increment() {
            let mut database = Database::new();
            database.set(KEY, "5").unwrap();
            let result = database.decrby(KEY, 4).unwrap();

            assert_eq!(result, SuccessQuery::Integer(1));
        }
    }

    mod strlen_test {
        use super::*;

        #[test]
        fn test_strlen_returns_the_lenght_of_value_key() {
            let mut database = Database::new();
            database.set(KEY, VALUE).unwrap();

            if let SuccessQuery::Integer(value) = database.strlen(KEY).unwrap() {
                assert_eq!(value, 5);
            }
        }

        #[test]
        fn test_strlen_returns_zero_if_the_key_not_exists_in_database() {
            let mut database = Database::new();
            if let SuccessQuery::Integer(value) = database.strlen(KEY).unwrap() {
                assert_eq!(value, 0);
            }
        }
    }

    mod mset_test {
        use super::*;

        const KEY_A: &str = "KEY_A";
        const VALUE_A: &str = "VALUE_A";

        const KEY_B: &str = "KEY_B";
        const VALUE_B: &str = "VALUE_B";

        const KEY_C: &str = "KEY_C";
        const VALUE_C: &str = "VALUE_C";

        const KEY_D: &str = "KEY_D";
        const VALUE_D: &str = "VALUE_D";

        #[test]
        fn test_mset_set_multiple_key_and_value_ok() {
            let mut database = Database::new();
            let vec_key_value = vec![
                KEY_A, VALUE_A, KEY_B, VALUE_B, KEY_C, VALUE_C, KEY_D, VALUE_D,
            ];

            let result = database.mset(vec_key_value).unwrap();
            assert_eq!(result, SuccessQuery::Success);

            let result_get1 = database.get(KEY_A).unwrap();
            let result_get2 = database.get(KEY_B).unwrap();
            let result_get3 = database.get(KEY_C).unwrap();
            let result_get4 = database.get(KEY_D).unwrap();

            assert_eq!(result_get1, SuccessQuery::String(VALUE_A.to_owned()));
            assert_eq!(result_get2, SuccessQuery::String(VALUE_B.to_owned()));
            assert_eq!(result_get3, SuccessQuery::String(VALUE_C.to_owned()));
            assert_eq!(result_get4, SuccessQuery::String(VALUE_D.to_owned()));
        }
    }

    mod mget_test {
        use super::*;

        const KEY_A: &str = "KEY_A";
        const VALUE_A: &str = "VALUE_A";

        const KEY_B: &str = "KEY_B";
        const VALUE_B: &str = "VALUE_B";

        const KEY_C: &str = "KEY_C";
        const VALUE_C: &str = "VALUE_C";

        const KEY_D: &str = "KEY_D";
        const VALUE_D: &str = "VALUE_D";

        fn create_a_database_with_key_values() -> Database {
            let mut database = Database::new();

            database.set(KEY_A, VALUE_A).unwrap();
            database.set(KEY_B, VALUE_B).unwrap();
            database.set(KEY_C, VALUE_C).unwrap();
            database.set(KEY_D, VALUE_D).unwrap();

            database
        }

        #[test]
        fn test_mget_return_all_values_of_keys_if_all_keys_are_in_database() {
            let database = create_a_database_with_key_values();

            let vec_keys = vec![KEY_A, KEY_B, KEY_C, KEY_D];

            if let SuccessQuery::List(list) = database.mget(vec_keys).unwrap() {
                assert_eq!(list[0], SuccessQuery::String(VALUE_A.to_owned()));
                assert_eq!(list[1], SuccessQuery::String(VALUE_B.to_owned()));
                assert_eq!(list[2], SuccessQuery::String(VALUE_C.to_owned()));
                assert_eq!(list[3], SuccessQuery::String(VALUE_D.to_owned()));
            }
        }

        #[test]

        fn test_mget_returns_nil_for_some_key_that_no_exists_in_database() {
            let mut database = Database::new();
            database.set(KEY_A, VALUE_A).unwrap();
            database.set(KEY_B, VALUE_B).unwrap();

            let vec_keys = vec![KEY_A, KEY_B, KEY_C, KEY_D];

            if let SuccessQuery::List(list) = database.mget(vec_keys).unwrap() {
                assert_eq!(list[0], SuccessQuery::String(VALUE_A.to_owned()));
                assert_eq!(list[1], SuccessQuery::String(VALUE_B.to_owned()));
                assert_eq!(list[2], SuccessQuery::Nil);
                assert_eq!(list[3], SuccessQuery::Nil);
            }
        }
    }
}

#[cfg(test)]
mod group_keys {
    use super::*;

    const KEY: &str = "KEY";
    const SECOND_KEY: &str = "SECOND_KEY";

    const VALUE: &str = "VALUE";
    const SECOND_VALUE: &str = "SECOND_VALUE";

    mod copy_test {
        use super::*;

        #[test]
        fn test_copy_set_dolly_sheep_then_copy_to_clone() {
            let mut database = Database::new();

            database.set(KEY, VALUE).unwrap();
            let result = database.copy(KEY, SECOND_KEY);
            assert_eq!(result.unwrap(), SuccessQuery::Success);
            if let SuccessQuery::String(value) = database.strlen(SECOND_KEY).unwrap() {
                assert_eq!(value.to_string(), VALUE);
            }
        }

        #[test]
        fn test_copy_set_dolly_sheep_then_copy_to_clone_when_clone_exist() {
            let mut database = Database::new();
            database.set(KEY, VALUE).unwrap();
            database.set(SECOND_KEY, SECOND_VALUE).unwrap();
            let result = database.copy(KEY, SECOND_KEY);

            assert_eq!(result.unwrap_err(), DataBaseError::KeyAlredyExist);
        }

        #[test]
        fn test_copy_try_to_copy_a_key_does_not_exist() {
            let mut database = Database::new();
            let result = database.copy(KEY, SECOND_KEY);

            assert_eq!(result.unwrap_err(), DataBaseError::NonExistentKey);
        }
    }

    mod del_test {
        use super::*;

        #[test]
        fn test_del_key_value_returns_succes() {
            let mut database = Database::new();
            database.set(KEY, VALUE).unwrap();

            let result = database.del(KEY);
            assert_eq!(result.unwrap(), SuccessQuery::Success);
            assert_eq!(
                database.get(KEY).unwrap_err(),
                DataBaseError::NonExistentKey
            );
        }
        #[test]
        fn test_del_key_non_exist_returns_error() {
            let mut database = Database::new();

            let result = database.del(KEY);
            assert_eq!(result.unwrap_err(), DataBaseError::NonExistentKey);
        }
    }

    mod exists_test {

        use super::*;

        #[test]
        fn test_exists_key_non_exist_returns_bool_false() {
            let mut database = Database::new();

            let result = database.exists(KEY);
            assert_eq!(result.unwrap(), SuccessQuery::Boolean(false));
            assert_eq!(
                database.get(KEY).unwrap_err(),
                DataBaseError::NonExistentKey
            );
        }

        #[test]
        fn test_exists_key_hello_returns_bool_true() {
            let mut database = Database::new();
            database.set(KEY, VALUE).unwrap();

            let result = database.exists(KEY);

            assert_eq!(result.unwrap(), SuccessQuery::Boolean(true));

            if let SuccessQuery::String(value) = database.strlen(KEY).unwrap() {
                assert_eq!(value.to_string(), VALUE);
            }
        }
    }

    mod keys_test {
        use super::*;

        const FIRST_NAME: &str = "firstname";
        const LAST_NAME: &str = "lastname";
        const AGE: &str = "age";

        const FIRST_NAME_VALUE: &str = "Alex";
        const LAST_NAME_VALUE: &str = "Arbieto";
        const AGE_VALUE: &str = "22";

        fn create_database_with_keys() -> Database {
            let mut database = Database::new();
            database.set(FIRST_NAME, FIRST_NAME_VALUE).unwrap();
            database.set(LAST_NAME, LAST_NAME_VALUE).unwrap();
            database.set(AGE, AGE_VALUE).unwrap();
            database
        }

        #[test]
        fn test_keys_obtain_keys_with_name() {
            let mut database = create_database_with_keys();
            if let Ok(SuccessQuery::List(list)) = database.keys("*name") {
                let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();

                assert!(list.contains(&FIRST_NAME.to_owned()));
                assert!(list.contains(&LAST_NAME.to_owned()));
            }
        }

        #[test]
        fn test_keys_obtain_keys_with_four_question_name() {
            let mut database = create_database_with_keys();
            if let Ok(SuccessQuery::List(list)) = database.keys("????name") {
                let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();

                assert!(list.contains(&LAST_NAME.to_owned()));
            }
        }

        const KEY_A: &str = "key";
        const KEY_B: &str = "keeeey";
        const KEY_C: &str = "ky";
        const NO_MATCH: &str = "notmatch";

        const VAL_A: &str = "valA";
        const VAL_B: &str = "valB";
        const VAL_C: &str = "valC";
        const VAL_D: &str = "valD";

        fn create_database_with_keys_two() -> Database {
            let mut database = Database::new();
            database.set(KEY_A, VAL_A).unwrap();
            database.set(KEY_B, VAL_B).unwrap();
            database.set(KEY_C, VAL_C).unwrap();
            database.set(NO_MATCH, VAL_D).unwrap();
            database
        }

        #[test]
        fn test_keys_obtain_all_keys_with_an_asterisk_in_the_middle() {
            let mut database = create_database_with_keys_two();

            if let Ok(SuccessQuery::List(list)) = database.keys("k*y") {
                let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();

                assert!(list.contains(&KEY_A.to_owned()));
                assert!(list.contains(&KEY_B.to_owned()));
                assert!(list.contains(&KEY_C.to_owned()));
            }
        }

        #[test]
        fn test_keys_obtain_all_keys_with_question_in_the_middle() {
            let mut database = create_database_with_keys_two();

            if let Ok(SuccessQuery::List(list)) = database.keys("k?y") {
                let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();

                assert!(list.contains(&KEY_A.to_owned()));
            }
        }

        const HALL: &str = "hall";
        const HELLO: &str = "hello";
        const HEEEELLO: &str = "heeeeeello";
        const HALLO: &str = "hallo";
        const HXLLO: &str = "hxllo";
        const HLLO: &str = "hllo";
        const RHLLO: &str = r"h\llo";
        const AHLLO: &str = "ahllo";
        const HALLOWN: &str = "hallown";

        fn create_database_with_keys_three() -> Database {
            let mut database = Database::new();
            database.set(HELLO, VAL_A).unwrap();
            database.set(HEEEELLO, VAL_A).unwrap();
            database.set(HALLO, VAL_B).unwrap();
            database.set(HXLLO, VAL_C).unwrap();
            database.set(HLLO, VAL_C).unwrap();
            database.set(RHLLO, VAL_D).unwrap();
            database.set(AHLLO, VAL_D).unwrap();
            database.set(HALLOWN, VAL_D).unwrap();
            database.set(HALL, VAL_D).unwrap();

            database
        }

        #[test]
        fn test_keys_obtain_keys_with_h_question_llo_matches_correctly() {
            let mut database = create_database_with_keys_three();

            if let Ok(SuccessQuery::List(list)) = database.keys("h?llo") {
                let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();

                assert!(list.contains(&HELLO.to_owned()));
                assert!(list.contains(&HXLLO.to_owned()));
                assert!(list.contains(&RHLLO.to_owned()));
                assert!(list.contains(&HXLLO.to_owned()));
                assert!(list.contains(&RHLLO.to_owned()));
            }
        }

        #[test]
        fn test_keys_obtain_keys_with_h_asterisk_llo_matches_correctly() {
            let mut database = create_database_with_keys_three();

            if let Ok(SuccessQuery::List(list)) = database.keys("h*llo") {
                let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();

                assert!(list.contains(&HELLO.to_owned()));
                assert!(list.contains(&HALLO.to_owned()));
                assert!(list.contains(&HEEEELLO.to_owned()));
                assert!(list.contains(&HELLO.to_owned()));
            }
        }

        #[test]
        fn test_keys_obtain_keys_with_nomatch_returns_empty_list() {
            let mut database = create_database_with_keys();

            if let SuccessQuery::List(list) = database.keys("nomatch").unwrap() {
                assert_eq!(list.len(), 0);
            }
        }
    }

    mod rename_test {
        use super::*;

        #[test]
        fn test_rename_key_with_mykey_get_hello() {
            let mut database = Database::new();
            database.set(KEY, VALUE).unwrap();

            let result = database.rename(KEY, SECOND_KEY).unwrap();
            assert_eq!(result, SuccessQuery::Success);

            let result = database.get(KEY).unwrap_err();
            assert_eq!(result, DataBaseError::NonExistentKey);
        }

        #[test]
        fn test_rename_key_non_exists_error() {
            let mut database = Database::new();

            let result = database.rename(KEY, SECOND_KEY).unwrap_err();

            assert_eq!(result, DataBaseError::NonExistentKey);
        }
    }
}

#[cfg(test)]
mod group_list {

    const KEY: &str = "KEY";
    const VALUE: &str = "VALUE";

    const VALUEA: &str = "ValueA";
    const VALUEB: &str = "ValueB";
    const VALUEC: &str = "ValueC";
    const VALUED: &str = "ValueD";

    use super::*;

    fn database_with_a_list() -> Database {
        let mut database = Database::new();

        database.lpush(KEY, VALUEA).unwrap();
        database.lpush(KEY, VALUEB).unwrap();
        database.lpush(KEY, VALUEC).unwrap();
        database.lpush(KEY, VALUED).unwrap();

        database
    }

    fn database_with_a_three_repeated_values() -> Database {
        let mut database = Database::new();
        database.lpush(KEY, VALUEA).unwrap();
        database.lpush(KEY, VALUEA).unwrap();
        database.lpush(KEY, VALUEC).unwrap();
        database.lpush(KEY, VALUEA).unwrap();

        database
    }

    fn database_with_a_string() -> Database {
        let mut database = Database::new();
        database.append(KEY, VALUE).unwrap();
        database
    }

    mod llen_test {

        use super::*;

        #[test]
        fn test_llen_on_a_non_existent_key_gets_len_zero() {
            let mut database = Database::new();

            let result = database.llen(KEY);

            assert_eq!(result.unwrap(), SuccessQuery::Integer(0));
        }

        #[test]
        fn test_llen_on_a_list_with_one_value() {
            let mut database = Database::new();
            database.lpush(KEY, VALUE).unwrap();

            let result = database.llen(KEY);

            assert_eq!(result.unwrap(), SuccessQuery::Integer(1));
        }

        #[test]
        fn test_llen_on_a_list_with_more_than_one_value() {
            let mut database = database_with_a_list();

            let result = database.llen(KEY).unwrap();

            assert_eq!(result, SuccessQuery::Integer(4));
        }

        #[test]
        fn test_llen_on_an_existent_key_that_isnt_a_list() {
            let mut database = database_with_a_string();

            let result = database.llen(KEY);

            assert_eq!(result.unwrap_err(), DataBaseError::NotAList);
        }
    }

    mod lindex_test {
        use super::*;

        #[test]
        fn test_lindex_on_a_non_existent_key() {
            let mut database = Database::new();

            let result = database.lindex(KEY, 10);

            assert_eq!(result.unwrap(), SuccessQuery::Nil);
        }

        #[test]
        fn test_lindex_with_a_list_with_one_value_on_idex_zero() {
            let mut database = Database::new();
            database.lpush(KEY, VALUE).unwrap();

            if let SuccessQuery::String(value) = database.lindex(KEY, 0).unwrap() {
                assert_eq!(value, VALUE);
            }
        }

        #[test]
        fn test_lindex_with_more_than_one_value() {
            let mut database = database_with_a_list();

            if let SuccessQuery::String(value) = database.lindex(KEY, 3).unwrap() {
                assert_eq!(value, VALUEA);
            }
        }

        #[test]
        fn test_lindex_with_more_than_one_value_with_a_negative_index() {
            let mut database = database_with_a_list();

            if let SuccessQuery::String(value) = database.lindex(KEY, -1).unwrap() {
                assert_eq!(value, VALUEA);
            }
        }

        #[test]
        fn test_lindex_on_a_reverse_out_of_bound() {
            let mut database = database_with_a_list();

            let result = database.lindex(KEY, -5);

            assert_eq!(result.unwrap(), SuccessQuery::Nil);
        }

        #[test]
        fn test_lindex_on_an_out_of_bound() {
            let mut database = database_with_a_list();

            let result = database.lindex(KEY, 100);

            assert_eq!(result.unwrap(), SuccessQuery::Nil);
        }
    }

    mod lpop_test {
        use super::*;

        #[test]
        fn test_lpop_on_a_non_existent_key() {
            let mut database = Database::new();

            let result = database.lpop(KEY).unwrap();
            assert_eq!(result, SuccessQuery::Nil);
        }

        #[test]
        fn test_lpop_with_a_list_with_one_value_on_idex_zero() {
            let mut database = Database::new();
            database.lpush(KEY, VALUE).unwrap();

            if let SuccessQuery::String(val) = database.lpop(KEY).unwrap() {
                assert_eq!(val.to_string(), VALUE);
            }

            let result = database.llen(KEY).unwrap();
            assert_eq!(result, SuccessQuery::Integer(0));
        }

        #[test]
        fn test_lpop_with_a_list_with_more_than_one_value() {
            let mut database = database_with_a_list();

            if let SuccessQuery::String(val) = database.lpop(KEY).unwrap() {
                assert_eq!(val.to_string(), VALUED);
            }

            if let StorageValue::List(list) = database.dictionary.get(KEY).unwrap() {
                assert_eq!(list[0], VALUEC);
            }
        }

        #[test]
        fn test_lpop_on_an_empty_list() {
            let mut database = Database::new();
            database.lpush(KEY, VALUE).unwrap();

            if let SuccessQuery::String(val) = database.lpop(KEY).unwrap() {
                assert_eq!(val.to_string(), VALUE);
            }

            let value = database.lpop(KEY).unwrap();
            assert_eq!(value, SuccessQuery::Nil);

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

            let result = database.lpush(KEY, VALUE).unwrap();
            assert_eq!(result, SuccessQuery::Integer(1));

            if let StorageValue::List(list) = database.dictionary.get(KEY).unwrap() {
                assert_eq!(list.len(), 1);
                assert_eq!(list[0], VALUE);
            }
        }

        #[test]
        fn test_lpush_on_an_existent_key_is_valid() {
            let mut database = Database::new();
            database.lpush(KEY, VALUEA).unwrap();
            let result = database.lpush(KEY, VALUEB).unwrap();

            assert_eq!(result, SuccessQuery::Integer(2));

            if let StorageValue::List(list) = database.dictionary.get(KEY).unwrap() {
                assert_eq!(list.len(), 2);
                assert_eq!(list[1], VALUEA);
                assert_eq!(list[0], VALUEB);
            }
        }

        #[test]
        fn test_lpush_on_an_existent_key_that_isnt_a_list() {
            let mut database = Database::new();
            database.append(KEY, VALUE).unwrap();

            let result = database.lpush(KEY, VALUE).unwrap_err();

            assert_eq!(result, DataBaseError::NotAList);
        }
    }

    mod lrange_test {
        use super::*;

        #[test]
        fn test_lrange_on_zero_zero_range() {
            let mut database = Database::new();
            database.lpush(KEY, VALUE).unwrap();

            if let SuccessQuery::List(list) = database.lrange(KEY, 0, 0).unwrap() {
                assert_eq!(list[0].to_string(), VALUE);
            }
        }

        #[test]
        fn test_lrange_on_zero_two_range_applied_to_a_list_with_four_elements() {
            let mut database = database_with_a_list();

            if let SuccessQuery::List(list) = database.lrange(KEY, 0, 2).unwrap() {
                let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();
                let second_list: Vec<&str> = vec![VALUED, VALUEC, VALUEB];
                let pair_list: Vec<(&String, &str)> = list.iter().zip(second_list).collect();
                pair_list.iter().for_each(|x| {
                    assert_eq!(x.0, x.1);
                })
            }
        }

        #[test]
        fn test_lrange_on_one_three_range_applied_to_a_list_with_four_elements() {
            let mut database = database_with_a_list();

            if let SuccessQuery::List(list) = database.lrange(KEY, 1, 3).unwrap() {
                let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();
                let second_list: Vec<&str> = vec![VALUEC, VALUEB, VALUEA];
                let pair_list: Vec<(&String, &str)> = list.iter().zip(second_list).collect();
                pair_list.iter().for_each(|x| {
                    assert_eq!(x.0, x.1);
                })
            }
        }

        #[test]
        fn test_lrange_on_a_superior_out_of_bound() {
            let mut database = database_with_a_list();

            let result = database.lrange(KEY, 1, 5).unwrap();

            assert_eq!(result, SuccessQuery::List(Vec::new()));
        }

        #[test]
        fn test_lrange_on_an_inferior_out_of_bound() {
            let mut database = database_with_a_list();

            let result = database.lrange(KEY, -10, 3).unwrap();

            assert_eq!(result, SuccessQuery::List(Vec::new()));
        }

        #[test]
        fn test_lrange_on_an_two_way_out_of_bound() {
            let mut database = database_with_a_list();

            let result = database.lrange(KEY, -10, 10).unwrap();

            assert_eq!(result, SuccessQuery::List(Vec::new()));
        }

        #[test]
        fn test_lrange_with_valid_negative_bounds() {
            let mut database = database_with_a_list();

            if let SuccessQuery::List(list) = database.lrange(KEY, -3, -1).unwrap() {
                let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();
                let second_list: Vec<&str> = vec![VALUEC, VALUEB, VALUEA];
                let pair_list: Vec<(&String, &str)> = list.iter().zip(second_list).collect();
                pair_list.iter().for_each(|x| {
                    assert_eq!(x.0, x.1);
                })
            }
        }
    }

    mod lrem_test {
        use super::*;

        #[test]
        fn test_lrem_2_on_a_list() {
            let mut database = database_with_a_three_repeated_values();

            let result = database.lrem(KEY, 2, VALUEA);

            assert_eq!(SuccessQuery::Integer(2), result.unwrap());

            if let Some(StorageValue::List(list)) = database.dictionary.get(KEY) {
                assert_eq!(list[0], VALUEC);
                assert_eq!(list[1], VALUEA);
            }
        }

        #[test]
        fn test_lrem_negative_2_on_a_list() {
            let mut database = database_with_a_three_repeated_values();

            let result = database.lrem(KEY, -2, VALUEA);

            assert_eq!(SuccessQuery::Integer(2), result.unwrap());
            if let StorageValue::List(list) = database.dictionary.get(KEY).unwrap() {
                assert_eq!(list[0], VALUEA);
                assert_eq!(list[1], VALUEC);
            }
        }

        #[test]
        fn test_lrem_zero_on_a_list() {
            let mut database = database_with_a_three_repeated_values();

            let result = database.lrem(KEY, 0, VALUEA);

            assert_eq!(SuccessQuery::Integer(1), result.unwrap());
            if let StorageValue::List(list) = database.dictionary.get(KEY).unwrap() {
                assert_eq!(list[0], VALUEC);
            }
        }

        #[test]
        fn test_lset_with_a_non_existen_key() {
            let mut database = Database::new();

            let result = database.lset(KEY, 0, &VALUEA).unwrap_err();

            assert_eq!(result, DataBaseError::NonExistentKey);
        }

        #[test]
        fn test_lset_with_a_value_that_isn_a_list() {
            let mut database = database_with_a_string();

            let result = database.lset(KEY, 0, &VALUEA).unwrap_err();

            assert_eq!(result, DataBaseError::NotAList);
        }

        #[test]
        fn test_lset_on_a_list_with_values() {
            let mut database = database_with_a_list();

            let result = database.lset(KEY, 0, &VALUEA);
            assert_eq!(SuccessQuery::Success, result.unwrap());

            if let StorageValue::List(list) = database.dictionary.get(KEY).unwrap() {
                assert_eq!(list[0], VALUEA);
            }
        }
    }
}

#[cfg(test)]
mod group_set {
    use super::*;

    const KEY: &str = "KEY";
    const KEY_WITH_STR: &str = "KEY_WITH_STRING";
    const VALUE_A: &str = "VALUE_A";
    const NON_EXIST_KEY: &str = "NON_EXIST_KEY";
    const NON_EXIST_ELEMENT: &str = "NON_EXIST_ELEMENT";
    const ELEMENT: &str = "ELEMENT";
    const ELEMENT_2: &str = "ELEMENT2";
    const ELEMENT_3: &str = "ELEMENT3";
    const OTHER_ELEMENT: &str = "OTHER_ELEMENT";

    mod saad_test {
        use super::*;

        #[test]
        fn test_sadd_create_new_set_with_element_returns_1_if_key_set_not_exist_in_database() {
            let mut database = Database::new();

            let result = database.sadd(KEY, ELEMENT).unwrap();
            assert_eq!(result, SuccessQuery::Boolean(true));
            let is_member = database.sismember(KEY, ELEMENT).unwrap();
            assert_eq!(is_member, SuccessQuery::Boolean(true));
        }

        #[test]
        fn test_sadd_create_set_with_repeating_elements_returns_0() {
            let mut database = Database::new();
            database.sadd(KEY, ELEMENT).unwrap();

            let result = database.sadd(KEY, ELEMENT).unwrap();
            assert_eq!(result, SuccessQuery::Boolean(false));
            let len_set = database.scard(KEY).unwrap();
            assert_eq!(len_set, SuccessQuery::Integer(1));
        }

        #[test]
        fn test_sadd_key_with_another_type_of_set_returns_err() {
            let mut database = Database::new();
            database.set(KEY, ELEMENT).unwrap();

            let result = database.sadd(KEY, ELEMENT).unwrap_err();

            assert_eq!(result, DataBaseError::NotASet);
        }

        #[test]
        fn test_sadd_add_element_with_set_created_returns_1() {
            let mut database = Database::new();
            database.sadd(KEY, ELEMENT).unwrap();

            let result = database.sadd(KEY, OTHER_ELEMENT).unwrap();
            assert_eq!(result, SuccessQuery::Boolean(true));
            let len_set = database.scard(KEY).unwrap();
            assert_eq!(len_set, SuccessQuery::Integer(2));
        }
    }

    mod sismember_test {
        use super::*;

        #[test]
        fn test_sismember_set_with_element_returns_1() {
            let mut database = Database::new();
            database.sadd(KEY, ELEMENT).unwrap();

            let is_member = database.sismember(KEY, ELEMENT).unwrap();

            assert_eq!(is_member, SuccessQuery::Boolean(true));
        }

        #[test]
        fn test_sismember_set_without_element_returns_0() {
            let mut database = Database::new();
            database.sadd(KEY, ELEMENT).unwrap();

            let result = database.sismember(KEY, OTHER_ELEMENT).unwrap();

            assert_eq!(result, SuccessQuery::Boolean(false));
        }

        #[test]
        fn test_sismember_key_with_another_type_of_set_returns_err() {
            let mut database = Database::new();
            database.set(KEY, ELEMENT).unwrap();
            let result = database.sismember(KEY, ELEMENT).unwrap_err();

            assert_eq!(result, DataBaseError::NotASet);
        }

        #[test]
        fn test_sismember_with_non_exist_key_set_returns_0() {
            let mut database = Database::new();

            let result = database.sismember(KEY, ELEMENT).unwrap();

            assert_eq!(result, SuccessQuery::Boolean(false));
        }
    }

    mod scard_test {
        use super::*;

        #[test]
        fn test_scard_set_with_one_element_returns_1() {
            let mut database = Database::new();
            database.sadd(KEY, ELEMENT).unwrap();

            let len_set = database.scard(KEY).unwrap();

            assert_eq!(len_set, SuccessQuery::Integer(1));
        }

        #[test]
        fn test_scard_create_set_with_multiple_elements_returns_lenght_of_set() {
            let mut database = Database::new();

            for i in 0..10 {
                let _ = database.sadd(KEY, &i.to_string());
            }

            let len_set = database.scard(KEY).unwrap();

            assert_eq!(len_set, SuccessQuery::Integer(10));
        }

        #[test]
        fn test_scard_key_with_another_type_of_set_returns_err() {
            let mut database = Database::new();
            database.set(KEY, ELEMENT).unwrap();

            let result = database.scard(KEY).unwrap_err();

            assert_eq!(result, DataBaseError::NotASet);
        }

        #[test]
        fn test_scard_key_set_not_exist_in_database_returns_0() {
            let mut database = Database::new();

            let result = database.scard(KEY).unwrap();

            assert_eq!(result, SuccessQuery::Boolean(false));
        }
    }

    mod smembers_test {
        use super::*;

        #[test]
        fn test_smembers_with_elements_return_list_of_elements() {
            let mut database = Database::new();
            database.sadd(KEY, ELEMENT).unwrap();
            database.sadd(KEY, OTHER_ELEMENT).unwrap();

            if let SuccessQuery::List(list) = database.smembers(KEY).unwrap() {
                for elem in list {
                    let is_member = database.sismember(KEY, &elem.to_string()).unwrap();
                    assert_eq!(is_member, SuccessQuery::Boolean(true));
                }
            }
        }

        #[test]
        fn test_smembers_with_non_exist_key_return_empty_list() {
            let mut database = Database::new();

            let result = database.smembers(NON_EXIST_KEY).unwrap();

            assert_eq!(result, SuccessQuery::List(Vec::new()));
        }

        #[test]
        fn test_smembers_key_with_another_type_another_besides_set_return_err() {
            let mut database = Database::new();
            let _ = database.set(KEY_WITH_STR, VALUE_A);

            let result = database.smembers(KEY_WITH_STR).unwrap_err();

            assert_eq!(result, DataBaseError::NotASet);
        }
    }

    mod srem_test {
        use super::*;

        #[test]
        fn test_srem_one_member_in_set_returns_1_if_set_contains_member() {
            let members = vec![ELEMENT];
            let mut database = Database::new();
            database.sadd(KEY, ELEMENT).unwrap();

            let result = database.srem(KEY, members).unwrap();
            let is_member = database.sismember(KEY, &ELEMENT.to_string()).unwrap();

            assert_eq!(result, SuccessQuery::Integer(1));
            assert_eq!(is_member, SuccessQuery::Boolean(false));
        }

        #[test]
        fn test_srem_key_no_exits_in_database_returns_0() {
            let mut database = Database::new();
            let members_to_rmv = vec![ELEMENT];

            let result = database.srem(KEY, members_to_rmv).unwrap();

            assert_eq!(result, SuccessQuery::Boolean(false));
        }

        #[test]
        fn test_srem_multiple_members_in_set_returns_lenght_of_removed_elements_in_the_set() {
            let mut database = Database::new();
            let members = vec![ELEMENT, ELEMENT_2, ELEMENT_3];
            let members_to_rmv = vec![ELEMENT, ELEMENT_2, ELEMENT_3, NON_EXIST_ELEMENT];

            for member in &members {
                let _ = database.sadd(KEY, member.clone());
            }

            let result = database.srem(KEY, members_to_rmv).unwrap();
            assert_eq!(result, SuccessQuery::Integer(3));

            for member in members {
                let is_member = database.sismember(KEY, &member.to_string()).unwrap();
                assert_eq!(is_member, SuccessQuery::Boolean(false));
            }
        }

        #[test]
        fn test_srem_key_with_another_type_another_besides_set_return_err() {
            let mut database = Database::new();
            let members_to_rmv = vec![ELEMENT];

            let _ = database.set(KEY_WITH_STR, VALUE_A).unwrap();

            let result = database.srem(KEY_WITH_STR, members_to_rmv).unwrap_err();

            assert_eq!(result, DataBaseError::NotASet);
        }
    }
}

#[cfg(test)]
mod group_server {
    use super::*;
    const KEY1: &str = "key1";
    const VALUE1: &str = "value1";
    const KEY2: &str = "key2";
    const VALUE2: &str = "value2";

    mod flushdb_test {
        use super::*;

        #[test]
        fn flushdb_clear_dictionary() {
            let mut db = Database::new();
            let _ = db.mset(vec![KEY1, VALUE1, KEY2, VALUE2]);
            let r = db.get(KEY1).unwrap();
            assert_eq!(r, SuccessQuery::String(VALUE1.to_owned()));
            let r = db.get(KEY2).unwrap();
            assert_eq!(r, SuccessQuery::String(VALUE2.to_owned()));

            let r = db.flushdb().unwrap();
            assert_eq!(r, SuccessQuery::Success);
            assert!(db.dictionary.is_empty());
        }
    }

    mod dbsize_test {
        use super::*;

        #[test]
        fn dbsize_empty_gets_0() {
            let db = Database::new();

            let r = db.dbsize().unwrap();
            assert_eq!(r, SuccessQuery::Integer(0));
        }

        #[test]
        fn dbsize_with_one_element_gets_1() {
            let mut db = Database::new();
            let _ = db.set(KEY1, VALUE1);
            let r = db.get(KEY1).unwrap();
            assert_eq!(r, SuccessQuery::String(VALUE1.to_owned()));

            let r = db.dbsize().unwrap();
            assert_eq!(r, SuccessQuery::Integer(1));
        }

        #[test]
        fn dbsize_with_two_element_gets_2() {
            let mut db = Database::new();
            let _ = db.mset(vec![KEY1, VALUE1, KEY2, VALUE2]);
            let r = db.get(KEY1).unwrap();
            assert_eq!(r, SuccessQuery::String(VALUE1.to_owned()));
            let r = db.get(KEY2).unwrap();
            assert_eq!(r, SuccessQuery::String(VALUE2.to_owned()));

            let r = db.dbsize().unwrap();
            assert_eq!(r, SuccessQuery::Integer(2));
        }
    }
}
