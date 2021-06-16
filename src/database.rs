use crate::databasehelper::{DataBaseError, KeyTTL, MessageTTL, StorageValue, SuccessQuery, RespondTTL};
use regex::Regex;
use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fmt::{self, Formatter};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

type Dictionary = Arc<Mutex<HashMap<String, StorageValue>>>;
type TtlVector = Arc<Mutex<Vec<KeyTTL>>>;

pub struct Database {
    dictionary: Dictionary,
    ttl_msg_sender: Sender<MessageTTL>,
}

impl<'a> Clone for Database {
    fn clone(&self) -> Self {
        Database::new_from_db(self.ttl_msg_sender.clone(), self.dictionary.clone())
    }
}

fn executor(dictionary: Dictionary, ttl_vector: TtlVector) {
    loop {
        thread::sleep(Duration::new(1, 0));
        let keys_ttl = ttl_vector.clone();
        let mut keys_locked = keys_ttl.lock().unwrap();

        while let Some(ttl) = keys_locked.get(0) {
            if ttl.expire_time < SystemTime::now() {
                let ttl_key = keys_locked.remove(0);
                let mut dic_locked = dictionary.lock().unwrap();
                dic_locked.remove(&ttl_key.key);
            } else {
                break;
            }
        }

        if keys_locked.is_empty() {
            break;
        }
    }
}

impl Database {
    pub fn new(ttl_msg_sender: Sender<MessageTTL>) -> Database {
        Database {
            dictionary: Arc::new(Mutex::new(HashMap::new())),
            ttl_msg_sender,
        }
    }

    pub fn new_from_db(ttl_msg_sender: Sender<MessageTTL>, dictionary: Dictionary) -> Database {
        Database {
            dictionary,
            ttl_msg_sender,
        }
    }

    pub fn ttl_supervisor_run(&self, reciver: Receiver<MessageTTL>) {
        let dictionary = self.dictionary.clone();

        thread::spawn(move || {
            let ttl_keys: TtlVector = Arc::new(Mutex::new(Vec::new()));

            for message in reciver.iter() {
                match message {
                    MessageTTL::Expire(new_key_ttl) => {
                        let keys = ttl_keys.clone();
                        let mut keys_locked = keys.lock().unwrap();

                        match keys_locked.binary_search(&new_key_ttl) {
                            Ok(pos) => {
                                keys_locked.remove(pos);
                                keys_locked.insert(pos, new_key_ttl);
                            }
                            Err(pos) => {
                                keys_locked.insert(pos, new_key_ttl);
                            }
                        }

                        if keys_locked.len() == 1 {
                            let dic = dictionary.clone();
                            let ttl_vector = ttl_keys.clone();

                            thread::spawn(move || {
                                executor(dic, ttl_vector);
                            });
                        }
                    }
                    MessageTTL::Clear(key) => {
                        let keys = ttl_keys.clone();
                        let mut keys_locked = keys.lock().unwrap();

                        if let Some(pos) = keys_locked.iter().position(|x| x.key == key) {
                            keys_locked.remove(pos);
                        }
                    }

                    MessageTTL::Transfer(from_key, to_key) => {
                        let keys = ttl_keys.clone();
                        let mut keys_locked = keys.lock().unwrap();

                        if let Some(pos) = keys_locked.iter().position(|x| x.key == from_key) {
                            let ttl = keys_locked.get_mut(pos).unwrap();
                            ttl.key = to_key;
                        }
                    }
                    MessageTTL::TTL(key) => {
                        let keys = ttl_keys.clone();
                        let mut keys_locked = keys.lock().unwrap();

                        if let Some(pos) = keys_locked.iter().position(|x| x.key == key) {
                            let ttl = keys_locked.get(pos).unwrap();
                            ttl_respond.send(RespondTTL::TTL(ttl.expire_time));
                        }
                    }
                };
            }
        });
    }

    // KEYS

    pub fn copy(&self, key: &str, to_key: &str) -> Result<SuccessQuery, DataBaseError> {
        let mut dictionary = self.dictionary.lock().unwrap();

        if dictionary.contains_key(to_key) {
            return Err(DataBaseError::KeyAlredyExist);
        }

        match dictionary.get(key) {
            Some(val) => {
                let val = val.clone();
                dictionary.insert(to_key.to_owned(), val);
                Ok(SuccessQuery::Success)
            }
            None => Err(DataBaseError::NonExistentKey),
        }
    }

    pub fn del(&self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        let mut dictionary = self.dictionary.lock().unwrap();
        match dictionary.remove(key) {
            Some(_) => Ok(SuccessQuery::Success),
            None => Err(DataBaseError::NonExistentKey),
        }
    }

    pub fn exists(&self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        let dictionary = self.dictionary.lock().unwrap();
        let bool = dictionary.contains_key(key);
        Ok(SuccessQuery::Boolean(bool))
    }

    pub fn expire(
        &self,
        key: &str,
        seconds: u64,
    ) -> Result<SuccessQuery, DataBaseError> {
        let dictionary = self.dictionary.lock().unwrap();

        if dictionary.contains_key(key) {
            let duration = Duration::new(seconds, 0);
            let expire_time = SystemTime::now().checked_add(duration).unwrap();
            let key_ttl = KeyTTL::new(key, expire_time);
            self.ttl_msg_sender.send(MessageTTL::Expire(key_ttl)).unwrap();
            Ok(SuccessQuery::Boolean(true))
        } else {
            Ok(SuccessQuery::Boolean(false))
        }
    }

    //expireat

    pub fn keys(&self, pattern: &str) -> Result<SuccessQuery, DataBaseError> {
        let dictionary = self.dictionary.lock().unwrap();

        let patt: String = r"^".to_owned() + pattern + r"$";
        let patt: String = patt.replace("*", ".*");
        let patt: String = patt.replace("?", ".");
        let list: Vec<SuccessQuery> = dictionary
            .keys()
            .filter(|x| match Regex::new(&patt) {
                Ok(re) => re.is_match(x),
                Err(_) => false,
            })
            .map(|item| SuccessQuery::String(item.to_owned()))
            .collect::<Vec<SuccessQuery>>();

        Ok(SuccessQuery::List(list))
    }

    //persist
    pub fn persist(
        &self,
        key: &str,
    ) -> Result<SuccessQuery, DataBaseError> {
        let dictionary = self.dictionary.lock().unwrap();

        if dictionary.contains_key(key) {
            self.ttl_msg_sender.send(MessageTTL::Clear(key.to_owned())).unwrap();
            Ok(SuccessQuery::Boolean(true))
        } else {
            Ok(SuccessQuery::Boolean(false))
        }
    }

    pub fn rename(&self, old_key: &str, new_key: &str) -> Result<SuccessQuery, DataBaseError> {
        let mut dictionary = self.dictionary.lock().unwrap();

        match dictionary.remove(old_key) {
            Some(value) => {
                dictionary.insert(new_key.to_owned(), value);
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

    pub fn append(&self, key: &str, value: &str) -> Result<SuccessQuery, DataBaseError> {
        let mut dictionary = self.dictionary.lock().unwrap();

        if dictionary.contains_key(key) {
            if let Some(StorageValue::String(val)) = dictionary.get_mut(key) {
                val.push_str(&value);
                let len_result = val.len() as i32;
                Ok(SuccessQuery::Integer(len_result))
            } else {
                Err(DataBaseError::NotAString)
            }
        } else {
            let len_result = value.len() as i32;
            dictionary.insert(key.to_owned(), StorageValue::String(value.to_string()));
            Ok(SuccessQuery::Integer(len_result))
        }
    }

    pub fn decrby(&self, key: &str, decr: i32) -> Result<SuccessQuery, DataBaseError> {
        let mut dictionary = self.dictionary.lock().unwrap();

        if let Some(StorageValue::String(val)) = dictionary.get_mut(key) {
            let val = match val.parse::<i32>() {
                Ok(val) => val - decr,
                Err(_) => return Err(DataBaseError::NotAnInteger),
            };

            dictionary.insert(key.to_owned(), StorageValue::String(val.to_string()));
            Ok(SuccessQuery::Integer(val))
        } else {
            Err(DataBaseError::NotAnInteger)
        }
    }

    pub fn get(&self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        let dictionary = self.dictionary.lock().unwrap();

        match dictionary.get(key) {
            Some(StorageValue::String(val)) => Ok(SuccessQuery::String(val.clone())),
            Some(_) => Err(DataBaseError::NotAString),
            None => Err(DataBaseError::NonExistentKey),
        }
    }

    pub fn getdel(&self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        let mut dictionary = self.dictionary.lock().unwrap();

        match dictionary.remove(key) {
            Some(StorageValue::String(val)) => Ok(SuccessQuery::String(val)),
            Some(_) => Err(DataBaseError::NotAString),
            None => Err(DataBaseError::NonExistentKey),
        }
    }

    pub fn getset(&self, key: &str, new_val: &str) -> Result<SuccessQuery, DataBaseError> {
        let mut dictionary = self.dictionary.lock().unwrap();

        let old_value = match dictionary.remove(key) {
            Some(StorageValue::String(old_value)) => old_value,
            Some(_) => return Err(DataBaseError::NotAString),
            None => return Err(DataBaseError::NonExistentKey),
        };

        dictionary.insert(key.to_owned(), StorageValue::String(new_val.to_owned()));
        Ok(SuccessQuery::String(old_value))
    }

    pub fn incrby(&self, key: &str, incr: i32) -> Result<SuccessQuery, DataBaseError> {
        self.decrby(key, -incr)
    }

    pub fn mget(&self, params: Vec<&str>) -> Result<SuccessQuery, DataBaseError> {
        let dictionary = self.dictionary.lock().unwrap();

        let mut list: Vec<SuccessQuery> = Vec::new();

        for key in params {
            match dictionary.get(key) {
                Some(StorageValue::String(value)) => list.push(SuccessQuery::String(value.clone())),
                Some(_) | None => list.push(SuccessQuery::Nil),
            }
        }

        Ok(SuccessQuery::List(list))
    }

    pub fn mset(&self, params: Vec<&str>) -> Result<SuccessQuery, DataBaseError> {
        let mut dictionary = self.dictionary.lock().unwrap();

        for i in (0..params.len()).step_by(2) {
            let key = params.get(i).unwrap();
            let value = params.get(i + 1).unwrap();

            dictionary.insert(key.to_string(), StorageValue::String(value.to_string()));
        }
        Ok(SuccessQuery::Success)
    }

    pub fn set(&self, key: &str, val: &str) -> Result<SuccessQuery, DataBaseError> {
        let mut dictionary = self.dictionary.lock().unwrap();

        dictionary.insert(key.to_owned(), StorageValue::String(val.to_owned()));
        Ok(SuccessQuery::Success)
    }

    pub fn strlen(&self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        let mut dictionary = self.dictionary.lock().unwrap();

        if dictionary.contains_key(key) {
            if let Some(StorageValue::String(val)) = dictionary.get_mut(key) {
                Ok(SuccessQuery::Integer(val.len() as i32))
            } else {
                Err(DataBaseError::NotAString)
            }
        } else {
            Ok(SuccessQuery::Integer(0))
        }
    }

    // Lists

    pub fn lindex(&self, key: &str, index: i32) -> Result<SuccessQuery, DataBaseError> {
        let dictionary = self.dictionary.lock().unwrap();

        match dictionary.get(key) {
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

    pub fn llen(&self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        let dictionary = self.dictionary.lock().unwrap();

        match dictionary.get(key) {
            Some(StorageValue::List(list)) => Ok(SuccessQuery::Integer(list.len() as i32)),
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(SuccessQuery::Integer(0)),
        }
    }

    pub fn lpop(&self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        let mut dictionary = self.dictionary.lock().unwrap();
        match dictionary.get_mut(key) {
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

    pub fn lpush(&self, key: &str, value: &str) -> Result<SuccessQuery, DataBaseError> {
        let mut dictionary = self.dictionary.lock().unwrap();

        match dictionary.get_mut(key) {
            Some(StorageValue::List(list)) => {
                list.insert(0, value.to_owned());
                Ok(SuccessQuery::Integer(list.len() as i32))
            }
            Some(_) => Err(DataBaseError::NotAList),
            None => {
                let list: Vec<String> = vec![value.to_owned()];
                let len = list.len();
                dictionary.insert(key.to_owned(), StorageValue::List(list));
                Ok(SuccessQuery::Integer(len as i32))
            }
        }
    }

    pub fn lpushx(&self, key: &str, value: &str) -> Result<SuccessQuery, DataBaseError> {
        let mut dictionary = self.dictionary.lock().unwrap();

        match dictionary.get_mut(key) {
            Some(StorageValue::List(list)) => {
                list.insert(0, value.to_owned());
                Ok(SuccessQuery::Integer(list.len() as i32))
            }
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(SuccessQuery::Integer(0)),
        }
    }

    pub fn lrange(&self, key: &str, ini: i32, end: i32) -> Result<SuccessQuery, DataBaseError> {
        let mut sub_list: Vec<SuccessQuery> = Vec::new();
        let dictionary = self.dictionary.lock().unwrap();

        match dictionary.get(key) {
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
        &self,
        key: &str,
        mut count: i32,
        elem: &str,
    ) -> Result<SuccessQuery, DataBaseError> {
        let mut dictionary = self.dictionary.lock().unwrap();
        match dictionary.get_mut(key) {
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
        &self,
        key: &str,
        index: usize,
        value: &str,
    ) -> Result<SuccessQuery, DataBaseError> {
        let mut dictionary = self.dictionary.lock().unwrap();

        match dictionary.get_mut(key) {
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

    pub fn rpop(&self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        let mut dictionary = self.dictionary.lock().unwrap();

        match dictionary.get_mut(key) {
            Some(StorageValue::List(list)) => match list.pop() {
                Some(value) => Ok(SuccessQuery::String(value)),
                None => Ok(SuccessQuery::Nil),
            },
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(SuccessQuery::Nil),
        }
    }

    pub fn rpush(&self, key: &str, value: &str) -> Result<SuccessQuery, DataBaseError> {
        let mut dictionary = self.dictionary.lock().unwrap();

        match dictionary.get_mut(key) {
            Some(StorageValue::List(list)) => {
                list.push(value.to_owned());
                Ok(SuccessQuery::Integer(value.len() as i32))
            }
            Some(_) => Err(DataBaseError::NotAList),
            None => {
                let list: Vec<String> = vec![value.to_owned()];
                let len = list.len();
                dictionary.insert(key.to_owned(), StorageValue::List(list));
                Ok(SuccessQuery::Integer(len as i32))
            }
        }
    }

    pub fn rpushx(&self, key: &str, value: &str) -> Result<SuccessQuery, DataBaseError> {
        let mut dictionary = self.dictionary.lock().unwrap();

        match dictionary.get_mut(key) {
            Some(StorageValue::List(list)) => {
                list.push(value.to_owned());
                Ok(SuccessQuery::Integer(value.len() as i32))
            }
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(SuccessQuery::Boolean(false)),
        }
    }

    //SETS

    pub fn sismember(&self, key: &str, value: &str) -> Result<SuccessQuery, DataBaseError> {
        let mut dictionary = self.dictionary.lock().unwrap();
        match dictionary.get_mut(key) {
            Some(StorageValue::Set(hash_set)) => match hash_set.get(value) {
                Some(_val) => Ok(SuccessQuery::Boolean(true)),
                None => Ok(SuccessQuery::Boolean(false)),
            },
            Some(_) => Err(DataBaseError::NotASet),
            None => Ok(SuccessQuery::Boolean(false)),
        }
    }

    pub fn scard(&self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        let mut dictionary = self.dictionary.lock().unwrap();

        match dictionary.get_mut(key) {
            Some(StorageValue::Set(hash_set)) => Ok(SuccessQuery::Integer(hash_set.len() as i32)),
            Some(_) => Err(DataBaseError::NotASet),
            None => Ok(SuccessQuery::Boolean(false)),
        }
    }

    pub fn sadd(&self, key: &str, value: &str) -> Result<SuccessQuery, DataBaseError> {
        let mut dictionary = self.dictionary.lock().unwrap();

        match dictionary.get_mut(key) {
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
                dictionary.insert(key.to_owned(), StorageValue::Set(set));
                Ok(SuccessQuery::Boolean(true))
            }
        }
    }
}

impl fmt::Display for Database {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let dictionary = self.dictionary.lock().unwrap();

        for (key, value) in dictionary.iter() {
            writeln!(f, "key: {}, value: {}", key, value)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod ttl_commands {
    use super::*;
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::time::Duration;

    const KEY_A: &str = "KEY_A";
    const VALUE_A: &str = "VALUE_A";

    const KEY_B: &str = "KEY_B";
    const VALUE_B: &str = "VALUE_B";

    const KEY_C: &str = "KEY_C";
    const VALUE_C: &str = "VALUE_C";

    const KEY_D: &str = "KEY_D";
    const VALUE_D: &str = "VALUE_D";


    // duration_since

    #[test]
    fn ttl_supervisor_run_supervaise_a_key() {
        let (send, recv): (Sender<MessageTTL>, Receiver<MessageTTL>) = mpsc::channel();
        
        let db = Database::new(send.clone());

        db.ttl_supervisor_run(recv);

        db.append(KEY_A, VALUE_A).unwrap();

        let now = SystemTime::now();
        let expire_time_a = now.checked_add(Duration::new(1, 0)).unwrap();
        let ttl_pair = KeyTTL::new(KEY_A, expire_time_a);

        if let SuccessQuery::Boolean(value) = db.exists(KEY_A).unwrap() {
            assert_eq!(value, true);
        }

        send.send(MessageTTL::Expire(ttl_pair)).unwrap();

        thread::sleep(Duration::new(2, 0));

        if let SuccessQuery::Boolean(value) = db.exists(KEY_A).unwrap() {
            assert_eq!(value, false);
        }
    }

    #[test]
    fn ttl_supervisor_run_supervaise_two_key() {
        let (send, recv): (Sender<MessageTTL>, Receiver<MessageTTL>) = mpsc::channel();
        let db = Database::new(send.clone());

        db.ttl_supervisor_run(recv);

        db.append(KEY_A, VALUE_A).unwrap();
        db.append(KEY_B, VALUE_B).unwrap();

        let now = SystemTime::now();
        let expire_time_a = now.checked_add(Duration::new(1, 0)).unwrap();
        let expire_time_b = now.checked_add(Duration::new(5, 0)).unwrap();

        let ttl_pair_a = KeyTTL::new(KEY_A, expire_time_a);
        let ttl_pair_b = KeyTTL::new(KEY_B, expire_time_b);

        if let SuccessQuery::Boolean(value) = db.exists(KEY_A).unwrap() {
            assert_eq!(value, true);
        }

        if let SuccessQuery::Boolean(value) = db.exists(KEY_B).unwrap() {
            assert_eq!(value, true);
        }

        send.send(MessageTTL::Expire(ttl_pair_a)).unwrap();
        send.send(MessageTTL::Expire(ttl_pair_b)).unwrap();

        thread::sleep(Duration::new(2, 0));

        if let SuccessQuery::Boolean(value) = db.exists(KEY_A).unwrap() {
            assert_eq!(value, false);
        }

        thread::sleep(Duration::new(4, 0));

        if let SuccessQuery::Boolean(value) = db.exists(KEY_B).unwrap() {
            assert_eq!(value, false);
        }
    }



    const SEC: u64 = 1;

    #[test]
    fn ttl_supervisor_run_supervaise_four_keys() {
        let (send, recv): (Sender<KeyTTL>, Receiver<KeyTTL>) = mpsc::channel();
        let db = Database::new();
        db.ttl_supervisor_run(recv);

        db.append(KEY_A, VALUE_A).unwrap();
        db.append(KEY_B, VALUE_B).unwrap();
        db.append(KEY_C, VALUE_C).unwrap();
        db.append(KEY_D, VALUE_D).unwrap();

        let now = SystemTime::now();
        let expire_time_a = now.checked_add(Duration::from_secs(SEC)).unwrap();
        let expire_time_b = now.checked_add(Duration::from_secs(SEC * 2)).unwrap();
        let expire_time_c = now.checked_add(Duration::from_secs(SEC * 3)).unwrap();
        let expire_time_d = now.checked_add(Duration::from_secs(SEC * 4)).unwrap();

        let ttl_pair_a = KeyTTL::new(KEY_A.to_owned(), expire_time_a);
        let ttl_pair_b = KeyTTL::new(KEY_B.to_owned(), expire_time_b);
        let ttl_pair_c = KeyTTL::new(KEY_C.to_owned(), expire_time_c);
        let ttl_pair_d = KeyTTL::new(KEY_D.to_owned(), expire_time_d);

        if let SuccessQuery::Boolean(value) = db.exists(KEY_A).unwrap() {
            assert_eq!(value, true);
        }

        if let SuccessQuery::Boolean(value) = db.exists(KEY_B).unwrap() {
            assert_eq!(value, true);
        }

        if let SuccessQuery::Boolean(value) = db.exists(KEY_C).unwrap() {
            assert_eq!(value, true);
        }

        if let SuccessQuery::Boolean(value) = db.exists(KEY_D).unwrap() {
            assert_eq!(value, true);
        }

        send.send(ttl_pair_a).unwrap();
        send.send(ttl_pair_b).unwrap();
        send.send(ttl_pair_c).unwrap();
        send.send(ttl_pair_d).unwrap();

        thread::sleep(Duration::from_secs(SEC * 2));

        if let SuccessQuery::Boolean(value) = db.exists(KEY_A).unwrap() {
            assert_eq!(value, false);
        }

        if let SuccessQuery::Boolean(value) = db.exists(KEY_B).unwrap() {
            assert_eq!(value, true);
        }

        if let SuccessQuery::Boolean(value) = db.exists(KEY_C).unwrap() {
            assert_eq!(value, true);
        }

        thread::sleep(Duration::from_secs(SEC));

        if let SuccessQuery::Boolean(value) = db.exists(KEY_B).unwrap() {
            assert_eq!(value, false);
        }
        if let SuccessQuery::Boolean(value) = db.exists(KEY_C).unwrap() {
            assert_eq!(value, true);
        }

        thread::sleep(Duration::from_secs(SEC));

        if let SuccessQuery::Boolean(value) = db.exists(KEY_C).unwrap() {
            assert_eq!(value, false);
        }

        if let SuccessQuery::Boolean(value) = db.exists(KEY_D).unwrap() {
            assert_eq!(value, true);
        }

        thread::sleep(Duration::from_secs(SEC));

        if let SuccessQuery::Boolean(value) = db.exists(KEY_B).unwrap() {
            assert_eq!(value, false);
        }

        if let SuccessQuery::Boolean(value) = db.exists(KEY_C).unwrap() {
            assert_eq!(value, false);
        }

        if let SuccessQuery::Boolean(value) = db.exists(KEY_D).unwrap() {
            assert_eq!(value, false);
        }
    }

} 

//     #[test]
//     fn ttl_supervisor_run_supervaise_four_keys_one_of_the_key_is_inserted_with_a_lower_expire_time_the_actual_key(
//     ) {
//         let (send, recv): (Sender<KeyTTL>, Receiver<KeyTTL>) = mpsc::channel();
//         let db = Database::new();
//         db.ttl_supervisor_run(recv);

//         db.append(KEY_A, VALUE_A).unwrap();
//         db.append(KEY_B, VALUE_B).unwrap();
//         db.append(KEY_C, VALUE_C).unwrap();
//         db.append(KEY_D, VALUE_D).unwrap();

//         let now = SystemTime::now();
//         let expire_time_a = now.checked_add(Duration::from_secs(SEC)).unwrap();
//         let expire_time_b = now.checked_add(Duration::from_secs(SEC * 2)).unwrap();
//         let expire_time_c = now.checked_add(Duration::from_secs(SEC * 3)).unwrap();
//         let expire_time_d = now.checked_add(Duration::from_secs(SEC * 4)).unwrap();

//         let ttl_pair_a = KeyTTL::new(KEY_A.to_owned(), expire_time_a);
//         let ttl_pair_b = KeyTTL::new(KEY_B.to_owned(), expire_time_b);
//         let ttl_pair_c = KeyTTL::new(KEY_C.to_owned(), expire_time_c);
//         let ttl_pair_d = KeyTTL::new(KEY_D.to_owned(), expire_time_d);

//         if let SuccessQuery::Boolean(value) = db.exists(KEY_A).unwrap() {
//             assert_eq!(value, true);
//         }

//         if let SuccessQuery::Boolean(value) = db.exists(KEY_B).unwrap() {
//             assert_eq!(value, true);
//         }

//         if let SuccessQuery::Boolean(value) = db.exists(KEY_C).unwrap() {
//             assert_eq!(value, true);
//         }

//         if let SuccessQuery::Boolean(value) = db.exists(KEY_D).unwrap() {
//             assert_eq!(value, true);
//         }

//         send.send(ttl_pair_b).unwrap();
//         send.send(ttl_pair_c).unwrap();
//         send.send(ttl_pair_a).unwrap();
//         send.send(ttl_pair_d).unwrap();

//         thread::sleep(Duration::from_millis(SEC + 1));

//         if let SuccessQuery::Boolean(value) = db.exists(KEY_A).unwrap() {
//             assert_eq!(value, false);
//         }

//         if let SuccessQuery::Boolean(value) = db.exists(KEY_B).unwrap() {
//             assert_eq!(value, true);
//         }

//         if let SuccessQuery::Boolean(value) = db.exists(KEY_C).unwrap() {
//             assert_eq!(value, true);
//         }

//         if let SuccessQuery::Boolean(value) = db.exists(KEY_C).unwrap() {
//             assert_eq!(value, true);
//         }

//         thread::sleep(Duration::from_millis(SEC + 1));
//         if let SuccessQuery::Boolean(value) = db.exists(KEY_A).unwrap() {
//             assert_eq!(value, false);
//         }

//         if let SuccessQuery::Boolean(value) = db.exists(KEY_B).unwrap() {
//             assert_eq!(value, false);
//         }

//         if let SuccessQuery::Boolean(value) = db.exists(KEY_C).unwrap() {
//             assert_eq!(value, true);
//         }

//         if let SuccessQuery::Boolean(value) = db.exists(KEY_D).unwrap() {
//             assert_eq!(value, true);
//         }

//         thread::sleep(Duration::from_millis(SEC + 1));

//         if let SuccessQuery::Boolean(value) = db.exists(KEY_A).unwrap() {
//             assert_eq!(value, false);
//         }

//         if let SuccessQuery::Boolean(value) = db.exists(KEY_B).unwrap() {
//             assert_eq!(value, false);
//         }

//         if let SuccessQuery::Boolean(value) = db.exists(KEY_C).unwrap() {
//             assert_eq!(value, false);
//         }

//         if let SuccessQuery::Boolean(value) = db.exists(KEY_D).unwrap() {
//             assert_eq!(value, true);
//         }
//         thread::sleep(Duration::from_millis(SEC + 5));

//         if let SuccessQuery::Boolean(value) = db.exists(KEY_A).unwrap() {
//             assert_eq!(value, false);
//         }

//         if let SuccessQuery::Boolean(value) = db.exists(KEY_B).unwrap() {
//             assert_eq!(value, false);
//         }

//         if let SuccessQuery::Boolean(value) = db.exists(KEY_C).unwrap() {
//             assert_eq!(value, false);
//         }

//         if let SuccessQuery::Boolean(value) = db.exists(KEY_D).unwrap() {
//             assert_eq!(value, false);
//         }
//     }
// }

// #[cfg(test)]
// mod group_string {

//     use super::*;

//     const KEY: &str = "KEY";
//     const VALUE: &str = "VALUE";

//     fn create_database_with_string() -> Database {
//         let database = Database::new();

//         database.set(KEY, VALUE).unwrap();

//         database
//     }

//     mod append_test {

//         use super::*;

//         #[test]
//         fn test_append_new_key_return_lenght_of_the_value() {
//             let database = Database::new();

//             if let SuccessQuery::Integer(lenght) = database.append(KEY, VALUE).unwrap() {
//                 assert_eq!(lenght, 5);
//             }
//         }

//         #[test]
//         fn test_append_key_with_old_value_return_lenght_of_the_total_value() {
//             let database = Database::new();

//             if let SuccessQuery::Integer(lenght) = database.append(KEY, VALUE).unwrap() {
//                 assert_eq!(lenght, 5);
//                 if let SuccessQuery::Integer(lenght) = database.append(KEY, VALUE).unwrap() {
//                     assert_eq!(lenght, 10);
//                 }
//             }
//         }
//     }

//     mod get_test {

//         use super::*;

//         #[test]
//         fn test_get_returns_value_of_key() {
//             let database = create_database_with_string();

//             if let SuccessQuery::String(value) = database.get(KEY).unwrap() {
//                 assert_eq!(VALUE, value.to_string());
//             }
//         }

//         #[test]
//         fn test_get_returns_error_if_the_key_does_not_exist() {
//             let database = Database::new();

//             let result = database.get(KEY).unwrap_err();

//             assert_eq!(result, DataBaseError::NonExistentKey);
//         }
//     }

//     mod set_test {
//         use super::*;

//         #[test]
//         fn test_set_returns_ok() {
//             let database = Database::new();

//             let result = database.set(KEY, VALUE).unwrap();
//             assert_eq!(SuccessQuery::Success, result);

//             if let SuccessQuery::String(value) = database.get(KEY).unwrap() {
//                 assert_eq!(VALUE, value.to_string());
//             }
//         }
//     }

//     mod getdel_test {
//         use super::*;

//         #[test]
//         fn test_getdel_returns_old_value_of_key_and_after_value_of_key_what_not_exists_in_database_is_nul(
//         ) {
//             let database = create_database_with_string();

//             if let SuccessQuery::String(value) = database.getdel(KEY).unwrap() {
//                 assert_eq!(value.to_string(), VALUE);
//             }

//             let result = database.get(KEY).unwrap_err();
//             assert_eq!(result, DataBaseError::NonExistentKey);
//         }

//         #[test]
//         fn test_getdel_returns_nil_value_if_key_not_exist_in_database() {
//             let database = Database::new();

//             let result = database.getdel(KEY).unwrap_err();
//             assert_eq!(result, DataBaseError::NonExistentKey);
//         }
//     }

//     mod incrby_test {
//         use super::*;

//         #[test]
//         fn test_incrby_returns_lenght_of_the_resulting_value_after_increment() {
//             let database = Database::new();
//             database.set(KEY, "1").unwrap();

//             let result = database.incrby(KEY, 4).unwrap();

//             assert_eq!(result, SuccessQuery::Integer(5));
//         }
//     }

//     mod decrby_test {
//         use super::*;

//         #[test]
//         fn test_decrby_returns_lenght_of_the_resulting_value_after_increment() {
//             let database = Database::new();
//             database.set(KEY, "5").unwrap();
//             let result = database.decrby(KEY, 4).unwrap();

//             assert_eq!(result, SuccessQuery::Integer(1));
//         }
//     }

//     mod strlen_test {
//         use super::*;

//         #[test]
//         fn test_strlen_returns_the_lenght_of_value_key() {
//             let database = Database::new();
//             database.set(KEY, VALUE).unwrap();

//             if let SuccessQuery::Integer(value) = database.strlen(KEY).unwrap() {
//                 assert_eq!(value, 5);
//             }
//         }

//         #[test]
//         fn test_strlen_returns_zero_if_the_key_not_exists_in_database() {
//             let database = Database::new();
//             if let SuccessQuery::Integer(value) = database.strlen(KEY).unwrap() {
//                 assert_eq!(value, 0);
//             }
//         }
//     }

//     mod mset_test {
//         use super::*;

//         const KEY_A: &str = "KEY_A";
//         const VALUE_A: &str = "VALUE_A";

//         const KEY_B: &str = "KEY_B";
//         const VALUE_B: &str = "VALUE_B";

//         const KEY_C: &str = "KEY_C";
//         const VALUE_C: &str = "VALUE_C";

//         const KEY_D: &str = "KEY_D";
//         const VALUE_D: &str = "VALUE_D";

//         #[test]
//         fn test_mset_set_multiple_key_and_value_ok() {
//             let database = Database::new();
//             let vec_key_value = vec![
//                 KEY_A, VALUE_A, KEY_B, VALUE_B, KEY_C, VALUE_C, KEY_D, VALUE_D,
//             ];

//             let result = database.mset(vec_key_value).unwrap();
//             assert_eq!(result, SuccessQuery::Success);

//             let result_get1 = database.get(KEY_A).unwrap();
//             let result_get2 = database.get(KEY_B).unwrap();
//             let result_get3 = database.get(KEY_C).unwrap();
//             let result_get4 = database.get(KEY_D).unwrap();

//             assert_eq!(result_get1, SuccessQuery::String(VALUE_A.to_owned()));
//             assert_eq!(result_get2, SuccessQuery::String(VALUE_B.to_owned()));
//             assert_eq!(result_get3, SuccessQuery::String(VALUE_C.to_owned()));
//             assert_eq!(result_get4, SuccessQuery::String(VALUE_D.to_owned()));
//         }
//     }

//     mod mget_test {
//         use super::*;

//         const KEY_A: &str = "KEY_A";
//         const VALUE_A: &str = "VALUE_A";

//         const KEY_B: &str = "KEY_B";
//         const VALUE_B: &str = "VALUE_B";

//         const KEY_C: &str = "KEY_C";
//         const VALUE_C: &str = "VALUE_C";

//         const KEY_D: &str = "KEY_D";
//         const VALUE_D: &str = "VALUE_D";

//         fn create_a_database_with_key_values() -> Database {
//             let database = Database::new();

//             database.set(KEY_A, VALUE_A).unwrap();
//             database.set(KEY_B, VALUE_B).unwrap();
//             database.set(KEY_C, VALUE_C).unwrap();
//             database.set(KEY_D, VALUE_D).unwrap();

//             database
//         }

//         #[test]
//         fn test_mget_return_all_values_of_keys_if_all_keys_are_in_database() {
//             let database = create_a_database_with_key_values();

//             let vec_keys = vec![KEY_A, KEY_B, KEY_C, KEY_D];

//             if let SuccessQuery::List(list) = database.mget(vec_keys).unwrap() {
//                 assert_eq!(list[0], SuccessQuery::String(VALUE_A.to_owned()));
//                 assert_eq!(list[1], SuccessQuery::String(VALUE_B.to_owned()));
//                 assert_eq!(list[2], SuccessQuery::String(VALUE_C.to_owned()));
//                 assert_eq!(list[3], SuccessQuery::String(VALUE_D.to_owned()));
//             }
//         }

//         #[test]

//         fn test_mget_returns_nil_for_some_key_that_no_exists_in_database() {
//             let database = Database::new();
//             database.set(KEY_A, VALUE_A).unwrap();
//             database.set(KEY_B, VALUE_B).unwrap();

//             let vec_keys = vec![KEY_A, KEY_B, KEY_C, KEY_D];

//             if let SuccessQuery::List(list) = database.mget(vec_keys).unwrap() {
//                 assert_eq!(list[0], SuccessQuery::String(VALUE_A.to_owned()));
//                 assert_eq!(list[1], SuccessQuery::String(VALUE_B.to_owned()));
//                 assert_eq!(list[2], SuccessQuery::Nil);
//                 assert_eq!(list[3], SuccessQuery::Nil);
//             }
//         }
//     }
// }

// #[cfg(test)]
// mod group_keys {
//     use super::*;

//     const KEY: &str = "KEY";
//     const SECOND_KEY: &str = "SECOND_KEY";

//     const VALUE: &str = "VALUE";
//     const SECOND_VALUE: &str = "SECOND_VALUE";

//     mod copy_test {
//         use super::*;

//         #[test]
//         fn test_copy_set_dolly_sheep_then_copy_to_clone() {
//             let database = Database::new();

//             database.set(KEY, VALUE).unwrap();
//             let result = database.copy(KEY, SECOND_KEY);
//             assert_eq!(result.unwrap(), SuccessQuery::Success);
//             if let SuccessQuery::String(value) = database.strlen(SECOND_KEY).unwrap() {
//                 assert_eq!(value.to_string(), VALUE);
//             }
//         }

//         #[test]
//         fn test_copy_set_dolly_sheep_then_copy_to_clone_when_clone_exist() {
//             let database = Database::new();
//             database.set(KEY, VALUE).unwrap();
//             database.set(SECOND_KEY, SECOND_VALUE).unwrap();
//             let result = database.copy(KEY, SECOND_KEY);

//             assert_eq!(result.unwrap_err(), DataBaseError::KeyAlredyExist);
//         }

//         #[test]
//         fn test_copy_try_to_copy_a_key_does_not_exist() {
//             let database = Database::new();
//             let result = database.copy(KEY, SECOND_KEY);

//             assert_eq!(result.unwrap_err(), DataBaseError::NonExistentKey);
//         }
//     }

//     mod del_test {
//         use super::*;

//         #[test]
//         fn test_del_key_value_returns_succes() {
//             let database = Database::new();
//             database.set(KEY, VALUE).unwrap();

//             let result = database.del(KEY);
//             assert_eq!(result.unwrap(), SuccessQuery::Success);
//             assert_eq!(
//                 database.get(KEY).unwrap_err(),
//                 DataBaseError::NonExistentKey
//             );
//         }
//         #[test]
//         fn test_del_key_non_exist_returns_error() {
//             let database = Database::new();

//             let result = database.del(KEY);
//             assert_eq!(result.unwrap_err(), DataBaseError::NonExistentKey);
//         }
//     }

//     mod exists_test {

//         use super::*;

//         #[test]
//         fn test_exists_key_non_exist_returns_bool_false() {
//             let database = Database::new();

//             let result = database.exists(KEY);
//             assert_eq!(result.unwrap(), SuccessQuery::Boolean(false));
//             assert_eq!(
//                 database.get(KEY).unwrap_err(),
//                 DataBaseError::NonExistentKey
//             );
//         }

//         #[test]
//         fn test_exists_key_hello_returns_bool_true() {
//             let database = Database::new();
//             database.set(KEY, VALUE).unwrap();

//             let result = database.exists(KEY);

//             assert_eq!(result.unwrap(), SuccessQuery::Boolean(true));

//             if let SuccessQuery::String(value) = database.strlen(KEY).unwrap() {
//                 assert_eq!(value.to_string(), VALUE);
//             }
//         }
//     }

//     mod keys_test {
//         use super::*;

//         const FIRST_NAME: &str = "firstname";
//         const LAST_NAME: &str = "lastname";
//         const AGE: &str = "age";

//         const FIRST_NAME_VALUE: &str = "Alex";
//         const LAST_NAME_VALUE: &str = "Arbieto";
//         const AGE_VALUE: &str = "22";

//         fn create_database_with_keys() -> Database {
//             let database = Database::new();
//             database.set(FIRST_NAME, FIRST_NAME_VALUE).unwrap();
//             database.set(LAST_NAME, LAST_NAME_VALUE).unwrap();
//             database.set(AGE, AGE_VALUE).unwrap();
//             database
//         }

//         #[test]
//         fn test_keys_obtain_keys_with_name() {
//             let database = create_database_with_keys();
//             if let Ok(SuccessQuery::List(list)) = database.keys("*name") {
//                 let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();

//                 assert!(list.contains(&FIRST_NAME.to_owned()));
//                 assert!(list.contains(&LAST_NAME.to_owned()));
//             }
//         }

//         #[test]
//         fn test_keys_obtain_keys_with_four_question_name() {
//             let database = create_database_with_keys();
//             if let Ok(SuccessQuery::List(list)) = database.keys("????name") {
//                 let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();

//                 assert!(list.contains(&LAST_NAME.to_owned()));
//             }
//         }

//         const KEY_A: &str = "key";
//         const KEY_B: &str = "keeeey";
//         const KEY_C: &str = "ky";
//         const NO_MATCH: &str = "notmatch";

//         const VAL_A: &str = "valA";
//         const VAL_B: &str = "valB";
//         const VAL_C: &str = "valC";
//         const VAL_D: &str = "valD";

//         fn create_database_with_keys_two() -> Database {
//             let database = Database::new();

//             database.set(KEY_A, VAL_A).unwrap();
//             database.set(KEY_B, VAL_B).unwrap();
//             database.set(KEY_C, VAL_C).unwrap();
//             database.set(NO_MATCH, VAL_D).unwrap();

//             database
//         }

//         #[test]
//         fn test_keys_obtain_all_keys_with_an_asterisk_in_the_middle() {
//             let database = create_database_with_keys_two();

//             if let Ok(SuccessQuery::List(list)) = database.keys("k*y") {
//                 let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();

//                 assert!(list.contains(&KEY_A.to_owned()));
//                 assert!(list.contains(&KEY_B.to_owned()));
//                 assert!(list.contains(&KEY_C.to_owned()));
//             }
//         }

//         #[test]
//         fn test_keys_obtain_all_keys_with_question_in_the_middle() {
//             let database = create_database_with_keys_two();

//             if let Ok(SuccessQuery::List(list)) = database.keys("k?y") {
//                 let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();

//                 assert!(list.contains(&KEY_A.to_owned()));
//             }
//         }

//         const HALL: &str = "hall";
//         const HELLO: &str = "hello";
//         const HEEEELLO: &str = "heeeeeello";
//         const HALLO: &str = "hallo";
//         const HXLLO: &str = "hxllo";
//         const HLLO: &str = "hllo";
//         const RHLLO: &str = r"h\llo";
//         const AHLLO: &str = "ahllo";
//         const HALLOWN: &str = "hallown";

//         fn create_database_with_keys_three() -> Database {
//             let database = Database::new();
//             database.set(HELLO, VAL_A).unwrap();
//             database.set(HEEEELLO, VAL_A).unwrap();
//             database.set(HALLO, VAL_B).unwrap();
//             database.set(HXLLO, VAL_C).unwrap();
//             database.set(HLLO, VAL_C).unwrap();
//             database.set(RHLLO, VAL_D).unwrap();
//             database.set(AHLLO, VAL_D).unwrap();
//             database.set(HALLOWN, VAL_D).unwrap();
//             database.set(HALL, VAL_D).unwrap();

//             database
//         }

//         #[test]
//         fn test_keys_obtain_keys_with_h_question_llo_matches_correctly() {
//             let database = create_database_with_keys_three();

//             if let Ok(SuccessQuery::List(list)) = database.keys("h?llo") {
//                 let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();

//                 assert!(list.contains(&HELLO.to_owned()));
//                 assert!(list.contains(&HXLLO.to_owned()));
//                 assert!(list.contains(&RHLLO.to_owned()));
//                 assert!(list.contains(&HXLLO.to_owned()));
//                 assert!(list.contains(&RHLLO.to_owned()));
//             }
//         }

//         #[test]
//         fn test_keys_obtain_keys_with_h_asterisk_llo_matches_correctly() {
//             let database = create_database_with_keys_three();

//             if let Ok(SuccessQuery::List(list)) = database.keys("h*llo") {
//                 let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();

//                 assert!(list.contains(&HELLO.to_owned()));
//                 assert!(list.contains(&HALLO.to_owned()));
//                 assert!(list.contains(&HEEEELLO.to_owned()));
//                 assert!(list.contains(&HELLO.to_owned()));
//             }
//         }

//         #[test]
//         fn test_keys_obtain_keys_with_nomatch_returns_empty_list() {
//             let database = create_database_with_keys();

//             if let SuccessQuery::List(list) = database.keys("nomatch").unwrap() {
//                 assert_eq!(list.len(), 0);
//             }
//         }
//     }

//     mod rename_test {
//         use super::*;

//         #[test]
//         fn test_rename_key_with_mykey_get_hello() {
//             let database = Database::new();
//             database.set(KEY, VALUE).unwrap();

//             let result = database.rename(KEY, SECOND_KEY).unwrap();
//             assert_eq!(result, SuccessQuery::Success);

//             let result = database.get(KEY).unwrap_err();
//             assert_eq!(result, DataBaseError::NonExistentKey);
//         }

//         #[test]
//         fn test_rename_key_non_exists_error() {
//             let database = Database::new();

//             let result = database.rename(KEY, SECOND_KEY).unwrap_err();

//             assert_eq!(result, DataBaseError::NonExistentKey);
//         }
//     }
// }

// #[cfg(test)]
// mod group_list {

//     const KEY: &str = "KEY";
//     const VALUE: &str = "VALUE";

//     const VALUEA: &str = "ValueA";
//     const VALUEB: &str = "ValueB";
//     const VALUEC: &str = "ValueC";
//     const VALUED: &str = "ValueD";

//     use super::*;

//     fn database_with_a_list() -> Database {
//         let database = Database::new();

//         database.lpush(KEY, VALUEA).unwrap();
//         database.lpush(KEY, VALUEB).unwrap();
//         database.lpush(KEY, VALUEC).unwrap();
//         database.lpush(KEY, VALUED).unwrap();

//         database
//     }

//     fn database_with_a_three_repeated_values() -> Database {
//         let database = Database::new();

//         database.lpush(KEY, VALUEA).unwrap();
//         database.lpush(KEY, VALUEA).unwrap();
//         database.lpush(KEY, VALUEC).unwrap();
//         database.lpush(KEY, VALUEA).unwrap();

//         database
//     }

//     fn database_with_a_string() -> Database {
//         let database = Database::new();

//         database.append(KEY, VALUE).unwrap();

//         database
//     }

//     mod llen_test {

//         use super::*;

//         #[test]
//         fn test_llen_on_a_non_existent_key_gets_len_zero() {
//             let database = Database::new();

//             let result = database.llen(KEY);

//             assert_eq!(result.unwrap(), SuccessQuery::Integer(0));
//         }

//         #[test]
//         fn test_llen_on_a_list_with_one_value() {
//             let database = Database::new();
//             database.lpush(KEY, VALUE).unwrap();

//             let result = database.llen(KEY);

//             assert_eq!(result.unwrap(), SuccessQuery::Integer(1));
//         }

//         #[test]
//         fn test_llen_on_a_list_with_more_than_one_value() {
//             let database = database_with_a_list();

//             let result = database.llen(KEY).unwrap();

//             assert_eq!(result, SuccessQuery::Integer(4));
//         }

//         #[test]
//         fn test_llen_on_an_existent_key_that_isnt_a_list() {
//             let database = database_with_a_string();

//             let result = database.llen(KEY);

//             assert_eq!(result.unwrap_err(), DataBaseError::NotAList);
//         }
//     }

//     mod lindex_test {
//         use super::*;

//         #[test]
//         fn test_lindex_on_a_non_existent_key() {
//             let database = Database::new();

//             let result = database.lindex(KEY, 10);

//             assert_eq!(result.unwrap(), SuccessQuery::Nil);
//         }

//         #[test]
//         fn test_lindex_with_a_list_with_one_value_on_idex_zero() {
//             let database = Database::new();

//             database.lpush(KEY, VALUE).unwrap();

//             if let SuccessQuery::String(value) = database.lindex(KEY, 0).unwrap() {
//                 assert_eq!(value, VALUE);
//             }
//         }

//         #[test]
//         fn test_lindex_with_more_than_one_value() {
//             let database = database_with_a_list();

//             if let SuccessQuery::String(value) = database.lindex(KEY, 3).unwrap() {
//                 assert_eq!(value, VALUEA);
//             }
//         }

//         #[test]
//         fn test_lindex_with_more_than_one_value_with_a_negative_index() {
//             let database = database_with_a_list();

//             if let SuccessQuery::String(value) = database.lindex(KEY, -1).unwrap() {
//                 assert_eq!(value, VALUEA);
//             }
//         }

//         #[test]
//         fn test_lindex_on_a_reverse_out_of_bound() {
//             let database = database_with_a_list();

//             let result = database.lindex(KEY, -5);

//             assert_eq!(result.unwrap(), SuccessQuery::Nil);
//         }

//         #[test]
//         fn test_lindex_on_an_out_of_bound() {
//             let database = database_with_a_list();

//             let result = database.lindex(KEY, 100);

//             assert_eq!(result.unwrap(), SuccessQuery::Nil);
//         }
//     }

//     mod lpop_test {
//         use super::*;

//         #[test]
//         fn test_lpop_on_a_non_existent_key() {
//             let database = Database::new();

//             let result = database.lpop(KEY).unwrap();

//             assert_eq!(result, SuccessQuery::Nil);
//         }

//         #[test]
//         fn test_lpop_with_a_list_with_one_value_on_idex_zero() {
//             let database = Database::new();

//             database.lpush(KEY, VALUE).unwrap();

//             if let SuccessQuery::String(val) = database.lpop(KEY).unwrap() {
//                 assert_eq!(val.to_string(), VALUE);
//             }

//             let result = database.llen(KEY).unwrap();
//             assert_eq!(result, SuccessQuery::Integer(0));
//         }

//         #[test]
//         fn test_lpop_with_a_list_with_more_than_one_value() {
//             let database = database_with_a_list();

//             if let SuccessQuery::String(val) = database.lpop(KEY).unwrap() {
//                 assert_eq!(val.to_string(), VALUED);
//             }
//             let dictionary = database.dictionary.lock().unwrap();

//             if let StorageValue::List(list) = dictionary.get(KEY).unwrap() {
//                 assert_eq!(list[0], VALUEC);
//             }
//         }

//         #[test]
//         fn test_lpop_on_an_empty_list() {
//             let database = Database::new();

//             database.lpush(KEY, VALUE).unwrap();

//             if let SuccessQuery::String(val) = database.lpop(KEY).unwrap() {
//                 assert_eq!(val.to_string(), VALUE);
//             }

//             let value = database.lpop(KEY).unwrap();
//             assert_eq!(value, SuccessQuery::Nil);

//             let dictionary = database.dictionary.lock().unwrap();

//             if let StorageValue::List(list) = dictionary.get(KEY).unwrap() {
//                 assert!(list.is_empty());
//             }
//         }
//     }

//     mod lpush_test {

//         use super::*;

//         #[test]
//         fn test_lpush_on_a_non_existent_key_creates_a_list_with_new_value() {
//             let database = Database::new();

//             let result = database.lpush(KEY, VALUE).unwrap();
//             assert_eq!(result, SuccessQuery::Integer(1));

//             let dictionary = database.dictionary.lock().unwrap();

//             if let StorageValue::List(list) = dictionary.get(KEY).unwrap() {
//                 assert_eq!(list.len(), 1);
//                 assert_eq!(list[0], VALUE);
//             }
//         }

//         #[test]
//         fn test_lpush_on_an_existent_key_is_valid() {
//             let database = Database::new();

//             database.lpush(KEY, VALUEA).unwrap();

//             let result = database.lpush(KEY, VALUEB).unwrap();

//             assert_eq!(result, SuccessQuery::Integer(2));

//             let dictionary = database.dictionary.lock().unwrap();

//             if let StorageValue::List(list) = dictionary.get(KEY).unwrap() {
//                 assert_eq!(list.len(), 2);
//                 assert_eq!(list[1], VALUEA);
//                 assert_eq!(list[0], VALUEB);
//             }
//         }

//         #[test]
//         fn test_lpush_on_an_existent_key_that_isnt_a_list() {
//             let database = Database::new();

//             database.append(KEY, VALUE).unwrap();

//             let result = database.lpush(KEY, VALUE).unwrap_err();

//             assert_eq!(result, DataBaseError::NotAList);
//         }
//     }

//     mod lrange_test {
//         use super::*;

//         #[test]
//         fn test_lrange_on_zero_zero_range() {
//             let database = Database::new();

//             database.lpush(KEY, VALUE).unwrap();

//             if let SuccessQuery::List(list) = database.lrange(KEY, 0, 0).unwrap() {
//                 assert_eq!(list[0].to_string(), VALUE);
//             }
//         }

//         #[test]
//         fn test_lrange_on_zero_two_range_applied_to_a_list_with_four_elements() {
//             let database = database_with_a_list();

//             if let SuccessQuery::List(list) = database.lrange(KEY, 0, 2).unwrap() {
//                 let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();
//                 let second_list: Vec<&str> = vec![VALUED, VALUEC, VALUEB];
//                 let pair_list: Vec<(&String, &str)> = list.iter().zip(second_list).collect();
//                 pair_list.iter().for_each(|x| {
//                     assert_eq!(x.0, x.1);
//                 })
//             }
//         }

//         #[test]
//         fn test_lrange_on_one_three_range_applied_to_a_list_with_four_elements() {
//             let database = database_with_a_list();

//             if let SuccessQuery::List(list) = database.lrange(KEY, 1, 3).unwrap() {
//                 let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();
//                 let second_list: Vec<&str> = vec![VALUEC, VALUEB, VALUEA];
//                 let pair_list: Vec<(&String, &str)> = list.iter().zip(second_list).collect();
//                 pair_list.iter().for_each(|x| {
//                     assert_eq!(x.0, x.1);
//                 })
//             }
//         }

//         #[test]
//         fn test_lrange_on_a_superior_out_of_bound() {
//             let database = database_with_a_list();

//             let result = database.lrange(KEY, 1, 5).unwrap();

//             assert_eq!(result, SuccessQuery::List(Vec::new()));
//         }

//         #[test]
//         fn test_lrange_on_an_inferior_out_of_bound() {
//             let database = database_with_a_list();

//             let result = database.lrange(KEY, -10, 3).unwrap();

//             assert_eq!(result, SuccessQuery::List(Vec::new()));
//         }

//         #[test]
//         fn test_lrange_on_an_two_way_out_of_bound() {
//             let database = database_with_a_list();

//             let result = database.lrange(KEY, -10, 10).unwrap();

//             assert_eq!(result, SuccessQuery::List(Vec::new()));
//         }

//         #[test]
//         fn test_lrange_with_valid_negative_bounds() {
//             let database = database_with_a_list();

//             if let SuccessQuery::List(list) = database.lrange(KEY, -3, -1).unwrap() {
//                 let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();
//                 let second_list: Vec<&str> = vec![VALUEC, VALUEB, VALUEA];
//                 let pair_list: Vec<(&String, &str)> = list.iter().zip(second_list).collect();
//                 pair_list.iter().for_each(|x| {
//                     assert_eq!(x.0, x.1);
//                 })
//             }
//         }
//     }

//     mod lrem_test {
//         use super::*;

//         #[test]
//         fn test_lrem_2_on_a_list() {
//             let database = database_with_a_three_repeated_values();

//             let result = database.lrem(KEY, 2, VALUEA);

//             assert_eq!(SuccessQuery::Integer(2), result.unwrap());

//             let dictionary = database.dictionary.lock().unwrap();

//             if let Some(StorageValue::List(list)) = dictionary.get(KEY) {
//                 assert_eq!(list[0], VALUEC);
//                 assert_eq!(list[1], VALUEA);
//             }
//         }

//         #[test]
//         fn test_lrem_negative_2_on_a_list() {
//             let database = database_with_a_three_repeated_values();

//             let result = database.lrem(KEY, -2, VALUEA);

//             assert_eq!(SuccessQuery::Integer(2), result.unwrap());

//             let dictionary = database.dictionary.lock().unwrap();

//             if let StorageValue::List(list) = dictionary.get(KEY).unwrap() {
//                 assert_eq!(list[0], VALUEA);
//                 assert_eq!(list[1], VALUEC);
//             }
//         }

//         #[test]
//         fn test_lrem_zero_on_a_list() {
//             let database = database_with_a_three_repeated_values();

//             let result = database.lrem(KEY, 0, VALUEA);

//             let dictionary = database.dictionary.lock().unwrap();

//             assert_eq!(SuccessQuery::Integer(1), result.unwrap());
//             if let StorageValue::List(list) = dictionary.get(KEY).unwrap() {
//                 assert_eq!(list[0], VALUEC);
//             }
//         }

//         #[test]
//         fn test_lset_with_a_non_existen_key() {
//             let database = Database::new();

//             let result = database.lset(KEY, 0, &VALUEA).unwrap_err();

//             assert_eq!(result, DataBaseError::NonExistentKey);
//         }

//         #[test]
//         fn test_lset_with_a_value_that_isn_a_list() {
//             let database = database_with_a_string();

//             let result = database.lset(KEY, 0, &VALUEA).unwrap_err();

//             assert_eq!(result, DataBaseError::NotAList);
//         }

//         #[test]
//         fn test_lset_on_a_list_with_values() {
//             let database = database_with_a_list();

//             let result = database.lset(KEY, 0, &VALUEA);
//             assert_eq!(SuccessQuery::Success, result.unwrap());

//             let dictionary = database.dictionary.lock().unwrap();

//             if let StorageValue::List(list) = dictionary.get(KEY).unwrap() {
//                 assert_eq!(list[0], VALUEA);
//             }
//         }
//     }
// }

// #[cfg(test)]
// mod group_set {
//     use super::*;

//     const KEY: &str = "KEY";
//     const ELEMENT: &str = "ELEMENT";
//     const OTHER_ELEMENT: &str = "OTHER_ELEMENT";

//     #[test]
//     fn test_sadd_create_new_set_with_element_returns_1_if_key_set_not_exist_in_database() {
//         let database = Database::new();

//         let result = database.sadd(KEY, ELEMENT).unwrap();
//         assert_eq!(result, SuccessQuery::Boolean(true));
//         let is_member = database.sismember(KEY, ELEMENT).unwrap();
//         assert_eq!(is_member, SuccessQuery::Boolean(true));
//     }

//     #[test]
//     fn test_sadd_create_set_with_repeating_elements_returns_0() {
//         let database = Database::new();
//         database.sadd(KEY, ELEMENT).unwrap();

//         let result = database.sadd(KEY, ELEMENT).unwrap();
//         assert_eq!(result, SuccessQuery::Boolean(false));
//         let len_set = database.scard(KEY).unwrap();
//         assert_eq!(len_set, SuccessQuery::Integer(1));
//     }

//     #[test]
//     fn test_sadd_key_with_another_type_of_set_returns_err() {
//         let database = Database::new();
//         database.set(KEY, ELEMENT).unwrap();

//         let result = database.sadd(KEY, ELEMENT).unwrap_err();

//         assert_eq!(result, DataBaseError::NotASet);
//     }

//     #[test]
//     fn test_sadd_add_element_with_set_created_returns_1() {
//         let database = Database::new();
//         database.sadd(KEY, ELEMENT).unwrap();

//         let result = database.sadd(KEY, OTHER_ELEMENT).unwrap();
//         assert_eq!(result, SuccessQuery::Boolean(true));
//         let len_set = database.scard(KEY).unwrap();
//         assert_eq!(len_set, SuccessQuery::Integer(2));
//     }

//     #[test]
//     fn test_sismember_set_with_element_returns_1() {
//         let database = Database::new();
//         database.sadd(KEY, ELEMENT).unwrap();

//         let is_member = database.sismember(KEY, ELEMENT).unwrap();

//         assert_eq!(is_member, SuccessQuery::Boolean(true));
//     }

//     #[test]
//     fn test_sismember_set_without_element_returns_0() {
//         let database = Database::new();
//         database.sadd(KEY, ELEMENT).unwrap();

//         let result = database.sismember(KEY, OTHER_ELEMENT).unwrap();

//         assert_eq!(result, SuccessQuery::Boolean(false));
//     }

//     #[test]
//     fn test_sismember_key_with_another_type_of_set_returns_err() {
//         let database = Database::new();
//         database.set(KEY, ELEMENT).unwrap();
//         let result = database.sismember(KEY, ELEMENT).unwrap_err();

//         assert_eq!(result, DataBaseError::NotASet);
//     }

//     #[test]
//     fn test_sismember_with_non_exist_key_set_returns_0() {
//         let database = Database::new();

//         let result = database.sismember(KEY, ELEMENT).unwrap();

//         assert_eq!(result, SuccessQuery::Boolean(false));
//     }

//     #[test]
//     fn test_scard_set_with_one_element_returns_1() {
//         let database = Database::new();
//         database.sadd(KEY, ELEMENT).unwrap();

//         let len_set = database.scard(KEY).unwrap();

//         assert_eq!(len_set, SuccessQuery::Integer(1));
//     }

//     #[test]
//     fn test_scard_create_set_with_multiple_elements_returns_lenght_of_set() {
//         let database = Database::new();

//         for i in 0..10 {
//             let _ = database.sadd(KEY, &i.to_string());
//         }

//         let len_set = database.scard(KEY).unwrap();

//         assert_eq!(len_set, SuccessQuery::Integer(10));
//     }

//     #[test]
//     fn test_scard_key_with_another_type_of_set_returns_err() {
//         let database = Database::new();
//         database.set(KEY, ELEMENT).unwrap();

//         let result = database.scard(KEY).unwrap_err();

//         assert_eq!(result, DataBaseError::NotASet);
//     }

//     #[test]
//     fn test_scard_key_set_not_exist_in_database_returns_0() {
//         let database = Database::new();

//         let result = database.scard(KEY).unwrap();

//         assert_eq!(result, SuccessQuery::Boolean(false));
//     }
// }
