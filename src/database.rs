use crate::databasehelper::{
    DataBaseError, KeyTtl, MessageTtl, RespondTtl, SortFlags, StorageValue, SuccessQuery,
};
use crate::hash_shard::HashShard;
use crate::matcher::matcher;
use core::str;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt::{self, Formatter};
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::sync::mpsc::{self, channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

#[doc(hidden)]
type TtlVector = Arc<Mutex<Vec<KeyTtl>>>;

/// A Database implemented in a multithreading context.
///
/// Database uses Arc and Mutex to be shared safety in a multithreading context
/// implementing clone.
/// It also supervises the keys's time to live and save its own data every 30
/// seconds.
///
pub struct Database {
    #[doc(hidden)]
    dictionary: HashShard,
    #[doc(hidden)]
    ttl_msg_sender: Sender<MessageTtl>,
    #[doc(hidden)]
    db_dump_path: String,
}

#[doc(hidden)]
fn open_serializer(path: &str) -> Result<File, String> {
    match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(path)
    {
        Err(why) => Err(format!("Couldn't open file: {}", why)),
        Ok(file) => Ok(file),
    }
}

#[doc(hidden)]
fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

impl Database {
    /// Creates a new Database.
    ///
    /// If there's somenthing in path_to_dump.txt loads all the data there and run the
    /// ttl_supervisor, wich supervises the time to live for every key.
    /// # Examples
    /// Basic Usage:
    /// ```
    /// let mut database = Database("path_to_dump.txt");
    /// ```
    pub fn new(db_dump_path: String) -> Database {
        let (ttl_msg_sender, ttl_rec) = mpsc::channel();

        let mut database = Database {
            dictionary: HashShard::new(),
            ttl_msg_sender,
            db_dump_path,
        };

        database.ttl_supervisor_run(ttl_rec);

        if let Ok(lines) = read_lines(&database.db_dump_path) {
            let mut dic = database.dictionary.clone();
            let mut expires: Vec<(String, i64)> = Vec::new();

            for line in lines.flatten() {
                if &line[0..3] == "TTL" {
                    let ttl_list: Vec<&str> = line.split_whitespace().collect();

                    if ttl_list.len() > 1 {
                        let ttl_list: Vec<&str> = ttl_list[1].split(';').collect();
                        for &key_ttl in ttl_list.iter() {
                            if !key_ttl.is_empty() {
                                let key_ttl: Vec<&str> = key_ttl.split(',').collect();
                                //Get Key
                                let key: Vec<&str> = key_ttl[0].split(':').collect();
                                let key = key[1];
                                //Get TTL
                                let ttl: Vec<&str> = key_ttl[1].split(':').collect();
                                let ttl = ttl[1].parse::<i64>().unwrap();

                                expires.push((key.to_string(), ttl));
                            }
                        }
                    }
                } else {
                    let key_value: Vec<&str> = line.split(',').collect();

                    let key: Vec<&str> = key_value[0].split(':').collect();
                    let key = key[1];

                    let value = key_value[1];
                    if let Ok(value) = StorageValue::unserialize(value) {
                        dic.insert(key.to_string(), value);
                    }
                }
            }

            for (key, ttl) in expires {
                database.expireat(&key, ttl).unwrap();
            }
        }

        let serializer_db = database.clone();
        serializer_db.run_serializer();

        database
    }

    #[doc(hidden)]
    pub fn new_from_db(
        ttl_msg_sender: Sender<MessageTtl>,
        dictionary: HashShard,
        db_dump_path: String,
    ) -> Database {
        Database {
            dictionary,
            ttl_msg_sender,
            db_dump_path,
        }
    }

    #[doc(hidden)]
    pub fn run_serializer(&self) {
        let path = self.db_dump_path.clone();
        let dic = self.dictionary.clone();
        let ttl_msg_sender = self.ttl_msg_sender.clone();
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(30));
            let mut serializer = open_serializer(&path).unwrap();

            serializer.set_len(0).unwrap();

            let (sender, reciver) = channel();
            ttl_msg_sender.send(MessageTtl::AllTtL(sender)).unwrap();

            if let Err(e) = write!(serializer, "TTL ") {
                eprintln!("Couldn't write: {}", e);
            }

            if let Ok(RespondTtl::List(list)) = reciver.recv() {
                let guard = list.lock().unwrap();

                for key_ttl in guard.iter() {
                    let duration = key_ttl
                        .expire_time
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap();
                    if let Err(e) = write!(
                        serializer,
                        "Key:{},Ttl:{};",
                        key_ttl.key,
                        duration.as_secs()
                    ) {
                        eprintln!("Couldn't write: {}", e);
                    }
                }
            }

            if let Err(e) = writeln!(serializer) {
                eprintln!("Couldn't write: {}", e);
            }

            for (key, value) in dic.key_value() {
                if let Err(e) = writeln!(serializer, "Key:{},{}", key, value.serialize()) {
                    eprintln!("Couldn't write: {}", e);
                }
            }
        });
    }

    #[doc(hidden)]
    pub fn ttl_supervisor_run(&mut self, reciver: Receiver<MessageTtl>) {
        let mut dictionary = self.dictionary.clone();

        thread::spawn(move || {
            let ttl_keys: TtlVector = Arc::new(Mutex::new(Vec::new()));

            for message in reciver.iter() {
                match message {
                    MessageTtl::Expire(new_key_ttl) => {
                        let keys = ttl_keys.clone();
                        let mut keys_locked = keys.lock().unwrap();

                        if let Some(pos) = keys_locked.iter().position(|x| x.key == new_key_ttl.key)
                        {
                            let mut value = keys_locked.get_mut(pos).unwrap();
                            value.expire_time = new_key_ttl.expire_time;
                        } else {
                            match keys_locked.binary_search(&new_key_ttl) {
                                Ok(pos) => {
                                    keys_locked.remove(pos);
                                    keys_locked.insert(pos, new_key_ttl);
                                }
                                Err(pos) => {
                                    keys_locked.insert(pos, new_key_ttl);
                                }
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
                    MessageTtl::Clear(key) => {
                        let keys = ttl_keys.clone();
                        let mut keys_locked = keys.lock().unwrap();

                        if let Some(pos) = keys_locked.iter().position(|x| x.key == key) {
                            keys_locked.remove(pos);
                        }
                    }

                    MessageTtl::Transfer(from_key, to_key) => {
                        let keys = ttl_keys.clone();
                        let mut keys_locked = keys.lock().unwrap();

                        if let Some(pos) = keys_locked.iter().position(|x| x.key == from_key) {
                            let ttl = keys_locked.get_mut(pos).unwrap();
                            ttl.key = to_key;
                        }
                    }
                    MessageTtl::Ttl(key, sender_respond) => {
                        let keys = ttl_keys.clone();
                        let keys_locked = keys.lock().unwrap();

                        if let Some(pos) = keys_locked.iter().position(|x| x.key == key) {
                            let ttl = keys_locked.get(pos).unwrap();
                            sender_respond
                                .send(RespondTtl::Ttl(ttl.expire_time))
                                .unwrap();
                        } else {
                            sender_respond.send(RespondTtl::Persistent).unwrap();
                        }
                    }
                    MessageTtl::AllTtL(sender_respond) => {
                        sender_respond
                            .send(RespondTtl::List(ttl_keys.clone()))
                            .unwrap();
                    }
                    MessageTtl::Check(key) => {
                        let keys = ttl_keys.clone();
                        let mut keys_locked = keys.lock().unwrap();

                        if let Some(pos) = keys_locked.iter().position(|x| x.key == key) {
                            let ttl = keys_locked.get(pos).unwrap();
                            if ttl.expire_time < SystemTime::now() {
                                dictionary.remove(&ttl.key);
                                keys_locked.remove(pos);
                            }
                        }
                    }
                };
            }
        });
    }

    // KEYS

    /// Delete all the keys of the currently selected DB. This command never fails.
    ///
    /// Reply: SuccessQuery:Success
    ///
    /// # Examples
    /// ```
    /// let mut db = Database("path_to_dump.txt");
    ///
    /// db.mset(vec!["KEY1", "VALUE1", "KEY2", "VALUE2"]).unwrap();
    /// let r = db.get("KEY1").unwrap();
    /// assert_eq!(r, SuccessQuery::String("VALUE1"));
    /// let r = db.get("KEY2").unwrap();
    /// assert_eq!(r, SuccessQuery::String("VALUE2"));
    ///
    /// let r = db.flushdb().unwrap();
    /// assert_eq!(r, SuccessQuery::Success);
    ///
    /// let guard = db.dictionary;
    /// assert!(guard.len() == 0);
    /// ```
    pub fn flushdb(&mut self) -> Result<SuccessQuery, DataBaseError> {
        self.dictionary.clear();
        Ok(SuccessQuery::Success)
    }

    /// Return the number of keys in the currently-selected database. This command never fails.
    ///
    /// Reply: SuccessQuery:Integer(n) where n is the number of keys in currently database.
    ///
    /// # Examples
    /// ```
    /// let mut db = Database("path_to_dump.txt");
    /// let _ = db.set(KEY1, VALUE1);
    /// let r = db.get(KEY1).unwrap();
    /// assert_eq!(r, SuccessQuery::String(VALUE1.to_owned()));
    ///
    /// let r = db.dbsize().unwrap();
    /// assert_eq!(r, SuccessQuery::Integer(1));
    /// ```
    pub fn dbsize(&self) -> Result<SuccessQuery, DataBaseError> {
        Ok(SuccessQuery::Integer(self.dictionary.len() as i32))
    }

    /// This command copies the value stored at the source key to the destination key.
    ///
    /// The command returns an error when the destination key already exists.
    ///
    /// Reply: SuccessQuery:Success if copies sucessful. Database error in wrong case.
    ///
    /// # Examples
    /// ```
    /// let mut database = Database("path_to_dump.txt");
    ///
    /// database.set("KEY", "VALUE").unwrap();
    /// database.set("SECOND_KEY", "SECOND_VALUE").unwrap();
    /// let result = database.copy("KEY", "SECOND_KEY");
    ///
    /// assert_eq!(result.unwrap_err(), DataBaseError::KeyAlredyExist);
    /// ```
    pub fn copy(&mut self, key: &str, to_key: &str) -> Result<SuccessQuery, DataBaseError> {
        if self._exists(to_key) {
            return Err(DataBaseError::KeyAlredyExist);
        }

        if !self._exists(key) {
            return Err(DataBaseError::NonExistentKey);
        }

        self.dictionary.touch(key);
        let dictionary = self.dictionary.get_atomic_hash(key);
        let mut dictionary = dictionary.lock().unwrap();
        match dictionary.get(key) {
            Some(val) => {
                let val = val.clone();
                dictionary.insert(to_key.to_owned(), val);
                Ok(SuccessQuery::Success)
            }
            None => Err(DataBaseError::NonExistentKey),
        }
    }

    /// Removes the specified keys. A key is ignored if it does not exist.
    ///
    /// Reply: SuccessQuery:Integer(n) where n=1 if were removed or n=0 if were not removed.
    ///
    /// # Examples
    /// ```
    /// todo
    /// ```
    pub fn del(&mut self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        self.dictionary.remove(key);
        Ok(SuccessQuery::Success)
    }

    #[doc(hidden)]
    fn _exists(&self, key: &str) -> bool {
        let contains_key = self.dictionary.contains_key(key);
        let expire_time_passed = match self.get_expire_time(key) {
            RespondTtl::Ttl(expire_time) => expire_time < SystemTime::now(),
            _ => false,
        };

        !expire_time_passed && contains_key
    }

    /// Returns if key exists.
    ///
    /// Reply: SuccessQuery:Integer(n) where n is specified by:
    ///
    /// n=1 if the key exists.
    ///
    /// n=0 if the key does not exist.
    ///
    /// # Examples
    /// ```
    /// let mut database = Database("path_to_dump.txt");
    /// database.set("KEY", "VALUE").unwrap();
    ///
    /// let result = database.exists("KEY").unwrap();
    ///
    /// assert_eq!(result, SuccessQuery::Boolean(true));
    /// ```
    pub fn exists(&mut self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        self.dictionary.touch(key);
        Ok(SuccessQuery::Boolean(self._exists(key)))
    }

    /// Set a timeout on key. After the timeout has expired, the key will automatically be deleted.
    /// A key with an associated timeout is often said to be volatile in Redis terminology.
    ///
    /// The timeout will only be cleared by commands that delete or overwrite the contents of the key, including DEL, SET, GETSET and
    /// all the *STORE commands.
    ///
    /// This means that all the operations that conceptually alter the value stored at the key without replacing it with a new one will leave the timeout untouched.
    /// For instance, incrementing the value of a key with INCR, pushing a new value into a list with LPUSH,
    /// or altering the field value of a hash with HSET are all operations that will leave the timeout untouched.
    ///
    ///
    /// The timeout can also be cleared, turning the key back into a persistent key, using the PERSIST command.
    ///
    /// If a key is renamed with RENAME, the associated time to live is transferred to the new key name.
    ///
    /// If a key is overwritten by RENAME, like in the case of an existing key Key_A that is overwritten by a call like RENAME Key_B Key_A,
    /// it does not matter if the original Key_A had a timeout associated or not,
    /// the new key Key_A will inherit all the characteristics of Key_B.
    ///
    /// Reply: SuccessQuery:Integer(n) where n is specified by:
    ///
    /// n=1 if the timeout was set.
    ///
    /// n=0 if key does not exist.
    /// # Examples
    /// ```
    /// todo
    /// ```
    pub fn expire(&mut self, key: &str, seconds: i64) -> Result<SuccessQuery, DataBaseError> {
        if !self._exists(key) {
            return Ok(SuccessQuery::Boolean(false));
        }
        self.dictionary.touch(key);

        if seconds < 0 {
            self.dictionary.remove(key);
        } else {
            let duration = Duration::new(seconds as u64, 0);
            let expire_time = SystemTime::now().checked_add(duration).unwrap();
            let key_ttl = KeyTtl::new(key, expire_time);
            self.ttl_msg_sender
                .send(MessageTtl::Expire(key_ttl))
                .unwrap();
        }

        Ok(SuccessQuery::Boolean(true))
    }

    /// EXPIREAT has the same effect and semantic as EXPIRE, but instead of specifying the number of seconds representing the TTL (time to live),
    /// it takes an absolute Unix timestamp (seconds since January 1, 1970).
    ///
    /// A timestamp in the past will delete the key immediately.
    /// Please for the specific semantics of the command refer to the documentation of EXPIRE.
    ///
    /// Reply: SuccessQuery:Integer(n) where n is specified by:
    ///
    /// n=1 if the timeout was set.
    ///
    /// n=0 if key does not exist.
    /// # Examples
    /// ```
    /// todo
    /// ```
    pub fn expireat(&mut self, key: &str, seconds: i64) -> Result<SuccessQuery, DataBaseError> {
        if !self._exists(key) {
            return Ok(SuccessQuery::Boolean(false));
        }
        self.dictionary.touch(key);

        let duration = Duration::new(seconds as u64, 0);
        let expire_time = SystemTime::UNIX_EPOCH.checked_add(duration).unwrap();

        if expire_time < SystemTime::now() {
            self.dictionary.remove(key);
        } else {
            let key_ttl = KeyTtl::new(key, expire_time);
            self.ttl_msg_sender
                .send(MessageTtl::Expire(key_ttl))
                .unwrap();
        }

        Ok(SuccessQuery::Boolean(true))
    }

    /// Returns all keys matching pattern.
    ///
    /// This command is intended for debugging and special operations, such as changing your keyspace layout.
    ///
    /// Supported glob-style patterns:
    ///
    /// h?llo matches hello, hallo and hxllo
    ///
    /// h*llo matches hllo and heeeello
    ///
    /// h{ae}llo matches hello and hallo, but not hillo
    ///
    /// h{^e}llo matches hallo, hbllo, ... but not hello
    ///
    /// h{a-b}llo matches hallo and hbllo
    ///
    /// list of keys matching pattern.
    ///
    /// Reply: SuccessQuery:List(list) where list is an array of keys matching pattern.
    ///
    /// # Examples
    /// ```
    /// let mut database = Database("path_to_dump.txt");
    /// database.set("firstname", "alex").unwrap();
    /// database.set("lastname", "arbieto").unwrap();
    /// database.set("age", "22").unwrap();
    ///
    /// if let Ok(SuccessQuery::List(list)) = database.keys("????name") {
    ///     let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();
    ///
    ///     assert!(list.contains("lastname"));
    /// }
    /// ```
    /// other example with * pattern:
    ///
    /// ```
    /// let mut database = Database("path_to_dump.txt");;
    /// database.set("firstname", "alex").unwrap();
    /// database.set("lastname", "arbieto").unwrap();
    /// database.set("age", "22").unwrap();
    ///
    /// if let Ok(SuccessQuery::List(list)) = database.keys("*name") {
    ///     let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();
    ///
    ///     assert!(list.contains("firstname"));
    ///     assert!(list.contains("lastname"));
    /// }
    /// ```
    pub fn keys(&mut self, pattern: &str) -> Result<SuccessQuery, DataBaseError> {
        let keys = self.dictionary.keys();
        let list: Vec<SuccessQuery> = keys
            .iter()
            .filter(|x| matcher(x, pattern))
            .map(|item| SuccessQuery::String(item.to_string()))
            .collect::<Vec<SuccessQuery>>();

        Ok(SuccessQuery::List(list))
    }

    /// Remove the existing timeout on key, turning the key from volatile (a key with an expire set)
    /// to persistent (a key that will never expire as no timeout is associated).
    ///
    /// Reply: SuccessQuery:Integer(n) where n is specified by:
    ///
    /// n=1 if the timeout was removed.
    ///
    /// n=0 if key does not exist or does not have an associated timeout.
    /// # Examples
    /// ```
    /// todo
    /// ```
    pub fn persist(&mut self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        if !self._exists(key) {
            return Err(DataBaseError::NonExistentKey);
        }
        self.dictionary.touch(key);

        self.ttl_msg_sender
            .send(MessageTtl::Clear(key.to_owned()))
            .unwrap();

        Ok(SuccessQuery::Boolean(true))
    }

    /// Renames key to newkey.
    ///
    /// It returns an error when key does not exist.
    ///
    /// If newkey already exists it is overwritten, when this happens RENAME executes an implicit DEL operation,
    /// so if the deleted key contains a very big value it may cause high latency even if RENAME itself is usually a constant-time operation.
    ///
    /// Reply: SuccessQuery::Success if success result or DatabaseError::NonExistentKey if key does not exists.
    ///
    /// # Examples
    /// ```
    /// let mut database = Database("path_to_dump.txt");
    /// database.set("KEY", "VALUE").unwrap();
    ///
    /// let result = database.rename("KEY", "SECOND_KEY").unwrap();
    /// assert_eq!(result, SuccessQuery::Success);
    ///
    /// let result = database.get("KEY").unwrap();
    /// assert_eq!(result, SuccessQuery::Nil);
    /// ```
    pub fn rename(&mut self, old_key: &str, new_key: &str) -> Result<SuccessQuery, DataBaseError> {
        if !self._exists(old_key) {
            return Err(DataBaseError::NonExistentKey);
        }

        match self.dictionary.remove(old_key) {
            Some(value) => {
                self.dictionary.insert(new_key.to_owned(), value);

                self.ttl_msg_sender
                    .send(MessageTtl::Transfer(old_key.to_owned(), new_key.to_owned()))
                    .unwrap();

                Ok(SuccessQuery::Success)
            }
            None => Err(DataBaseError::NonExistentKey),
        }
    }

    #[doc(hidden)]
    pub fn sort_by(
        &mut self,
        to_order: &mut Vec<String>,
        pattern: &str,
    ) -> Result<Vec<String>, DataBaseError> {
        let mut list_elem_weight: Vec<(&str, i32)> = Vec::new();

        let keys = self.dictionary.keys();
        let list_key_match = keys
            .iter()
            .filter(|x| matcher(x, pattern) && self._exists(x))
            .collect::<Vec<&String>>();

        for (i, elem) in to_order.iter().enumerate() {
            let without_weight = 0;
            for pal in list_key_match.iter() {
                if pal.contains(&elem.to_owned()) {
                    let dictionary = self.dictionary.get_atomic_hash(&(*pal).clone());
                    let dictionary = dictionary.lock().unwrap();
                    if let Some((StorageValue::String(val), _)) = dictionary.get(&(*pal).clone()) {
                        let result_weight = match val.parse::<i32>() {
                            Ok(weight_ok) => Ok(weight_ok),
                            Err(_) => Err(DataBaseError::SortByParseError),
                        };
                        if let Ok(weight) = result_weight {
                            list_elem_weight.push((elem, weight));
                            break;
                        } else {
                            return Err(DataBaseError::SortByParseError);
                        }
                    }
                }
            }
            if list_elem_weight.len() < i + 1 {
                list_elem_weight.push((elem, without_weight));
            }
        }

        if list_elem_weight.is_empty() {
            return Ok(to_order.to_vec());
        }

        list_elem_weight.sort_by(|a, b| a.1.cmp(&b.1));

        let to_build: Vec<String> = list_elem_weight.iter().map(|x| x.0.to_string()).collect();
        Ok(to_build)
    }

    #[doc(hidden)]
    pub fn limit(
        to_order: &mut Vec<String>,
        pos_begin: &mut i32,
        num_elems: &mut i32,
    ) -> Vec<String> {
        let empty_list = Vec::new();
        if *pos_begin >= to_order.len() as i32 {
            return empty_list;
        }
        if *num_elems == 0 {
            return empty_list;
        }
        if *pos_begin < 0 {
            *pos_begin = 0;
        }
        if *num_elems < 0 || *num_elems > to_order.len() as i32 {
            *num_elems = to_order.len() as i32;
        }
        to_order.to_vec()
    }

    #[doc(hidden)]
    pub fn sort_limit(
        to_order: &mut Vec<String>,
        pos_begin: &mut i32,
        num_elems: &mut i32,
    ) -> Result<Vec<String>, DataBaseError> {
        let mut to_build: Vec<String> = Database::limit(to_order, pos_begin, num_elems);
        Database::sort_without_flags(&mut to_build)
    }

    #[doc(hidden)]
    pub fn desc(to_order: &mut Vec<String>) -> Vec<String> {
        to_order.reverse();
        to_order.to_vec()
    }

    #[doc(hidden)]
    pub fn sort_desc(to_order: &mut Vec<String>) -> Result<Vec<String>, DataBaseError> {
        let mut parse_error = false;
        let mut to_order: Vec<_> = to_order
            .iter()
            .map(|x| match x.parse::<i32>() {
                Ok(val) => val,
                Err(_) => {
                    parse_error = true;
                    -1
                }
            })
            .collect();
        if parse_error {
            return Err(DataBaseError::SortParseError);
        }
        to_order.sort_by(|a, b| b.cmp(&a));
        let to_build: Vec<String> = to_order.iter().map(|x| x.to_string()).collect();
        Ok(to_build)
    }

    #[doc(hidden)]
    pub fn sort_without_flags(to_order: &mut Vec<String>) -> Result<Vec<String>, DataBaseError> {
        let mut parse_error = false;
        let mut to_order: Vec<_> = to_order
            .iter()
            .map(|x| match x.parse::<i32>() {
                Ok(val) => val,
                Err(_) => {
                    parse_error = true;
                    -1
                }
            })
            .collect();
        if parse_error {
            return Err(DataBaseError::SortParseError);
        }
        to_order.sort_unstable();
        let to_build: Vec<String> = to_order.iter().map(|x| x.to_string()).collect();
        Ok(to_build)
    }

    #[doc(hidden)]
    pub fn build_sort_vector(
        to_build: Vec<String>,
        pos_begin: &i32,
        num_elems: &i32,
    ) -> Result<Vec<SuccessQuery>, DataBaseError> {
        let mut result_list: Vec<SuccessQuery> = Vec::new();
        for i in *pos_begin..(pos_begin + num_elems) {
            result_list.push(SuccessQuery::String((&to_build[i as usize]).to_string()));
        }
        Ok(result_list)
    }

    #[doc(hidden)]
    pub fn _sort(
        &mut self,
        to_order: &mut Vec<String>,
        sort_flags: SortFlags,
    ) -> Result<Vec<SuccessQuery>, DataBaseError> {
        let mut num_elems = to_order.len() as i32;
        let mut pos_begin = 0;
        let mut to_order: Vec<String> = to_order.iter().map(|x| x.to_string()).collect();

        match sort_flags {
            SortFlags::WithoutFlags => match Database::sort_without_flags(&mut to_order) {
                Ok(to_build) => Database::build_sort_vector(to_build, &pos_begin, &num_elems),
                Err(err) => Err(err),
            },
            SortFlags::Alpha => {
                to_order.sort();
                Database::build_sort_vector(to_order, &pos_begin, &num_elems)
            }
            SortFlags::Desc => match Database::sort_desc(&mut to_order) {
                Ok(to_build) => Database::build_sort_vector(to_build, &pos_begin, &num_elems),
                Err(err) => Err(err),
            },
            SortFlags::Limit(mut pos_ini, mut num_elem) => {
                match Database::sort_limit(&mut to_order, &mut pos_ini, &mut num_elem) {
                    Ok(to_build) => Database::build_sort_vector(to_build, &pos_ini, &num_elem),
                    Err(err) => Err(err),
                }
            }
            SortFlags::By(pattern) => match self.sort_by(&mut to_order, pattern) {
                Ok(to_build) => Database::build_sort_vector(to_build, &pos_begin, &num_elems),
                Err(err) => Err(err),
            },
            SortFlags::CompositeFlags(sort_flags) => {
                if sort_flags
                    .iter()
                    .any(|s| matches!(s, SortFlags::Limit(_, _)))
                    && sort_flags.iter().any(|s| matches!(s, SortFlags::Desc))
                    && sort_flags.len() == 2
                {
                    return match Database::sort_desc(&mut to_order) {
                        Ok(mut to_build) => {
                            let to_build: Vec<String> =
                                Database::limit(&mut to_build, &mut pos_begin, &mut num_elems);
                            Database::build_sort_vector(to_build, &pos_begin, &num_elems)
                        }
                        Err(err) => Err(err),
                    };
                }

                if sort_flags.iter().any(|s| matches!(s, SortFlags::By(_))) {
                    let pattern: Option<String> = sort_flags.iter().find_map(|d| match d {
                        SortFlags::By(pattern) => Some(pattern.to_string()),
                        _ => None,
                    });

                    if let Ok(to_build) = self.sort_by(&mut to_order, &pattern.clone().unwrap()) {
                        to_order = to_build;
                    } else if let Err(err) = self.sort_by(&mut to_order, &pattern.unwrap()) {
                        return Err(err);
                    }
                }

                if sort_flags.iter().any(|s| matches!(s, SortFlags::Alpha))
                    && !sort_flags.iter().any(|s| matches!(s, SortFlags::By(_)))
                {
                    to_order.sort();
                }

                for flag in sort_flags {
                    if let SortFlags::Desc = flag {
                        to_order = Database::desc(&mut to_order);
                    }
                    if let SortFlags::Limit(l_pos_begin, l_num_elems) = flag {
                        pos_begin = l_pos_begin;
                        num_elems = l_num_elems;
                        to_order = Database::limit(&mut to_order, &mut pos_begin, &mut num_elems);
                    }
                }
                Database::build_sort_vector(to_order, &pos_begin, &num_elems)
            }
        }
    }

    /// # Sort in simplest Form
    ///
    /// Returns or stores the elements contained in the list or set at key. By default,
    /// sorting is numeric and elements are compared by their value interpreted as double precision floating point number.
    ///
    /// `database.sort("key", SortingFlags::WithoutFlags)`
    ///
    /// # Sort with Params
    ///
    /// Assuming key is a list of numbers, this command will return the same list with the elements sorted from small to large.
    ///
    /// In order to sort the numbers from large to small to large. use the DESC modifier:
    ///
    /// `database.sort("key", SortFlags::Desc)`
    ///
    /// When mylist contains string values and you want to sort them lexicographically, use the ALPHA modifier:
    ///
    /// `database.sort("key", SortFlags::Alpha)`
    ///
    /// The number of returned elements can be limited using the LIMIT modifier.
    ///
    /// This modifier takes the offset argument, specifying the number of elements to skip and the count argument,
    /// specifying the number of elements to return from starting at offset.
    ///
    /// The following example will return 10 elements of the sorted version of mylist, starting at element 0 (offset is zero-based):
    ///
    /// `database.sort("key", SortFlags::Limit(0, 10))`
    ///
    /// Almost all modifiers can be used together.
    /// The following example will return the first 5 elements, lexicographically sorted in descending order:
    ///
    /// `database.sort("key", SortFlags::CompositeFlags(![SortFlags::Limit(0,5), SortingFlags::Alpha, SortingFlags::Desc]))`
    ///
    /// # Sorting By External Keys
    ///
    /// Sometimes you want to sort elements using external keys as weights to compare instead of comparing the actual elements in the list,
    /// set or sorted set.
    ///
    /// Let's say the list mylist contains the elements 1, 2 and 3 representing unique IDs of objects stored in object_1, object_2 and object_3.
    /// When these objects have associated weights stored in weight_1, weight_2 and weight_3,
    /// SORT can be instructed to use these weights to sort mylist with the following statement:
    ///
    /// `database.sort("key", SortingFlags::By("weight_*"))`
    ///
    /// The BY option takes a pattern (equal to weight_* in this example) that is used to generate the keys that are used for sorting.
    ///
    /// These key names are obtained substituting the first occurrence of * with the actual value of the element in the list (1, 2 and 3 in this example).
    ///
    /// Reply: without passing the store option the command returns a SuccessQuery::List(list) where list is an array of sorted elements.
    ///
    /// # Examples
    /// ```
    /// let mut database = Database("path_to_dump.txt");
    /// database
    ///     .lpush("LIST", ["3", "1", "2"].to_vec())
    ///     .unwrap();
    ///
    /// if let SuccessQuery::List(list) = database.sort("LIST", SortFlags::WithoutFlags).unwrap()
    /// {
    ///     let list_result: Vec<String> = list.iter().map(|x| x.to_string()).collect();
    ///     let to_compare_list: Vec<&str> = vec!["1", "2", "3"];
    ///     let pair_list: Vec<(&String, &str)> =
    ///         list_result.iter().zip(to_compare_list).collect();
    ///     pair_list.iter().for_each(|x| {
    ///         assert_eq!(x.0, x.1);
    ///     });
    /// }
    /// ```
    pub fn sort(
        &mut self,
        key: &str,
        sort_flags: SortFlags,
    ) -> Result<SuccessQuery, DataBaseError> {
        if !self._exists(key) {
            return Ok(SuccessQuery::List(Vec::new()));
        }
        self.dictionary.touch(key);
        let dictionary = self.dictionary.get_atomic_hash(key);
        let dictionary = dictionary.lock().unwrap();
        let mut to_order = match dictionary.get(key) {
            Some((StorageValue::Set(hash_set), _)) => {
                hash_set.iter().map(|s| s.to_string()).collect()
            }
            Some((StorageValue::List(list), _)) => list.iter().map(|x| x.to_string()).collect(),
            Some(_) => return Err(DataBaseError::NotAList),
            None => return Ok(SuccessQuery::List(Vec::new())),
        };
        drop(dictionary);

        match self._sort(&mut to_order, sort_flags) {
            Ok(result_list) => Ok(SuccessQuery::List(result_list)),
            Err(e) => Err(e),
        }
    }

    /// Update the key's last access value
    ///
    /// Indicates in the log file the last previous access or the time since that previous access.
    ///
    /// Check if the key expiration condition (TTL) was met and if so, perform the deletion
    ///
    /// Reply: SuccessQuery::Integer(n) where n is specified by .
    ///
    /// n=1 if the key was be touched.
    ///
    /// n=0 if key does not be touch.
    ///
    /// # Examples
    /// ```
    /// let mut database = Database("path_to_dump.txt");
    ///
    ///
    ///
    /// ```
    pub fn touch(&mut self, key: &str) -> Option<u64> {
        self.ttl_msg_sender
            .send(MessageTtl::Check(key.to_string()))
            .unwrap();
        match self.dictionary.touch(key) {
            Some(t) => {
                self.ttl_msg_sender
                    .send(MessageTtl::Check(key.to_string()))
                    .unwrap();
                Some(t)
            }
            None => None,
        }
    }

    #[doc(hidden)]
    fn get_expire_time(&self, key: &str) -> RespondTtl {
        let (respond_sender, respond_reciver) = channel();

        self.ttl_msg_sender
            .send(MessageTtl::Ttl(key.to_owned(), respond_sender))
            .unwrap();

        respond_reciver.recv().unwrap()
    }

    /// Returns the remaining time to live of a key that has a timeout.
    /// This introspection capability allows a Redis client to check how many seconds a given key will continue to be part of the dataset.
    ///
    /// Reply: TTL in seconds, or a negative value in order to signal an error (see the description above).
    /// SuccessQuery::Integer(n) where n is the time to live of the key.
    ///
    /// SuccessQuery::Integer(n) when n is negative if an error ocurred according to:
    ///
    /// -2 if the key does not exist.
    ///
    /// -1 if the key exists but has no associated expire.
    ///
    /// # Examples
    /// ```
    /// todo
    /// ```
    pub fn ttl(&mut self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        if !self._exists(key) {
            return Ok(SuccessQuery::Integer(-2));
        }

        match self.get_expire_time(key) {
            RespondTtl::Ttl(time) => {
                let duration = time.duration_since(SystemTime::now()).unwrap();
                Ok(SuccessQuery::Integer(duration.as_secs() as i32))
            }
            RespondTtl::Persistent => Ok(SuccessQuery::Integer(-1)),
            _ => Ok(SuccessQuery::Integer(-1)),
        }
    }

    /// Returns the string representation of the type of the value stored at key.
    /// The different types that can be returned are: string, list, set.
    ///
    /// Reply: SuccessQuery::String(s) where s is type of key or none when key does not exist.
    ///
    /// # Examples
    /// ```
    /// todo
    /// ```
    pub fn get_type(&mut self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        if !self._exists(key) {
            return Ok(SuccessQuery::String("none".to_owned()));
        }

        let dictionary = self.dictionary.get_atomic_hash(key);
        let mut dictionary = dictionary.lock().unwrap();
        match dictionary.get_mut(key) {
            Some((val, last_access)) => {
                *last_access = SystemTime::now();
                Ok(SuccessQuery::String(val.get_type()))
            }
            None => Ok(SuccessQuery::String("none".to_owned())),
        }
    }

    //STRINGS

    /// This command appends the value at the end of the string, if key already exists and is a string.
    /// If key does not exist it is created and set as an empty string, so APPEND will be similar to SET in this special case.
    ///
    /// Returns: SuccessQuery of the length of the string after the append operation.
    ///
    /// # Examples
    ///
    /// ```
    /// let database = Database::new("dump_path.txt");
    /// if let SuccessQuery::Integer(lenght) = database.append("key", "value").unwrap() {
    ///     assert_eq!(lenght, 5);
    /// }
    /// ```
    pub fn append(&mut self, key: &str, value: &str) -> Result<SuccessQuery, DataBaseError> {
        if self._exists(key) {
            let dictionary = self.dictionary.get_atomic_hash(key);
            let mut dictionary = dictionary.lock().unwrap();
            if let Some((StorageValue::String(val), last_access)) = dictionary.get_mut(key) {
                val.push_str(&value);
                let len_result = val.len() as i32;
                *last_access = SystemTime::now();
                Ok(SuccessQuery::Integer(len_result))
            } else {
                Err(DataBaseError::NotAString)
            }
        } else {
            let len_result = value.len() as i32;
            self.dictionary
                .insert(key.to_owned(), StorageValue::String(value.to_string()));
            Ok(SuccessQuery::Integer(len_result))
        }
    }

    /// Decrements the number stored at key by decrement.
    /// If the key does not exist, it is set to 0 before performing the operation.
    /// An error is returned if the key contains a value of the wrong type or contains a string that can not be represented as integer.
    /// This operation is limited to 64 bit signed integers.
    ///
    /// # Examples
    /// ```
    /// let database = Database::new("dump_path.txt");
    /// let database = create_database();
    ///
    /// database.set(KEY, "5").unwrap();
    /// let result = database.decrby(KEY, 4).unwrap();
    ///
    /// assert_eq!(result, SuccessQuery::Integer(1));
    /// ```
    pub fn decrby(&mut self, key: &str, decr: i32) -> Result<SuccessQuery, DataBaseError> {
        if !self._exists(key) {
            self.dictionary
                .insert(key.to_string(), StorageValue::String("0".to_string()));
        }

        let dictionary = self.dictionary.get_atomic_hash(key);
        let mut dictionary = dictionary.lock().unwrap();
        if let Some((StorageValue::String(val), _)) = dictionary.get(key) {
            let val = match val.parse::<i32>() {
                Ok(val) => val - decr,
                Err(_) => return Err(DataBaseError::NotAnInteger),
            };

            dictionary.insert(
                key.to_owned(),
                (StorageValue::String(val.to_string()), SystemTime::now()),
            );
            Ok(SuccessQuery::Integer(val))
        } else {
            Err(DataBaseError::NotAString)
        }
    }

    /// Get the value of key.
    /// If the key does not exist the special value nil is returned.
    /// An error is returned if the value stored at key is not a string, because GET only handles string values.
    ///
    /// Returns value: SuccessQuery::String with the value of key, or DataBaseError::NonExistentKey when key does not exist.
    ///
    /// # Examples
    /// ```
    /// let db = Database::new("dump_path.txt");
    /// db.set("KEY", "VALUE").unwrap();
    /// if let SuccessQuery::String(value) = database.get("KEY").unwrap() {
    ///         assert_eq!("VALUE", value.to_string());
    /// }
    /// ```
    pub fn get(&mut self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        if !self._exists(key) {
            return Ok(SuccessQuery::Nil);
        }

        self.dictionary.touch(key);
        let dictionary = self.dictionary.get_atomic_hash(key);
        let mut dictionary = dictionary.lock().unwrap();
        match dictionary.get_mut(key) {
            Some((StorageValue::String(val), _)) => Ok(SuccessQuery::String(val.clone())),
            Some(_) => Err(DataBaseError::NotAString),
            None => Ok(SuccessQuery::Nil),
        }
    }

    /// Get the value of key and delete the key.
    /// This command is similar to GET, except for the fact that it also deletes the key on success
    /// (if and only if the key's value type is a string).
    ///
    /// Returns value: the value of key, nil when key does not exist, or an error if the key's value type isn't a string.
    ///
    /// # Examples
    /// ```
    /// let database = Database::new("path_to_dump.txt");
    /// database.set("KEY", "VALUE").unwrap();
    /// let database = create_database_with_string();
    ///
    /// if let SuccessQuery::String(value) = database.getdel("KEY").unwrap() {
    ///     assert_eq!(value.to_string(), "VALUE");
    /// }
    ///
    /// let result = database.get("KEY").unwrap_err();
    /// assert_eq!(result, DataBaseError::NonExistentKey);
    /// ```
    pub fn getdel(&mut self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        self.ttl_msg_sender
            .send(MessageTtl::Clear(key.to_owned()))
            .unwrap();

        match self.get(key) {
            Ok(SuccessQuery::String(val)) => {
                self.dictionary.remove(key);
                Ok(SuccessQuery::String(val))
            }
            other => other,
        }
    }

    /// Atomically sets key to value and returns the old value stored at key.
    /// Returns an error when key exists but does not hold a string value.
    /// Any previous time to live associated with the key is discarded on successful SET operation.
    ///
    /// Returns value: the old value stored at key, or nil when key did not exist.
    pub fn getset(&mut self, key: &str, new_val: &str) -> Result<SuccessQuery, DataBaseError> {
        let old_value = match self.get(key) {
            Ok(SuccessQuery::String(old_value)) => old_value,
            other => return other,
        };

        self.ttl_msg_sender
            .send(MessageTtl::Clear(key.to_owned()))
            .unwrap();

        self.dictionary
            .insert(key.to_owned(), StorageValue::String(new_val.to_owned()));

        Ok(SuccessQuery::String(old_value))
    }

    /// Increments the number stored at key by increment.
    /// If the key does not exist, it is set to 0 before performing the operation.
    /// An error is returned if the key contains a value of the wrong type or contains a string that can not be represented as integer.
    /// This operation is limited to 64 bit signed integers.
    ///
    /// # Examples
    ///
    /// ```
    /// let database = Database::new("dump_path.txt");
    ///
    /// database.set(KEY, "1").unwrap();
    ///
    /// let result = database.incrby(KEY, 4).unwrap();
    ///
    /// assert_eq!(result, SuccessQuery::Integer(5));
    ///
    /// ```
    pub fn incrby(&mut self, key: &str, incr: i32) -> Result<SuccessQuery, DataBaseError> {
        self.decrby(key, -incr)
    }

    /// Returns the values of all specified keys.
    /// For every key that does not hold a string value or does not exist, the special value nil is returned.
    /// Because of this, the operation never fails.
    /// Returns: list of SuccessQuery::String values at the specified keys.
    /// # Examples
    /// ```
    /// let mut database = Database::new("dump_path.txt");
    /// database.set("KEY_A", "VALUE_A").unwrap();
    /// database.set("KEY_B", "VALUE_B").unwrap();
    ///
    /// let vec_keys = vec!["KEY_A", "KEY_B", "KEY_C", "KEY_D"];
    ///
    /// if let SuccessQuery::List(list) = database.mget(vec_keys).unwrap() {
    ///     assert_eq!(list[0], SuccessQuery::String("VALUE_A"));
    ///     assert_eq!(list[1], SuccessQuery::String("VALUE_B"));
    ///     assert_eq!(list[2], SuccessQuery::Nil);
    ///     assert_eq!(list[3], SuccessQuery::Nil);
    /// }
    /// ```
    pub fn mget(&mut self, params: Vec<&str>) -> Result<SuccessQuery, DataBaseError> {
        let mut list: Vec<SuccessQuery> = Vec::new();

        for key in params {
            let r = self.get(key);
            match r {
                Err(DataBaseError::NotAString) => {
                    list.push(SuccessQuery::Nil);
                }
                other => {
                    list.push(other.unwrap());
                }
            }
        }

        Ok(SuccessQuery::List(list))
    }

    /// Sets the given keys to their respective values. MSET replaces existing values with new values, just as regular SET.
    ///
    /// Reply: always OK since MSET can't fail.
    ///
    /// # Examples
    /// ```
    /// let mut database = Database::new("dump_path.txt");
    ///
    /// let vec_key_value = vec![
    ///     "KEY_A", "VALUE_A", "KEY_B", "VALUE_B", "KEY_C", "VALUE_C", "KEY_D", "VALUE_D",
    /// ];
    ///
    /// let result = database.mset(vec_key_value).unwrap();
    /// assert_eq!(result, SuccessQuery::Success);
    ///
    /// let result_get1 = database.get("KEY_A").unwrap();
    /// let result_get2 = database.get("KEY_B").unwrap();
    /// let result_get3 = database.get("KEY_C").unwrap();
    ///
    /// assert_eq!(result_get1, SuccessQuery::String("VALUE_A"));
    /// assert_eq!(result_get2, SuccessQuery::String("VALUE_B"));
    /// assert_eq!(result_get3, SuccessQuery::String("VALUE_C"));
    /// ```
    pub fn mset(&mut self, params: Vec<&str>) -> Result<SuccessQuery, DataBaseError> {
        for i in (0..params.len()).step_by(2) {
            let key = params.get(i).unwrap();
            let value = params.get(i + 1).unwrap();

            let _ = self.set(key, value);
        }
        Ok(SuccessQuery::Success)
    }

    /// Set key to hold the string value.
    /// If key already holds a value, it is overwritten, regardless of its type.
    /// Any previous time to live associated with the key is discarded on successful SET operation.
    ///
    /// Reply: OK. SET was always executed correctly.
    ///
    /// # Example
    /// ```
    /// let database = Database::new("dump_path.txt");
    ///
    /// let result = database.set("KEY","VALUE").unwrap();
    /// assert_eq!(SuccessQuery::Success, result);
    ///
    /// if let SuccessQuery::String(value) = database.get("KEY").unwrap() {
    ///     assert_eq!("VALUE", value);
    /// }
    /// ```
    pub fn set(&mut self, key: &str, val: &str) -> Result<SuccessQuery, DataBaseError> {
        self.ttl_msg_sender
            .send(MessageTtl::Clear(key.to_owned()))
            .unwrap();

        self.dictionary
            .insert(key.to_owned(), StorageValue::String(val.to_owned()));

        Ok(SuccessQuery::Success)
    }

    /// Returns the length of the string value stored at key.
    /// An error is returned when key holds a non-string value.
    ///
    /// Reply: SuccessQuery::Integer with the length of the string at key, or 0 when key does not exist.
    /// # Examples
    /// ```
    /// let database = Database::new("dump_path.txt");
    ///
    /// database.set("KEY", "VALUE").unwrap();
    ///
    /// if let SuccessQuery::Integer(value) = database.strlen("KEY").unwrap() {
    ///     assert_eq!(value, 5);
    /// }
    /// ```
    pub fn strlen(&mut self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        match self.get(key) {
            Ok(SuccessQuery::String(val)) => Ok(SuccessQuery::Integer(val.len() as i32)),
            other => other,
        }
    }

    // Lists

    /// Returns the element at index in the list stored at key.
    /// The index is zero-based, so 0 means the first element, 1 the second element and so on.
    /// Negative indices can be used to designate elements starting at the tail of the list.
    /// Here, -1 means the last element, -2 means the penultimate and so forth.
    /// When the value at key is not a list, an error is returned.
    ///
    /// Reply: SuccessQuery::String(val) when val is the requested element, or SuccessQuery::Nil when index is out of range.
    /// # Examples
    /// ```
    /// let mut database = Database::new("dump_path.txt");
    ///
    /// database.lpush("KEY", ["VALUE"].to_vec()).unwrap();
    ///
    /// if let SuccessQuery::String(value) = database.lindex("KEY", 0).unwrap() {
    ///     assert_eq!(value, "VALUE");
    /// }
    /// ```
    pub fn lindex(&mut self, key: &str, index: i32) -> Result<SuccessQuery, DataBaseError> {
        if !self._exists(key) {
            return Ok(SuccessQuery::Nil);
        }
        let dictionary = self.dictionary.get_atomic_hash(key);
        let mut dictionary = dictionary.lock().unwrap();
        match dictionary.get_mut(key) {
            Some((StorageValue::List(list), last_access)) => {
                let index = if index < 0 {
                    ((list.len() as i32) + index) as usize
                } else {
                    index as usize
                };

                match list.get(index) {
                    Some(val) => {
                        *last_access = SystemTime::now();
                        Ok(SuccessQuery::String(val.clone()))
                    }
                    None => Ok(SuccessQuery::Nil),
                }
            }
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(SuccessQuery::Nil),
        }
    }

    /// Returns the length of the list stored at key.
    /// If key does not exist, it is interpreted as an empty list and 0 is returned.
    /// An error is returned when the value stored at key is not a list.
    ///
    /// Reply: SuccessQuery::Integer(n) when n is the the length of the list at key.
    /// # Examples
    /// ```
    /// let mut database = Database::new("dump_path.txt");
    ///
    /// let database = database.lpush("KEY", ["VALUE_A","VALUE_B", "VALUE_C"].to_vec()).unwrap();
    ///
    /// let result = database.llen("KEY").unwrap();
    ///
    /// assert_eq!(result, SuccessQuery::Integer(3));
    /// ```
    pub fn llen(&mut self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        if !self._exists(key) {
            return Ok(SuccessQuery::Integer(0));
        }
        let dictionary = self.dictionary.get_atomic_hash(key);
        let mut dictionary = dictionary.lock().unwrap();
        match dictionary.get_mut(key) {
            Some((StorageValue::List(list), last_access)) => {
                *last_access = SystemTime::now();
                Ok(SuccessQuery::Integer(list.len() as i32))
            }
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(SuccessQuery::Integer(0)),
        }
    }

    /// Removes and returns the first elements of the list stored at key.
    /// By default, the command pops a single element from the beginning of the list.
    /// When provided with the optional count argument, the reply will consist of up to count elements, depending on the list's length.
    ///
    /// Reply: SuccessQuery::String(s) when s is the the value of the first element, or SuccessQuery::Nil when key does not exist.
    /// # Examples
    /// ```
    /// let mut database = Database::new("dump_path.txt");
    ///
    /// let database = database.rpush("KEY", "VALUE_A").unwrap();
    /// let database = database.rpush("KEY", "VALUE_B").unwrap();
    ///
    /// if let SuccessQuery::String(val) = database.lpop("KEY").unwrap() {
    ///     assert_eq!(val.to_string(), "VALUE_A".to_string());
    /// }
    ///
    /// let result = database.llen("KEY").unwrap();
    /// assert_eq!(result, SuccessQuery::Integer(1));
    /// ```
    pub fn lpop(&mut self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        if !self._exists(key) {
            return Ok(SuccessQuery::Nil);
        }

        let dictionary = self.dictionary.get_atomic_hash(key);
        let mut dictionary = dictionary.lock().unwrap();
        match dictionary.get_mut(key) {
            Some((StorageValue::List(list), last_access)) => {
                *last_access = SystemTime::now();
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

    #[doc(hidden)]
    fn lpush_one(&mut self, key: &str, value: &str) -> Result<SuccessQuery, DataBaseError> {
        if !self._exists(key) {
            let list: Vec<String> = vec![value.to_owned()];
            let len = list.len();
            self.dictionary
                .insert(key.to_owned(), StorageValue::List(list));
            return Ok(SuccessQuery::Integer(len as i32));
        }

        self.dictionary.touch(key);
        let dictionary = self.dictionary.get_atomic_hash(key);
        let mut dictionary = dictionary.lock().unwrap();
        match dictionary.get_mut(key) {
            Some((StorageValue::List(list), _)) => {
                list.insert(0, value.to_owned());
                Ok(SuccessQuery::Integer(list.len() as i32))
            }
            _ => Err(DataBaseError::NotAList),
        }
    }

    /// Insert all the specified values at the head of the list stored at key.
    /// If key does not exist, it is created as empty list before performing the push operations.
    /// When key holds a value that is not a list, an error is returned.
    ///
    /// It is possible to push multiple elements using a single command call just specifying multiple arguments at the end of the command.
    /// Elements are inserted one after the other to the head of the list, from the leftmost element to the rightmost element.
    /// So for instance the command LPUSH mylist a b c will result into a list containing c as first element, b as second element and a as third element.
    ///
    /// Reply: SuccessQuery::Integer(n) when n is the the length of the list after the push operations.
    ///
    /// # Examples
    /// ```
    /// let mut database = Database::new("dump_path.txt");
    /// database.lpush("KEY", ["VALUEA"].to_vec()).unwrap();
    ///
    /// let result = database.lpush("KEY", ["VALUEB"].to_vec()).unwrap();
    ///
    /// assert_eq!(result, SuccessQuery::Integer(2));
    ///
    /// let dictionary = database.dictionary;
    /// let dictionary = dictionary.get_atomic_hash("KEY");
    /// let dictionary = dictionary.lock().unwrap();
    ///
    /// if let StorageValue::List(list) = dictionary.get("KEY").unwrap() {
    ///     assert_eq!(list.len(), 2);
    ///     assert_eq!(list[1], "VALUEA");
    ///     assert_eq!(list[0], "VALUEB");
    /// }
    /// ```
    pub fn lpush(&mut self, key: &str, values: Vec<&str>) -> Result<SuccessQuery, DataBaseError> {
        let mut result = self.lpush_one(key, values[0]);
        for item in values.iter().skip(1) {
            result = self.lpush_one(key, item)
        }

        result
    }

    /// Inserts specified value/s at the head of the list stored at key, only if key already exists and holds a list.
    /// In contrary to LPUSH, no operation will be performed when key does not yet exist.
    ///
    /// Reply: SuccessQuery::Integer(n) when n is the the length of the list after the push operation.
    ///
    /// n=0 if key not hold a list and not push data.
    ///
    /// # Examples
    /// ```
    /// let mut database = Database::new("dump_path.txt");
    /// database.lpushx("KEY", ["VALUEA"].to_vec()).unwrap();
    ///
    /// assert_eq!(result, SuccessQuery::Integer(0));
    /// ```
    /// ```
    /// let mut database = Database::new("dump_path.txt");
    /// database.lpush("KEY", ["VALUEA"].to_vec()).unwrap();
    /// database.lpushx("KEY", ["VALUEB"].to_vec()).unwrap();
    /// let dictionary = database.dictionary;
    /// let dictionary = dictionary.get_atomic_hash("KEY");
    /// let dictionary = dictionary.lock().unwrap();
    ///
    /// if let StorageValue::List(list) = dictionary.get("KEY").unwrap() {
    ///     assert_eq!(list.len(), 2);
    ///     assert_eq!(list[1], "VALUEA");
    ///     assert_eq!(list[0], "VALUEB");
    /// }
    /// ```
    pub fn lpushx(&mut self, key: &str, values: Vec<&str>) -> Result<SuccessQuery, DataBaseError> {
        if !self._exists(key) {
            return Ok(SuccessQuery::Integer(0));
        }

        let dictionary = self.dictionary.get_atomic_hash(key);
        let mut dictionary = dictionary.lock().unwrap();
        match dictionary.get_mut(key) {
            Some((StorageValue::List(list), last_access)) => {
                *last_access = SystemTime::now();
                values.iter().for_each(|&val| {
                    list.insert(0, val.to_owned());
                });
                Ok(SuccessQuery::Integer(list.len() as i32))
            }
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(SuccessQuery::Integer(0)),
        }
    }

    /// Returns the specified elements of the list stored at key.
    /// The offsets start and stop are zero-based indexes, with 0 being the first element of the list (the head of the list),
    /// 1 being the next element and so on.
    ///
    /// These offsets can also be negative numbers indicating offsets starting at the end of the list.
    /// For example, -1 is the last element of the list, -2 the penultimate, and so on.
    ///
    /// Reply: SuccessQuery::List(list) when list is the list of elements in the specified range.
    ///
    /// # Examples
    /// ```
    /// let mut database = Database::new("dump_path.txt");
    /// database.lpush("KEY", ["VALUEA","VALUEB", "VALUEC","VALUED"].to_vec()).unwrap();
    ///
    /// if let SuccessQuery::List(list) = database.lrange("KEY", 0, 2).unwrap() {
    ///     let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();
    ///     let second_list: Vec<&str> = vec!["VALUED", "VALUEC", "VALUEB"];
    ///     let pair_list: Vec<(&String, &str)> = list.iter().zip(second_list).collect();
    ///     pair_list.iter().for_each(|x| {
    ///         assert_eq!(x.0, x.1);
    ///     })
    /// }
    /// ```
    /// ```
    /// let mut database = Database::new("dump_path.txt");
    /// database.lpush("KEY", ["VALUEA","VALUEB", "VALUEC","VALUED"].to_vec()).unwrap();
    /// if let SuccessQuery::List(list) = database.lrange("KEY", 0, -1).unwrap() {
    ///     let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();
    ///     let second_list: Vec<&str> = vec!["VALUED", "VALUEC", "VALUEB", "VALUEA"];
    ///     let pair_list: Vec<(&String, &str)> = list.iter().zip(second_list).collect();
    ///     pair_list.iter().for_each(|x| {
    ///         assert_eq!(x.0, x.1);
    ///     })
    /// }
    /// ```
    pub fn lrange(&mut self, key: &str, ini: i32, end: i32) -> Result<SuccessQuery, DataBaseError> {
        let mut sub_list: Vec<SuccessQuery> = Vec::new();
        if !self._exists(key) {
            return Ok(SuccessQuery::List(sub_list));
        }

        let dictionary = self.dictionary.get_atomic_hash(key);
        let mut dictionary = dictionary.lock().unwrap();

        match dictionary.get_mut(key) {
            Some((StorageValue::List(list), last_access)) => {
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
                }

                *last_access = SystemTime::now();
                Ok(SuccessQuery::List(sub_list))
            }
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(SuccessQuery::List(sub_list)),
        }
    }

    /// Removes the first count occurrences of elements equal to element from the list stored at key.
    /// The count argument influences the operation in the following ways:
    ///
    /// count > 0: Remove elements equal to element moving from head to tail.
    ///
    /// count < 0: Remove elements equal to element moving from tail to head.
    ///
    /// count = 0: Remove all elements equal to element.
    ///
    /// For example, LREM list -2 "hello" will remove the last two occurrences of "hello" in the list stored at list.
    ///
    /// Note that non-existing keys are treated like empty lists, so when key does not exist, the command will always return 0.
    ///
    /// Reply: SuccessQuery::Integer(n) when n is the number of removed elements.
    ///
    /// # Examples
    /// ```
    /// let mut database = Database::new("dump_path.txt");
    /// database.lpush("KEY", ["VALUEA", "VALUEA", "VALUEC", "VALUEA"].to_vec()).unwrap();
    ///
    /// let result = database.lrem("KEY", 2, "VALUEA");
    ///
    /// assert_eq!(SuccessQuery::Integer(2), result.unwrap());
    ///
    /// let dictionary = database.dictionary;
    /// let dictionary = dictionary.get_atomic_hash("KEY");
    /// let dictionary = dictionary.lock().unwrap();
    ///
    /// if let Some(StorageValue::List(list)) = dictionary.get("KEY") {
    ///     assert_eq!(list[0], "VALUEC");
    ///     assert_eq!(list[1], "VALUEA");
    /// }
    /// ```
    pub fn lrem(
        &mut self,
        key: &str,
        mut count: i32,
        elem: &str,
    ) -> Result<SuccessQuery, DataBaseError> {
        if !self._exists(key) {
            return Ok(SuccessQuery::Integer(0));
        }

        let dictionary = self.dictionary.get_atomic_hash(key);
        let mut dictionary = dictionary.lock().unwrap();
        match dictionary.get_mut(key) {
            Some((StorageValue::List(list), last_access)) => {
                *last_access = SystemTime::now();
                match count.cmp(&0) {
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
                }
            }
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(SuccessQuery::Integer(0)),
        }
    }

    /// Sets the list element at index to element. For more information on the index argument, see lindex.
    ///
    /// An error is returned for out of range indexes.
    ///
    /// Reply: SuccessQuery::Success if set the list element correctly.
    ///
    /// # Examples
    /// ```
    /// let mut database = Database::new("dump_path.txt");
    /// database.lpush(KEY, ["VALUEA", "VALUEB", "VALUEC", "VALUED"].to_vec()).unwrap();
    ///
    /// let result = database.lset("KEY", 0, "VALUEA");
    /// assert_eq!(SuccessQuery::Success, result.unwrap());
    ///
    /// let dictionary = database.dictionary;
    /// let dictionary = dictionary.get_atomic_hash("KEY");
    /// let dictionary = dictionary.lock().unwrap();
    ///
    /// if let StorageValue::List(list) = dictionary.get("KEY").unwrap() {
    ///     assert_eq!(list[0], "VALUEA");
    /// }
    /// ```
    pub fn lset(
        &mut self,
        key: &str,
        index: usize,
        value: &str,
    ) -> Result<SuccessQuery, DataBaseError> {
        if !self._exists(key) {
            return Err(DataBaseError::NonExistentKey);
        }

        let dictionary = self.dictionary.get_atomic_hash(key);
        let mut dictionary = dictionary.lock().unwrap();
        match dictionary.get_mut(key) {
            Some((StorageValue::List(list), last_access)) => {
                *last_access = SystemTime::now();
                match list.get_mut(index) {
                    Some(val) => {
                        val.clear();
                        val.push_str(value);
                        Ok(SuccessQuery::Success)
                    }
                    None => Err(DataBaseError::IndexOutOfRange),
                }
            }
            Some(_) => Err(DataBaseError::NotAList),
            None => Err(DataBaseError::NonExistentKey),
        }
    }

    /// Removes and returns the last elements of the list stored at key.
    /// By default, the command pops a single element from the end of the list.
    ///
    /// Reply: SuccessQuery::String(val) when val is the value of the last element, or SuccessQuery::Nil when key does not exist.
    ///
    /// # Examples
    /// ```
    /// todo
    /// ```
    pub fn rpop(&mut self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        if !self._exists(key) {
            return Ok(SuccessQuery::Nil);
        }

        let dictionary = self.dictionary.get_atomic_hash(key);
        let mut dictionary = dictionary.lock().unwrap();
        match dictionary.get_mut(key) {
            Some((StorageValue::List(list), last_access)) => {
                *last_access = SystemTime::now();
                match list.pop() {
                    Some(value) => Ok(SuccessQuery::String(value)),
                    None => Ok(SuccessQuery::Nil),
                }
            }
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(SuccessQuery::Nil),
        }
    }

    /// Insert all the specified values at the tail of the list stored at key.
    /// If key does not exist, it is created as empty list before performing the push operation.
    /// When key holds a value that is not a list, an error is returned.
    ///
    /// It is possible to push multiple elements using a single command call just specifying multiple arguments at the end of the command.
    /// Elements are inserted one after the other to the tail of the list, from the leftmost element to the rightmost element.
    ///
    /// So for instance the command RPUSH mylist a b c will result into a list containing a as first element, b as second element and c as third element.
    ///
    /// Reply: the length of the list after the push operation.
    ///
    /// # Examples
    /// ```
    /// todo
    /// ```
    pub fn rpush(&mut self, key: &str, values: Vec<&str>) -> Result<SuccessQuery, DataBaseError> {
        if !self._exists(key) {
            let mut list: Vec<String> = Vec::new();
            values.iter().for_each(|&val| {
                list.push(val.to_owned());
            });

            let len = list.len();
            self.dictionary
                .insert(key.to_owned(), StorageValue::List(list));
            return Ok(SuccessQuery::Integer(len as i32));
        }

        let dictionary = self.dictionary.get_atomic_hash(key);
        let mut dictionary = dictionary.lock().unwrap();
        match dictionary.get_mut(key) {
            Some((StorageValue::List(list), last_access)) => {
                *last_access = SystemTime::now();
                values.iter().for_each(|&val| {
                    list.push(val.to_owned());
                });
                Ok(SuccessQuery::Integer(list.len() as i32))
            }
            _ => Err(DataBaseError::NotAList),
        }
    }

    /// Inserts specified values at the tail of the list stored at key, only if key already exists and holds a list.
    /// In contrary to RPUSH, no operation will be performed when key does not yet exist.
    ///
    /// Reply: SuccessQuery:Integer(n) when n is the length of the list after the push operation.
    ///
    /// # Examples
    /// ```
    /// todo
    /// ```
    pub fn rpushx(&mut self, key: &str, values: Vec<&str>) -> Result<SuccessQuery, DataBaseError> {
        if !self._exists(key) {
            return Ok(SuccessQuery::Boolean(false));
        }
        let dictionary = self.dictionary.get_atomic_hash(key);
        let mut dictionary = dictionary.lock().unwrap();
        match dictionary.get_mut(key) {
            Some((StorageValue::List(list), last_access)) => {
                *last_access = SystemTime::now();
                values.iter().for_each(|&val| {
                    list.push(val.to_owned());
                });
                Ok(SuccessQuery::Integer(list.len() as i32))
            }
            Some(_) => Err(DataBaseError::NotAList),
            None => Ok(SuccessQuery::Boolean(false)),
        }
    }

    //SETS

    /// Returns if member is a member of the set stored at key.
    ///
    /// Reply: SuccessQuery::Boolean(true) if the element is a member of the set.
    ///
    /// SuccessQuery::Boolean(false) if the element is not a member of the set, or if key does not exist.
    ///
    /// Error if key of database exists but not hold a Set.
    /// # Example
    /// ```
    /// let mut database = Database::new("dump_path.txt");
    /// let result = database.sadd("key", ["element"].to_vec()).unwrap();
    /// assert_eq!(result, SuccessQuery::Integer(1));
    /// let is_member = database.sismember("key", "element").unwrap();
    /// assert_eq!(is_member, SuccessQuery::Boolean(true));
    /// ```
    pub fn sismember(&mut self, key: &str, value: &str) -> Result<SuccessQuery, DataBaseError> {
        if !self._exists(key) {
            return Ok(SuccessQuery::Boolean(false));
        }

        let dictionary = self.dictionary.get_atomic_hash(key);
        let mut dictionary = dictionary.lock().unwrap();
        match dictionary.get_mut(key) {
            Some((StorageValue::Set(hash_set), last_access)) => {
                *last_access = SystemTime::now();
                match hash_set.get(value) {
                    Some(_val) => Ok(SuccessQuery::Boolean(true)),
                    None => Ok(SuccessQuery::Boolean(false)),
                }
            }
            Some(_) => Err(DataBaseError::NotASet),
            None => Ok(SuccessQuery::Boolean(false)),
        }
    }

    /// Returns the set cardinality (number of elements) of the set stored at key.
    ///
    /// Reply: SuccessQuery::Integer(n) when n is the cardinality (number of elements) of the set,
    /// or SuccessQuery::Boolean(false) if key does not exist.
    ///
    /// Error if key of database exists but not hold a Set.
    /// # Example
    /// ```
    /// let mut database = Database::new("dump_path.txt");
    /// let elements = vec!["0", "1", "2", "3"];
    ///
    /// let _ = database.sadd("key", elements);
    /// let len_set = database.scard("key").unwrap();
    ///
    /// assert_eq!(len_set, SuccessQuery::Integer(4));
    /// ```
    pub fn scard(&mut self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        if !self._exists(key) {
            return Ok(SuccessQuery::Boolean(false));
        }

        let dictionary = self.dictionary.get_atomic_hash(key);
        let mut dictionary = dictionary.lock().unwrap();
        match dictionary.get_mut(key) {
            Some((StorageValue::Set(hash_set), last_access)) => {
                *last_access = SystemTime::now();
                Ok(SuccessQuery::Integer(hash_set.len() as i32))
            }
            Some(_) => Err(DataBaseError::NotASet),
            None => Ok(SuccessQuery::Boolean(false)),
        }
    }

    #[doc(hidden)]
    pub fn sadd_one(&mut self, key: &str, value: &str) -> Result<SuccessQuery, DataBaseError> {
        if !self._exists(key) {
            let mut set: HashSet<String> = HashSet::new();
            set.insert(value.to_owned());
            self.dictionary
                .insert(key.to_owned(), StorageValue::Set(set));
            return Ok(SuccessQuery::Integer(1));
        }

        let dictionary = self.dictionary.get_atomic_hash(key);
        let mut dictionary = dictionary.lock().unwrap();
        match dictionary.get_mut(key) {
            Some((StorageValue::Set(hash_set), last_access)) => {
                *last_access = SystemTime::now();
                if hash_set.contains(value) {
                    Ok(SuccessQuery::Integer(0))
                } else {
                    hash_set.insert(value.to_owned());
                    Ok(SuccessQuery::Integer(1))
                }
            }
            _ => Err(DataBaseError::NotASet),
        }
    }

    /// Add the specified members to the set stored at key. Specified members that are already a member of this set are ignored.
    /// If key does not exist, a new set is created before adding the specified members.
    /// An error is returned when the value stored at key is not a set.
    ///
    /// Reply: SuccessQuery::Integer(n) when n is the number of elements that were added to the set,
    /// not including all the elements already present in the set.
    ///
    /// # Example
    /// ```
    /// let mut database = Database::new("dump_path.txt");
    /// database.sadd("KEY", ["ELEMENT"].to_vec()).unwrap();
    /// let result = database.sadd("KEY", ["ELEMENT", "ELEMENT_2", "ELEMENT_3"].to_vec()).unwrap();
    ///
    /// assert_eq!(result, SuccessQuery::Integer(2));
    /// let len_set = database.scard("KEY").unwrap();
    /// assert_eq!(len_set, SuccessQuery::Integer(3));
    /// ```
    pub fn sadd(&mut self, key: &str, values: Vec<&str>) -> Result<SuccessQuery, DataBaseError> {
        let mut elems_added = 0;
        let mut result = self.sadd_one(key, values[0]);
        if let Ok(SuccessQuery::Integer(val)) = result {
            elems_added += val;
        }
        for item in values.iter().skip(1) {
            result = self.sadd_one(key, item);
            if let Ok(SuccessQuery::Integer(val)) = result {
                elems_added += val;
            }
        }
        match result {
            Ok(SuccessQuery::Integer(_val)) => Ok(SuccessQuery::Integer(elems_added)),
            Err(err) => Err(err),
            _ => Err(DataBaseError::NotASet),
        }
    }

    /// Returns all the members of the set value stored at key.
    ///
    /// Reply: SuccessQuery::List(list) when list are all elements of the set.
    ///
    /// # Example
    /// ```
    /// let mut database = Database::new("dump_path.txt");
    /// database.sadd("KEY", ["ELEMENT"].to_vec()).unwrap();
    /// database.sadd("KEY", ["OTHER_ELEMENT"].to_vec()).unwrap();
    ///
    /// if let SuccessQuery::List(list) = database.smembers("KEY").unwrap() {
    ///     for elem in list {
    ///         let is_member = database.sismember("KEY", &elem.to_string()).unwrap();
    ///         assert_eq!(is_member, SuccessQuery::Boolean(true));
    ///     }
    /// }
    /// ```
    pub fn smembers(&mut self, key: &str) -> Result<SuccessQuery, DataBaseError> {
        let mut result: Vec<SuccessQuery> = Vec::new();
        if !self._exists(key) {
            return Ok(SuccessQuery::List(result));
        }

        let dictionary = self.dictionary.get_atomic_hash(key);
        let mut dictionary = dictionary.lock().unwrap();
        match dictionary.get_mut(key) {
            Some((StorageValue::Set(hash_set), last_access)) => {
                *last_access = SystemTime::now();
                for elem in hash_set.iter() {
                    result.push(SuccessQuery::String(elem.clone()));
                }
                Ok(SuccessQuery::List(result))
            }
            Some(_) => Err(DataBaseError::NotASet),
            None => Ok(SuccessQuery::List(result)),
        }
    }

    /// Remove the specified members from the set stored at key.
    /// Specified members that are not a member of this set are ignored.
    ///
    /// If key does not exist, it is treated as an empty set and this command returns 0.
    /// An error is returned when the value stored at key is not a set.
    ///
    /// Reply: SuccessQuery::Integer(n) when n is the number of members that were removed from the set, not including non existing members.
    ///
    /// # Example
    /// ```
    /// let mut database = Database::new("dump_path.txt");
    /// let members = vec!["ELEMENT"];
    ///
    /// database.sadd("KEY", ["ELEMENT"].to_vec()).unwrap();
    /// let result = database.srem("KEY", members).unwrap();
    /// let is_member = database.sismember("KEY", "ELEMENT").unwrap();
    ///
    /// assert_eq!(result, SuccessQuery::Integer(1));
    /// assert_eq!(is_member, SuccessQuery::Boolean(false));
    /// ```
    pub fn srem(
        &mut self,
        key: &str,
        members_to_rmv: Vec<&str>,
    ) -> Result<SuccessQuery, DataBaseError> {
        if !self._exists(key) {
            return Ok(SuccessQuery::Boolean(false));
        }

        let dictionary = self.dictionary.get_atomic_hash(key);
        let mut dictionary = dictionary.lock().unwrap();
        match dictionary.get_mut(key) {
            Some((StorageValue::Set(hash_set), last_access)) => {
                *last_access = SystemTime::now();
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

impl<'a> Clone for Database {
    fn clone(&self) -> Self {
        Database::new_from_db(
            self.ttl_msg_sender.clone(),
            self.dictionary.clone(),
            self.db_dump_path.clone(),
        )
    }
}

#[doc(hidden)]
fn executor(mut dictionary: HashShard, ttl_vector: TtlVector) {
    loop {
        thread::sleep(Duration::new(30, 0));
        let keys_ttl = ttl_vector.clone();
        let mut keys_locked = keys_ttl.lock().unwrap();

        while let Some(ttl) = keys_locked.get(0) {
            if ttl.expire_time < SystemTime::now() {
                let ttl_key = keys_locked.remove(0);
                dictionary.remove(&ttl_key.key);
            } else {
                break;
            }
        }

        if keys_locked.is_empty() {
            break;
        }
    }
}

impl fmt::Display for Database {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        for (key, value) in self.dictionary.key_value() {
            writeln!(f, "key: {}, value: {}", key, value)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod ttl_commands {
    use super::*;
    use std::time::Duration;

    const KEY_A: &str = "KEY_A";
    const VALUE_A: &str = "VALUE_A";

    const KEY_B: &str = "KEY_B";
    const VALUE_B: &str = "VALUE_B";

    const KEY_C: &str = "KEY_C";
    const VALUE_C: &str = "VALUE_C";

    const KEY_D: &str = "KEY_D";
    const VALUE_D: &str = "VALUE_D";

    const DB_DUMP: &str = "db_dump_path";

    // duration_since

    #[test]
    fn ttl_supervisor_run_supervaise_a_key() {
        let mut db = Database::new(DB_DUMP.to_string());

        db.append(KEY_A, VALUE_A).unwrap();

        let now = SystemTime::now();
        let expire_time_a = now.checked_add(Duration::new(1, 0)).unwrap();
        let ttl_pair = KeyTtl::new(KEY_A, expire_time_a);

        if let SuccessQuery::Boolean(value) = db.exists(KEY_A).unwrap() {
            assert_eq!(value, true);
        }

        db.ttl_msg_sender
            .send(MessageTtl::Expire(ttl_pair))
            .unwrap();

        thread::sleep(Duration::new(2, 0));

        if let SuccessQuery::Boolean(value) = db.exists(KEY_A).unwrap() {
            assert_eq!(value, false);
        }
    }

    #[test]
    fn ttl_supervisor_run_supervaise_two_key() {
        let mut db = Database::new(DB_DUMP.to_string());

        db.append(KEY_A, VALUE_A).unwrap();
        db.append(KEY_B, VALUE_B).unwrap();

        let now = SystemTime::now();
        let expire_time_a = now.checked_add(Duration::new(1, 0)).unwrap();
        let expire_time_b = now.checked_add(Duration::new(5, 0)).unwrap();

        let ttl_pair_a = KeyTtl::new(KEY_A, expire_time_a);
        let ttl_pair_b = KeyTtl::new(KEY_B, expire_time_b);

        if let SuccessQuery::Boolean(value) = db.exists(KEY_A).unwrap() {
            assert_eq!(value, true);
        }

        if let SuccessQuery::Boolean(value) = db.exists(KEY_B).unwrap() {
            assert_eq!(value, true);
        }

        db.ttl_msg_sender
            .send(MessageTtl::Expire(ttl_pair_a))
            .unwrap();
        db.ttl_msg_sender
            .send(MessageTtl::Expire(ttl_pair_b))
            .unwrap();

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
        let mut db = Database::new(DB_DUMP.to_string());

        db.append(KEY_A, VALUE_A).unwrap();
        db.append(KEY_B, VALUE_B).unwrap();
        db.append(KEY_C, VALUE_C).unwrap();
        db.append(KEY_D, VALUE_D).unwrap();

        let now = SystemTime::now();
        let expire_time_a = now.checked_add(Duration::from_secs(SEC)).unwrap();
        let expire_time_b = now.checked_add(Duration::from_secs(SEC * 3)).unwrap();
        let expire_time_c = now.checked_add(Duration::from_secs(SEC * 5)).unwrap();
        let expire_time_d = now.checked_add(Duration::from_secs(SEC * 10)).unwrap();

        let ttl_pair_a = KeyTtl::new(KEY_A, expire_time_a);
        let ttl_pair_b = KeyTtl::new(KEY_B, expire_time_b);
        let ttl_pair_c = KeyTtl::new(KEY_C, expire_time_c);
        let ttl_pair_d = KeyTtl::new(KEY_D, expire_time_d);

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

        db.ttl_msg_sender
            .send(MessageTtl::Expire(ttl_pair_a))
            .unwrap();
        db.ttl_msg_sender
            .send(MessageTtl::Expire(ttl_pair_b))
            .unwrap();
        db.ttl_msg_sender
            .send(MessageTtl::Expire(ttl_pair_c))
            .unwrap();
        db.ttl_msg_sender
            .send(MessageTtl::Expire(ttl_pair_d))
            .unwrap();

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

        if let SuccessQuery::Boolean(value) = db.exists(KEY_D).unwrap() {
            assert_eq!(value, true);
        }

        thread::sleep(Duration::from_secs(SEC * 2));

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

        thread::sleep(Duration::from_secs(SEC + 4));

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

    #[test]
    fn ttl_supervisor_run_supervaise_four_keys_one_of_the_key_is_inserted_with_a_lower_expire_time_the_actual_key(
    ) {
        let mut db = Database::new(DB_DUMP.to_string());

        db.append(KEY_A, VALUE_A).unwrap();
        db.append(KEY_B, VALUE_B).unwrap();
        db.append(KEY_C, VALUE_C).unwrap();
        db.append(KEY_D, VALUE_D).unwrap();

        let now = SystemTime::now();
        let expire_time_a = now.checked_add(Duration::from_secs(SEC)).unwrap();
        let expire_time_b = now.checked_add(Duration::from_secs(SEC * 3)).unwrap();
        let expire_time_c = now.checked_add(Duration::from_secs(SEC * 5)).unwrap();
        let expire_time_d = now.checked_add(Duration::from_secs(SEC * 10)).unwrap();

        let ttl_pair_a = KeyTtl::new(KEY_A, expire_time_a);
        let ttl_pair_b = KeyTtl::new(KEY_B, expire_time_b);
        let ttl_pair_c = KeyTtl::new(KEY_C, expire_time_c);
        let ttl_pair_d = KeyTtl::new(KEY_D, expire_time_d);

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

        db.ttl_msg_sender
            .send(MessageTtl::Expire(ttl_pair_b))
            .unwrap();
        db.ttl_msg_sender
            .send(MessageTtl::Expire(ttl_pair_c))
            .unwrap();
        db.ttl_msg_sender
            .send(MessageTtl::Expire(ttl_pair_a))
            .unwrap();
        db.ttl_msg_sender
            .send(MessageTtl::Expire(ttl_pair_d))
            .unwrap();

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

        if let SuccessQuery::Boolean(value) = db.exists(KEY_C).unwrap() {
            assert_eq!(value, true);
        }

        thread::sleep(Duration::from_secs(SEC * 2));
        if let SuccessQuery::Boolean(value) = db.exists(KEY_A).unwrap() {
            assert_eq!(value, false);
        }

        if let SuccessQuery::Boolean(value) = db.exists(KEY_B).unwrap() {
            assert_eq!(value, false);
        }

        if let SuccessQuery::Boolean(value) = db.exists(KEY_C).unwrap() {
            assert_eq!(value, true);
        }

        if let SuccessQuery::Boolean(value) = db.exists(KEY_D).unwrap() {
            assert_eq!(value, true);
        }

        thread::sleep(Duration::from_secs(SEC));

        if let SuccessQuery::Boolean(value) = db.exists(KEY_A).unwrap() {
            assert_eq!(value, false);
        }

        if let SuccessQuery::Boolean(value) = db.exists(KEY_B).unwrap() {
            assert_eq!(value, false);
        }

        if let SuccessQuery::Boolean(value) = db.exists(KEY_C).unwrap() {
            assert_eq!(value, false);
        }

        if let SuccessQuery::Boolean(value) = db.exists(KEY_D).unwrap() {
            assert_eq!(value, true);
        }
        thread::sleep(Duration::from_secs(SEC + 4));

        if let SuccessQuery::Boolean(value) = db.exists(KEY_A).unwrap() {
            assert_eq!(value, false);
        }

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

#[cfg(test)]
mod group_string {

    use super::*;

    const KEY: &str = "KEY";
    const VALUE: &str = "VALUE";

    const DB_DUMP: &str = "db_dump_path.txt";

    fn create_database_with_string() -> Database {
        let mut db = Database::new(DB_DUMP.to_string());
        db.set(KEY, VALUE).unwrap();
        db
    }

    fn create_database() -> Database {
        let db = Database::new(DB_DUMP.to_string());
        db
    }

    mod append_test {

        use super::*;

        #[test]
        fn test_append_new_key_return_lenght_of_the_value() {
            let mut database = create_database();

            if let SuccessQuery::Integer(lenght) = database.append(KEY, VALUE).unwrap() {
                assert_eq!(lenght, 5);
            }
        }

        #[test]
        fn test_append_key_with_old_value_return_lenght_of_the_total_value() {
            let mut database = create_database();

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
            let mut database = create_database();

            let result = database.get(KEY).unwrap();

            assert_eq!(result, SuccessQuery::Nil);
        }
    }

    mod set_test {
        use super::*;

        #[test]
        fn test_set_returns_ok() {
            let mut database = create_database();

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

            let result = database.get(KEY).unwrap();
            assert_eq!(result, SuccessQuery::Nil);
        }

        #[test]
        fn test_getdel_returns_nil_value_if_key_not_exist_in_database() {
            let mut database = create_database();

            let result = database.getdel(KEY).unwrap();
            assert_eq!(result, SuccessQuery::Nil);
        }
    }

    mod incrby_test {
        use super::*;

        #[test]
        fn test_incrby_returns_lenght_of_the_resulting_value_after_increment() {
            let mut database = create_database();

            database.set(KEY, "1").unwrap();

            let result = database.incrby(KEY, 4).unwrap();

            assert_eq!(result, SuccessQuery::Integer(5));
        }

        #[test]
        fn test_incrby_returns_error_if_the_value_of_key_not_hold_parseable_value_to_number() {
            let mut database = create_database();

            database.set(KEY, "1a").unwrap();

            let result = database.incrby(KEY, 4).unwrap_err();

            assert_eq!(result, DataBaseError::NotAnInteger);
        }
    }

    mod decrby_test {
        use super::*;

        #[test]
        fn test_decrby_returns_lenght_of_the_resulting_value_after_increment() {
            let mut database = create_database();

            database.set(KEY, "5").unwrap();
            let result = database.decrby(KEY, 4).unwrap();

            assert_eq!(result, SuccessQuery::Integer(1));
        }

        #[test]
        fn test_decrby_returns_error_if_the_value_of_key_not_hold_parseable_value_to_number() {
            let mut database = create_database();

            database.set(KEY, "5a").unwrap();
            let result = database.decrby(KEY, 4).unwrap_err();

            assert_eq!(result, DataBaseError::NotAnInteger);
        }
    }

    mod strlen_test {
        use super::*;

        #[test]
        fn test_strlen_returns_the_lenght_of_value_key() {
            let mut database = create_database();

            database.set(KEY, VALUE).unwrap();

            if let SuccessQuery::Integer(value) = database.strlen(KEY).unwrap() {
                assert_eq!(value, 5);
            }
        }

        #[test]
        fn test_strlen_returns_zero_if_the_key_not_exists_in_database() {
            let mut database = create_database();

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
            let mut database = create_database();

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
            let mut database = create_database();

            database.set(KEY_A, VALUE_A).unwrap();
            database.set(KEY_B, VALUE_B).unwrap();
            database.set(KEY_C, VALUE_C).unwrap();
            database.set(KEY_D, VALUE_D).unwrap();

            database
        }

        #[test]
        fn test_mget_return_all_values_of_keys_if_all_keys_are_in_database() {
            let mut database = create_a_database_with_key_values();

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
            let mut database = create_database();

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

    const DB_DUMP: &str = "db_dump_path.txt.txt";

    fn create_database() -> Database {
        let db = Database::new(DB_DUMP.to_string());
        db
    }

    const KEY: &str = "KEY";
    const SECOND_KEY: &str = "SECOND_KEY";

    const VALUE: &str = "VALUE";
    const SECOND_VALUE: &str = "SECOND_VALUE";

    mod copy_test {
        use super::*;

        #[test]
        fn test_copy_set_dolly_sheep_then_copy_to_clone() {
            let mut database = create_database();

            database.set(KEY, VALUE).unwrap();
            let result = database.copy(KEY, SECOND_KEY);
            assert_eq!(result.unwrap(), SuccessQuery::Success);
            if let SuccessQuery::String(value) = database.strlen(SECOND_KEY).unwrap() {
                assert_eq!(value.to_string(), VALUE);
            }
        }

        #[test]
        fn test_copy_set_dolly_sheep_then_copy_to_clone_when_clone_exist() {
            let mut database = create_database();

            database.set(KEY, VALUE).unwrap();
            database.set(SECOND_KEY, SECOND_VALUE).unwrap();
            let result = database.copy(KEY, SECOND_KEY);

            assert_eq!(result.unwrap_err(), DataBaseError::KeyAlredyExist);
        }

        #[test]
        fn test_copy_try_to_copy_a_key_does_not_exist() {
            let mut database = create_database();
            let result = database.copy(KEY, SECOND_KEY);

            assert_eq!(result.unwrap_err(), DataBaseError::NonExistentKey);
        }
    }

    mod del_test {
        use super::*;

        #[test]
        fn test_del_key_value_returns_succes() {
            let mut database = create_database();
            database.set(KEY, VALUE).unwrap();

            let result = database.del(KEY);
            assert_eq!(result.unwrap(), SuccessQuery::Success);
            assert_eq!(database.get(KEY).unwrap(), SuccessQuery::Nil);
        }
        #[test]
        fn test_del_key_non_exist_returns_nil() {
            let mut database = create_database();

            let result = database.del(KEY);
            assert_eq!(result.unwrap(), SuccessQuery::Success);
        }
    }

    mod exists_test {

        use super::*;

        #[test]
        fn test_exists_key_non_exist_returns_bool_false() {
            let mut database = create_database();

            let result = database.exists(KEY);
            assert_eq!(result.unwrap(), SuccessQuery::Boolean(false));
            assert_eq!(database.get(KEY).unwrap(), SuccessQuery::Nil);
        }

        #[test]
        fn test_exists_key_hello_returns_bool_true() {
            let mut database = create_database();
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
            let mut database = create_database();
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
            let mut database = create_database();

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
            let mut database = create_database();
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
            let mut database = create_database();
            database.set(KEY, VALUE).unwrap();

            let result = database.rename(KEY, SECOND_KEY).unwrap();
            assert_eq!(result, SuccessQuery::Success);

            let result = database.get(KEY).unwrap();
            assert_eq!(result, SuccessQuery::Nil);
        }

        #[test]
        fn test_rename_key_non_exists_error() {
            let mut database = create_database();

            let result = database.rename(KEY, SECOND_KEY).unwrap_err();

            assert_eq!(result, DataBaseError::NonExistentKey);
        }
    }

    mod sort_test {
        use super::*;
        const LIST: &str = "list";
        const SET: &str = "set";
        const VALUE_1: &str = "1";
        const VALUE_2: &str = "2";
        const VALUE_3: &str = "3";
        const VALUE_A: &str = "a";

        const LIMIT_OFFSET_OFF: i32 = 0;
        const LIMIT_COUNT_OFF: i32 = -1;
        const LIMIT_COUNT_ZERO: i32 = 0;

        const PATTERN: &str = "weight_*";
        const UNMATCH_PATTERN: &str = "no_exist";

        const KEY_WEIGHT_1: &str = "weight_1";
        const KEY_WEIGHT_2: &str = "weight_2";
        const KEY_WEIGHT_3: &str = "weight_3";

        const VAL_WEIGHT_1: &str = "10";
        const VAL_WEIGHT_2: &str = "30";
        const VAL_WEIGHT_3: &str = "20";
        const VAL_WEIGHT_2_NOT_NUMBER: &str = "a";

        #[test]
        fn test_sort_list_without_flags_return_sorted_list_with_numbers_ascending() {
            let mut database = create_database();
            database
                .lpush(LIST, [VALUE_3, VALUE_1, VALUE_2].to_vec())
                .unwrap();

            if let SuccessQuery::List(list) = database.sort(LIST, SortFlags::WithoutFlags).unwrap()
            {
                let list_result: Vec<String> = list.iter().map(|x| x.to_string()).collect();
                let to_compare_list: Vec<&str> = vec![VALUE_1, VALUE_2, VALUE_3];
                let pair_list: Vec<(&String, &str)> =
                    list_result.iter().zip(to_compare_list).collect();
                pair_list.iter().for_each(|x| {
                    assert_eq!(x.0, x.1);
                });
            }
        }

        #[test]
        fn test_sort_list_without_flags_return_err_if_elem_in_list_are_not_numbers() {
            let mut database = create_database();
            database
                .lpush(LIST, [VALUE_1, VALUE_2, VALUE_A].to_vec())
                .unwrap();

            let result = database.sort(LIST, SortFlags::WithoutFlags);

            assert_eq!(result.unwrap_err(), DataBaseError::SortParseError);
        }

        #[test]
        fn test_sort_list_with_alpha_on_return_sorted_list_with_numbers_and_string_values() {
            let mut database = create_database();
            database
                .lpush(LIST, [VALUE_A, VALUE_2, VALUE_3].to_vec())
                .unwrap();

            if let SuccessQuery::List(list) = database.sort(LIST, SortFlags::Alpha).unwrap() {
                let list_result: Vec<String> = list.iter().map(|x| x.to_string()).collect();
                let to_compare_list: Vec<&str> = vec![VALUE_2, VALUE_3, VALUE_A];
                let pair_list: Vec<(&String, &str)> =
                    list_result.iter().zip(to_compare_list).collect();
                pair_list.iter().for_each(|x| {
                    assert_eq!(x.0, x.1);
                });
            }
        }

        #[test]
        fn test_sort_list_with_desc_on_return_sorted_list_with_numbers_descending() {
            let mut database = create_database();
            database
                .lpush(LIST, [VALUE_1, VALUE_3, VALUE_2].to_vec())
                .unwrap();

            if let SuccessQuery::List(list) = database.sort(LIST, SortFlags::Desc).unwrap() {
                let list_result: Vec<String> = list.iter().map(|x| x.to_string()).collect();
                let to_compare_list: Vec<&str> = vec![VALUE_3, VALUE_2, VALUE_1];
                let pair_list: Vec<(&String, &str)> =
                    list_result.iter().zip(to_compare_list).collect();
                pair_list.iter().for_each(|x| {
                    assert_eq!(x.0, x.1);
                });
            }
        }

        #[test]
        fn test_sort_list_with_limit_offset_count_0_return_empty_list() {
            let mut database = create_database();
            database
                .lpush(LIST, [VALUE_1, VALUE_3, VALUE_2].to_vec())
                .unwrap();

            if let SuccessQuery::List(list) = database
                .sort(LIST, SortFlags::Limit(LIMIT_OFFSET_OFF, LIMIT_COUNT_ZERO))
                .unwrap()
            {
                assert!(list.is_empty());
            }
        }

        #[test]
        fn test_sort_list_with_limit_offset_count_in_range_of_list_return_sorted_list_with_count_elem(
        ) {
            let mut database = create_database();
            database
                .lpush(LIST, [VALUE_1, VALUE_3, VALUE_2].to_vec())
                .unwrap();

            if let SuccessQuery::List(list) = database
                .sort(LIST, SortFlags::Limit(LIMIT_OFFSET_OFF, 2))
                .unwrap()
            {
                let list_result: Vec<String> = list.iter().map(|x| x.to_string()).collect();
                let to_compare_list: Vec<&str> = vec![VALUE_1, VALUE_2];
                let pair_list: Vec<(&String, &str)> =
                    list_result.iter().zip(to_compare_list).collect();
                pair_list.iter().for_each(|x| {
                    assert_eq!(x.0, x.1);
                });
                assert_eq!(list.len(), 2);
            }
        }

        #[test]
        fn test_sort_list_with_limit_offset_count_negative_return_sorted_list_with_all_elements() {
            let mut database = create_database();
            database
                .lpush(LIST, [VALUE_1, VALUE_3, VALUE_2].to_vec())
                .unwrap();

            if let SuccessQuery::List(list) = database
                .sort(LIST, SortFlags::Limit(LIMIT_OFFSET_OFF, -1))
                .unwrap()
            {
                let list_result: Vec<String> = list.iter().map(|x| x.to_string()).collect();
                let to_compare_list: Vec<&str> = vec![VALUE_1, VALUE_2, VALUE_3];
                let pair_list: Vec<(&String, &str)> =
                    list_result.iter().zip(to_compare_list).collect();
                pair_list.iter().for_each(|x| {
                    assert_eq!(x.0, x.1);
                });
                assert_eq!(list.len(), 3);
            }
        }

        #[test]
        fn test_sort_list_with_limit_offset_neg_count_0_return_emptylist() {
            let mut database = create_database();
            database
                .lpush(LIST, [VALUE_1, VALUE_3, VALUE_2].to_vec())
                .unwrap();

            if let SuccessQuery::List(list) = database
                .sort(LIST, SortFlags::Limit(-2, LIMIT_COUNT_ZERO))
                .unwrap()
            {
                assert!(list.is_empty());
            }
        }

        #[test]
        fn test_sort_list_with_limit_offset_out_of_range_of_list_count_return_empty_list() {
            let mut database = create_database();
            database
                .lpush(LIST, [VALUE_1, VALUE_3, VALUE_2].to_vec())
                .unwrap();

            if let SuccessQuery::List(list) = database
                .sort(LIST, SortFlags::Limit(4, LIMIT_COUNT_OFF))
                .unwrap()
            {
                assert!(list.is_empty());
            }
        }

        #[test]
        fn test_sort_list_with_limit_offset_negative_count_negative_return_sorted_list_with_all_elements(
        ) {
            let mut database = create_database();
            database
                .lpush(LIST, [VALUE_1, VALUE_3, VALUE_2].to_vec())
                .unwrap();

            if let SuccessQuery::List(list) = database.sort(LIST, SortFlags::Limit(-2, -2)).unwrap()
            {
                let list_result: Vec<String> = list.iter().map(|x| x.to_string()).collect();
                let to_compare_list: Vec<&str> = vec![VALUE_1, VALUE_2, VALUE_3];
                let pair_list: Vec<(&String, &str)> =
                    list_result.iter().zip(to_compare_list).collect();
                pair_list.iter().for_each(|x| {
                    assert_eq!(x.0, x.1);
                });
                assert_eq!(list.len(), 3);
            }
        }

        #[test]
        fn test_sort_list_with_limit_offset_in_range_count_2_return_sorted_sublist_of_2_elements() {
            let mut database = create_database();
            database
                .lpush(LIST, [VALUE_1, VALUE_3, VALUE_2].to_vec())
                .unwrap();

            if let SuccessQuery::List(list) = database.sort(LIST, SortFlags::Limit(1, 2)).unwrap() {
                let list_result: Vec<String> = list.iter().map(|x| x.to_string()).collect();
                let to_compare_list: Vec<&str> = vec![VALUE_2, VALUE_3];
                let pair_list: Vec<(&String, &str)> =
                    list_result.iter().zip(to_compare_list).collect();
                pair_list.iter().for_each(|x| {
                    assert_eq!(x.0, x.1);
                });
                assert_eq!(list.len(), 2);
            }
        }

        #[test]
        fn test_sort_set_by_weight_return_list_ordered_by_weight() {
            let mut database = create_database();

            database.set(KEY_WEIGHT_3, VAL_WEIGHT_3).unwrap();
            database.set(KEY_WEIGHT_1, VAL_WEIGHT_1).unwrap();
            database.set(KEY_WEIGHT_2, VAL_WEIGHT_2).unwrap();

            database.sadd(SET, [VALUE_1].to_vec()).unwrap();
            database.sadd(SET, [VALUE_3].to_vec()).unwrap();
            database.sadd(SET, [VALUE_2].to_vec()).unwrap();

            if let SuccessQuery::List(list) = database.sort(SET, SortFlags::By(PATTERN)).unwrap() {
                let list_result: Vec<String> = list.iter().map(|x| x.to_string()).collect();
                let to_compare_list: Vec<&str> = vec![VALUE_1, VALUE_3, VALUE_2];
                let pair_list: Vec<(&String, &str)> =
                    list_result.iter().zip(to_compare_list).collect();
                pair_list.iter().for_each(|x| {
                    assert_eq!(x.0, x.1);
                });
                assert_eq!(list.len(), 3);
            }
        }

        #[test]
        fn test_sort_list_by_weight_with_pattern_that_no_match_return_list_unordered() {
            let mut database = create_database();

            database.set(KEY_WEIGHT_3, VAL_WEIGHT_3).unwrap();
            database.set(KEY_WEIGHT_1, VAL_WEIGHT_1).unwrap();
            database.set(KEY_WEIGHT_2, VAL_WEIGHT_2).unwrap();

            database.rpush(LIST, vec![VALUE_1]).unwrap();
            database.rpush(LIST, vec![VALUE_3]).unwrap();
            database.rpush(LIST, vec![VALUE_2]).unwrap();

            if let SuccessQuery::List(list) =
                database.sort(LIST, SortFlags::By(UNMATCH_PATTERN)).unwrap()
            {
                let list_result: Vec<String> = list.iter().map(|x| x.to_string()).collect();
                let to_compare_list: Vec<&str> = vec![VALUE_1, VALUE_3, VALUE_2];
                let pair_list: Vec<(&String, &str)> =
                    list_result.iter().zip(to_compare_list).collect();
                pair_list.iter().for_each(|x| {
                    assert_eq!(x.0, x.1);
                });
                assert_eq!(list.len(), 3);
            }
        }

        #[test]
        fn test_sort_set_by_weight_if_weight_of_elem_that_does_not_match_weight_is_set_to_zero_and_return_list_ordered_by_pattern(
        ) {
            let mut database = create_database();

            database.set(KEY_WEIGHT_3, VAL_WEIGHT_3).unwrap();
            database.set(KEY_WEIGHT_1, VAL_WEIGHT_1).unwrap();
            // Weight of VALUE_2 is zero.

            database.sadd(SET, [VALUE_1].to_vec()).unwrap();
            database.sadd(SET, [VALUE_3].to_vec()).unwrap();
            database.sadd(SET, [VALUE_2].to_vec()).unwrap();

            if let SuccessQuery::List(list) = database.sort(SET, SortFlags::By(PATTERN)).unwrap() {
                let list_result: Vec<String> = list.iter().map(|x| x.to_string()).collect();
                let to_compare_list: Vec<&str> = vec![VALUE_2, VALUE_1, VALUE_3];
                let pair_list: Vec<(&String, &str)> =
                    list_result.iter().zip(to_compare_list).collect();
                pair_list.iter().for_each(|x| {
                    assert_eq!(x.0, x.1);
                });
                assert_eq!(list.len(), 3);
            }
        }

        #[test]
        fn test_sort_set_by_weight_and_only_weight_isnt_a_number_return_err() {
            let mut database = create_database();

            database.set(KEY_WEIGHT_3, VAL_WEIGHT_3).unwrap();
            database.set(KEY_WEIGHT_1, VAL_WEIGHT_1).unwrap();
            database.set(KEY_WEIGHT_2, VAL_WEIGHT_2_NOT_NUMBER).unwrap();

            database.sadd(SET, [VALUE_1].to_vec()).unwrap();
            database.sadd(SET, [VALUE_3].to_vec()).unwrap();
            database.sadd(SET, [VALUE_2].to_vec()).unwrap();

            let result = database.sort(SET, SortFlags::By(PATTERN));
            assert_eq!(result.unwrap_err(), DataBaseError::SortByParseError);
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

    const DB_DUMP: &str = "db_dump_path.txt";
    use super::*;

    fn create_database() -> Database {
        let db = Database::new(DB_DUMP.to_string());
        db
    }

    fn database_with_a_list() -> Database {
        let mut database = create_database();

        database
            .lpush(KEY, [VALUEA, VALUEB, VALUEC, VALUED].to_vec())
            .unwrap();

        database
    }

    fn database_with_a_three_repeated_values() -> Database {
        let mut database = create_database();

        database
            .lpush(KEY, [VALUEA, VALUEA, VALUEC, VALUEA].to_vec())
            .unwrap();

        database
    }

    fn database_with_a_string() -> Database {
        let mut database = create_database();

        database.append(KEY, VALUE).unwrap();

        database
    }

    mod llen_test {

        use super::*;

        #[test]
        fn test_llen_on_a_non_existent_key_gets_len_zero() {
            let mut database = create_database();

            let result = database.llen(KEY);

            assert_eq!(result.unwrap(), SuccessQuery::Integer(0));
        }

        #[test]
        fn test_llen_on_a_list_with_one_value() {
            let mut database = create_database();

            database.lpush(KEY, [VALUE].to_vec()).unwrap();

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
            let mut database = create_database();

            let result = database.lindex(KEY, 10);

            assert_eq!(result.unwrap(), SuccessQuery::Nil);
        }

        #[test]
        fn test_lindex_with_a_list_with_one_value_on_idex_zero() {
            let mut database = create_database();

            database.lpush(KEY, [VALUE].to_vec()).unwrap();

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
            let mut database = create_database();

            let result = database.lpop(KEY).unwrap();

            assert_eq!(result, SuccessQuery::Nil);
        }

        #[test]
        fn test_lpop_with_a_list_with_one_value_on_idex_zero() {
            let mut database = create_database();

            database.lpush(KEY, [VALUE].to_vec()).unwrap();

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
            let dictionary = database.dictionary;
            let dictionary = dictionary.get_atomic_hash(KEY);
            let dictionary = dictionary.lock().unwrap();

            if let (StorageValue::List(list), _) = dictionary.get(KEY).unwrap() {
                assert_eq!(list[0], VALUEC);
            }
        }

        #[test]
        fn test_lpop_on_an_empty_list() {
            let mut database = create_database();

            database.lpush(KEY, [VALUE].to_vec()).unwrap();

            if let SuccessQuery::String(val) = database.lpop(KEY).unwrap() {
                assert_eq!(val.to_string(), VALUE);
            }

            let value = database.lpop(KEY).unwrap();
            assert_eq!(value, SuccessQuery::Nil);

            let dictionary = database.dictionary;
            let dictionary = dictionary.get_atomic_hash(KEY);
            let dictionary = dictionary.lock().unwrap();

            if let (StorageValue::List(list), _) = dictionary.get(KEY).unwrap() {
                assert!(list.is_empty());
            }
        }
    }

    mod lpush_test {

        use super::*;

        #[test]
        fn test_lpush_on_a_non_existent_key_creates_a_list_with_new_value() {
            let mut database = create_database();

            let result = database.lpush(KEY, [VALUE].to_vec()).unwrap();
            assert_eq!(result, SuccessQuery::Integer(1));

            let dictionary = database.dictionary;
            let dictionary = dictionary.get_atomic_hash(KEY);
            let dictionary = dictionary.lock().unwrap();

            if let (StorageValue::List(list), _) = dictionary.get(KEY).unwrap() {
                assert_eq!(list.len(), 1);
                assert_eq!(list[0], VALUE);
            }
        }

        #[test]
        fn test_lpush_on_an_existent_key_is_valid() {
            let mut database = create_database();

            database.lpush(KEY, [VALUEA].to_vec()).unwrap();

            let result = database.lpush(KEY, [VALUEB].to_vec()).unwrap();

            assert_eq!(result, SuccessQuery::Integer(2));

            let dictionary = database.dictionary;
            let dictionary = dictionary.get_atomic_hash(KEY);
            let dictionary = dictionary.lock().unwrap();

            if let (StorageValue::List(list), _) = dictionary.get(KEY).unwrap() {
                assert_eq!(list.len(), 2);
                assert_eq!(list[1], VALUEA);
                assert_eq!(list[0], VALUEB);
            }
        }

        #[test]
        fn test_lpush_on_an_existent_key_that_isnt_a_list() {
            let mut database = create_database();

            database.append(KEY, VALUE).unwrap();

            let result = database.lpush(KEY, [VALUE].to_vec()).unwrap_err();

            assert_eq!(result, DataBaseError::NotAList);
        }

        #[test]
        fn test_lpush_more_than_one_value_with_a_key_non_existent() {
            let mut database = create_database();

            let result = database
                .lpush(KEY, [VALUEA, VALUEB, VALUEC].to_vec())
                .unwrap();

            assert_eq!(result, SuccessQuery::Integer(3));

            let dictionary = database.dictionary;
            let dictionary = dictionary.get_atomic_hash(KEY);
            let dictionary = dictionary.lock().unwrap();

            if let (StorageValue::List(list), _) = dictionary.get(KEY).unwrap() {
                assert_eq!(list.len(), 3);
                assert_eq!(list[2], VALUEA);
                assert_eq!(list[1], VALUEB);
                assert_eq!(list[0], VALUEC);
            }
        }

        #[test]
        fn test_lpush_more_than_one_value_with_a_key_with_list() {
            let mut database = create_database();

            database.lpush(KEY, [VALUEA].to_vec()).unwrap();
            let result = database.lpush(KEY, [VALUEB, VALUEC].to_vec()).unwrap();

            assert_eq!(result, SuccessQuery::Integer(3));

            let dictionary = database.dictionary;
            let dictionary = dictionary.get_atomic_hash(KEY);
            let dictionary = dictionary.lock().unwrap();

            if let (StorageValue::List(list), _) = dictionary.get(KEY).unwrap() {
                assert_eq!(list.len(), 3);
                assert_eq!(list[2], VALUEA);
                assert_eq!(list[1], VALUEB);
                assert_eq!(list[0], VALUEC);
            }
        }
    }

    mod lrange_test {
        use super::*;

        #[test]
        fn test_lrange_on_zero_zero_range() {
            let mut database = create_database();

            database.lpush(KEY, [VALUE].to_vec()).unwrap();

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

            let dictionary = database.dictionary;
            let dictionary = dictionary.get_atomic_hash(KEY);
            let dictionary = dictionary.lock().unwrap();

            if let Some((StorageValue::List(list), _)) = dictionary.get(KEY) {
                assert_eq!(list[0], VALUEC);
                assert_eq!(list[1], VALUEA);
            }
        }

        #[test]
        fn test_lrem_negative_2_on_a_list() {
            let mut database = database_with_a_three_repeated_values();

            let result = database.lrem(KEY, -2, VALUEA);

            assert_eq!(SuccessQuery::Integer(2), result.unwrap());

            let dictionary = database.dictionary;
            let dictionary = dictionary.get_atomic_hash(KEY);
            let dictionary = dictionary.lock().unwrap();

            if let (StorageValue::List(list), _) = dictionary.get(KEY).unwrap() {
                assert_eq!(list[0], VALUEA);
                assert_eq!(list[1], VALUEC);
            }
        }

        #[test]
        fn test_lrem_zero_on_a_list() {
            let mut database = database_with_a_three_repeated_values();

            let result = database.lrem(KEY, 0, VALUEA);

            let dictionary = database.dictionary;
            let dictionary = dictionary.get_atomic_hash(KEY);
            let dictionary = dictionary.lock().unwrap();

            assert_eq!(SuccessQuery::Integer(1), result.unwrap());
            if let (StorageValue::List(list), _) = dictionary.get(KEY).unwrap() {
                assert_eq!(list[0], VALUEC);
            }
        }

        #[test]
        fn test_lset_with_a_non_existen_key() {
            let mut database = create_database();

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

            let dictionary = database.dictionary;
            let dictionary = dictionary.get_atomic_hash(KEY);
            let dictionary = dictionary.lock().unwrap();

            if let (StorageValue::List(list), _) = dictionary.get(KEY).unwrap() {
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

    const DB_DUMP: &str = "db_dump_path.txt";

    fn create_database() -> Database {
        let db = Database::new(DB_DUMP.to_string());
        db
    }

    mod saad_test {
        use super::*;

        #[test]
        fn test_sadd_create_new_set_with_element_returns_1_if_key_set_not_exist_in_database() {
            let mut database = create_database();

            let result = database.sadd(KEY, [ELEMENT].to_vec()).unwrap();
            assert_eq!(result, SuccessQuery::Integer(1));
            let is_member = database.sismember(KEY, ELEMENT).unwrap();
            assert_eq!(is_member, SuccessQuery::Boolean(true));
        }

        #[test]
        fn test_sadd_create_set_with_repeating_elements_returns_0() {
            let mut database = create_database();

            database.sadd(KEY, [ELEMENT].to_vec()).unwrap();

            let result = database.sadd(KEY, [ELEMENT].to_vec()).unwrap();
            assert_eq!(result, SuccessQuery::Integer(0));
            let len_set = database.scard(KEY).unwrap();
            assert_eq!(len_set, SuccessQuery::Integer(1));
        }

        #[test]
        fn test_sadd_key_with_another_type_of_set_returns_err() {
            let mut database = create_database();
            database.set(KEY, ELEMENT).unwrap();

            let result = database.sadd(KEY, [ELEMENT].to_vec()).unwrap_err();
            assert_eq!(result, DataBaseError::NotASet);
        }

        #[test]
        fn test_sadd_add_element_with_set_created_returns_1() {
            let mut database = create_database();
            database.sadd(KEY, [ELEMENT].to_vec()).unwrap();

            let result = database.sadd(KEY, [OTHER_ELEMENT].to_vec()).unwrap();
            assert_eq!(result, SuccessQuery::Integer(1));
            let len_set = database.scard(KEY).unwrap();
            assert_eq!(len_set, SuccessQuery::Integer(2));
        }

        #[test]
        fn test_sadd_add_multiple_elem_in_set_returns_lenght_of_correctly_added_set() {
            let mut database = create_database();
            database.sadd(KEY, [ELEMENT].to_vec()).unwrap();

            let result = database
                .sadd(KEY, [ELEMENT, ELEMENT_2, ELEMENT_3].to_vec())
                .unwrap();

            assert_eq!(result, SuccessQuery::Integer(2));
            let len_set = database.scard(KEY).unwrap();
            assert_eq!(len_set, SuccessQuery::Integer(3));
        }

        #[test]
        fn test_sadd_add_multiple_elem_in_key_that_not_contains_set_return_err() {
            let mut database = create_database();
            database.sadd(KEY, [ELEMENT].to_vec()).unwrap();
            database.set(KEY, ELEMENT).unwrap();

            let result = database
                .sadd(KEY, [ELEMENT, ELEMENT_2, ELEMENT_3].to_vec())
                .unwrap_err();

            assert_eq!(result, DataBaseError::NotASet);
        }
    }

    mod sismember_test {
        use super::*;

        #[test]
        fn test_sismember_set_with_element_returns_1() {
            let mut database = create_database();
            database.sadd(KEY, [ELEMENT].to_vec()).unwrap();

            let is_member = database.sismember(KEY, ELEMENT).unwrap();

            assert_eq!(is_member, SuccessQuery::Boolean(true));
        }

        #[test]
        fn test_sismember_set_without_element_returns_0() {
            let mut database = create_database();
            database.sadd(KEY, [ELEMENT].to_vec()).unwrap();

            let result = database.sismember(KEY, OTHER_ELEMENT).unwrap();

            assert_eq!(result, SuccessQuery::Boolean(false));
        }

        #[test]
        fn test_sismember_key_with_another_type_of_set_returns_err() {
            let mut database = create_database();
            database.set(KEY, ELEMENT).unwrap();
            let result = database.sismember(KEY, ELEMENT).unwrap_err();

            assert_eq!(result, DataBaseError::NotASet);
        }

        #[test]
        fn test_sismember_with_non_exist_key_set_returns_0() {
            let mut database = create_database();

            let result = database.sismember(KEY, ELEMENT).unwrap();

            assert_eq!(result, SuccessQuery::Boolean(false));
        }
    }

    mod srem_test {
        use super::*;
        #[test]
        fn test_srem_one_member_in_set_returns_1_if_set_contains_member() {
            let members = vec![ELEMENT];
            let mut database = create_database();
            database.sadd(KEY, [ELEMENT].to_vec()).unwrap();
            let result = database.srem(KEY, members).unwrap();
            let is_member = database.sismember(KEY, &ELEMENT.to_string()).unwrap();
            assert_eq!(result, SuccessQuery::Integer(1));
            assert_eq!(is_member, SuccessQuery::Boolean(false));
        }
        #[test]
        fn test_srem_key_no_exits_in_database_returns_0() {
            let mut database = create_database();
            let members_to_rmv = vec![ELEMENT];
            let result = database.srem(KEY, members_to_rmv).unwrap();
            assert_eq!(result, SuccessQuery::Boolean(false));
        }
        #[test]
        fn test_srem_multiple_members_in_set_returns_lenght_of_removed_elements_in_the_set() {
            let mut database = create_database();
            let members = vec![ELEMENT, ELEMENT_2, ELEMENT_3];
            let members_to_rmv = vec![ELEMENT, ELEMENT_2, ELEMENT_3, NON_EXIST_ELEMENT];

            let _ = database.sadd(KEY, members.clone());

            let result = database.srem(KEY, members_to_rmv).unwrap();
            assert_eq!(result, SuccessQuery::Integer(3));
            for member in members {
                let is_member = database.sismember(KEY, &member.to_string()).unwrap();
                assert_eq!(is_member, SuccessQuery::Boolean(false));
            }
        }
        #[test]
        fn test_srem_key_with_another_type_another_besides_set_return_err() {
            let mut database = create_database();
            let members_to_rmv = vec![ELEMENT];
            let _ = database.set(KEY_WITH_STR, VALUE_A).unwrap();
            let result = database.srem(KEY_WITH_STR, members_to_rmv).unwrap_err();
            assert_eq!(result, DataBaseError::NotASet);
        }
    }

    #[test]
    fn test_scard_set_with_one_element_returns_1() {
        let mut database = create_database();
        database.sadd(KEY, [ELEMENT].to_vec()).unwrap();

        let len_set = database.scard(KEY).unwrap();

        assert_eq!(len_set, SuccessQuery::Integer(1));
    }

    #[test]
    fn test_scard_create_set_with_multiple_elements_returns_lenght_of_set() {
        let mut database = create_database();
        let elements = vec!["0", "1", "2", "3"];

        let _ = database.sadd(KEY, elements);

        let len_set = database.scard(KEY).unwrap();
        assert_eq!(len_set, SuccessQuery::Integer(4));
    }

    #[test]
    fn test_scard_key_with_another_type_of_set_returns_err() {
        let mut database = create_database();
        database.set(KEY, ELEMENT).unwrap();

        let result = database.scard(KEY).unwrap_err();

        assert_eq!(result, DataBaseError::NotASet);
    }

    #[test]
    fn test_scard_key_set_not_exist_in_database_returns_0() {
        let mut database = create_database();

        let result = database.scard(KEY).unwrap();

        assert_eq!(result, SuccessQuery::Boolean(false));
    }

    mod smembers_test {
        use super::*;

        #[test]
        fn test_smembers_with_elements_return_list_of_elements() {
            let mut database = create_database();
            database.sadd(KEY, [ELEMENT].to_vec()).unwrap();
            database.sadd(KEY, [OTHER_ELEMENT].to_vec()).unwrap();

            if let SuccessQuery::List(list) = database.smembers(KEY).unwrap() {
                for elem in list {
                    let is_member = database.sismember(KEY, &elem.to_string()).unwrap();
                    assert_eq!(is_member, SuccessQuery::Boolean(true));
                }
            }
        }

        #[test]
        fn test_smembers_with_non_exist_key_return_empty_list() {
            let mut database = create_database();

            let result = database.smembers(NON_EXIST_KEY).unwrap();

            assert_eq!(result, SuccessQuery::List(Vec::new()));
        }

        #[test]
        fn test_smembers_key_with_another_type_another_besides_set_return_err() {
            let mut database = create_database();

            let _ = database.set(KEY_WITH_STR, VALUE_A);

            let result = database.smembers(KEY_WITH_STR).unwrap_err();

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

    const DB_DUMP: &str = "db_dump_path.txt";

    fn create_database() -> Database {
        let db = Database::new(DB_DUMP.to_string());
        db
    }

    mod flushdb_test {
        use super::*;

        #[test]
        fn flushdb_clear_dictionary() {
            let mut db = create_database();

            db.mset(vec![KEY1, VALUE1, KEY2, VALUE2]).unwrap();
            let r = db.get(KEY1).unwrap();
            assert_eq!(r, SuccessQuery::String(VALUE1.to_owned()));
            let r = db.get(KEY2).unwrap();
            assert_eq!(r, SuccessQuery::String(VALUE2.to_owned()));

            let r = db.flushdb().unwrap();
            assert_eq!(r, SuccessQuery::Success);

            let guard = db.dictionary;
            assert!(guard.len() == 0);
        }
    }

    mod dbsize_test {
        use super::*;

        #[test]
        fn dbsize_empty_gets_0() {
            let db = create_database();

            let r = db.dbsize().unwrap();
            assert_eq!(r, SuccessQuery::Integer(0));
        }

        #[test]
        fn dbsize_with_one_element_gets_1() {
            let mut db = create_database();
            let _ = db.set(KEY1, VALUE1);
            let r = db.get(KEY1).unwrap();
            assert_eq!(r, SuccessQuery::String(VALUE1.to_owned()));

            let r = db.dbsize().unwrap();
            assert_eq!(r, SuccessQuery::Integer(1));
        }

        #[test]
        fn dbsize_with_two_element_gets_2() {
            let mut db = create_database();
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
