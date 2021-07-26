use crate::databasehelper::StorageValue;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::SystemTime,
};

#[doc(hidden)]
type Dictionary = Arc<Mutex<HashMap<String, (StorageValue, SystemTime)>>>;
#[doc(hidden)]
const HASH_NUMBER: usize = 10;

/// A HashShard implemented with the simplest hash function in a multithreading
/// context.
///
/// HashShard uses Arc and Mutex to be shared safety in a multithreading context
/// implementing clone.
/// It is the one in charge of dividing all the data in shorter pieces.
///
pub struct HashShard {
    #[doc(hidden)]
    pub data: Arc<Mutex<Vec<Dictionary>>>,
}

impl HashShard {
    #[doc(hidden)]
    pub fn new_from_hs(data: Arc<Mutex<Vec<Dictionary>>>) -> HashShard {
        HashShard { data }
    }

    /// Creates a new HashShard full with HashMaps.
    /// # Examples
    /// Basic Usage:
    /// ```
    /// let hash_shard = HashShard::new();
    /// ```
    pub fn new() -> HashShard {
        let mut data = Vec::with_capacity(HASH_NUMBER);
        for _ in 0..HASH_NUMBER {
            data.push(Arc::new(Mutex::new(HashMap::new())));
        }
        HashShard {
            data: Arc::new(Mutex::new(data)),
        }
    }

    /// Updates the last access for the corresponding key and returns time passed since
    /// that point, if there's no key returns None.
    /// # Examples
    /// Basic Usage:
    /// ```
    /// let mut hash_shard = HashShard::new();
    ///
    /// let last_access = SystemTime::now();
    /// hash_shard.insert(
    ///     "key1".to_string(),
    ///     StorageValue::String("value1".to_string()),
    /// );
    ///
    /// sleep(std::time::Duration::from_secs(2));
    /// let time_passed = SystemTime::now()
    ///     .duration_since(last_access)
    ///     .expect("Clock may have gone backwards")
    ///     .as_secs();
    /// let r = hash_shard.touch("key1").unwrap();
    ///
    /// assert_eq!(r, time_passed);
    /// ```
    pub fn touch(&mut self, key: &str) -> Option<u64> {
        let atomic_hash = self.get_atomic_hash(&key);
        let mut atomic_hash = atomic_hash.lock().unwrap();
        match atomic_hash.get_mut(key) {
            Some((_, l)) => {
                let uptime_in_seconds = SystemTime::now()
                    .duration_since(*l)
                    .expect("Clock may have gone backwards");

                *l = SystemTime::now();
                Some(uptime_in_seconds.as_secs())
            }
            None => None,
        }
    }

    /// Gets a piece from the data wich possibly contains the corresponding key protected
    /// by an Arc and Mutex.
    /// # Examples
    /// Basic Usage:
    /// ```
    /// let mut hash_shard = HashShard::new();
    ///
    /// hash_shard.insert(
    ///     "key1".to_string(),
    ///     StorageValue::String("value1".to_string()),
    /// );
    ///
    /// let atomic_hash = hash_shard.get_atomic_hash("key1");
    /// let atomic_hash = atomic_hash.lock().unwrap();
    ///
    /// assert!(atomic_hash.contains_key("key1"));
    /// ```
    pub fn get_atomic_hash(&self, key: &str) -> Dictionary {
        let mut d = self.data.lock().unwrap();
        let atomic_hash = d.get_mut(hash_funcion(key)).unwrap();
        atomic_hash.clone()
    }

    /// Inserts a key-value pair into the hash shard.
    ///
    /// If the hash shard did not have this key present, None is returned.
    ///
    /// If the hash shard did have this key present, the value is updated, and the old value is
    /// returned.
    /// # Examples
    /// Basic Usage:
    /// ```
    /// let mut hash_shard = HashShard::new();
    /// 
    /// assert!(hash_shard
    ///     .insert("key1".to_string(), StorageValue::String("value1".to_string()))
    ///     .is_none());
    /// assert!(hash_shard.contains_key("key_1"));
    /// 
    /// if let Some(StorageValue::String(old_value)) =
    ///     hash_shard.insert("key_1".to_string(), StorageValue::String("value2".to_string()))
    /// {
    ///     assert_eq!(old_value, "value1");
    /// }
    /// ```
    pub fn insert(&mut self, key: String, value: StorageValue) -> Option<StorageValue> {
        let atomic_hash = self.get_atomic_hash(&key);
        let mut atomic_hash = atomic_hash.lock().unwrap();
        let r = atomic_hash.insert(key, (value, SystemTime::now()));
        r.map(|(value, _)| value)
    }

    /// Clears the hash shard, removing all key-value pairs.
    /// # Examples
    /// Basic Usage:
    /// ```
    /// let mut hash_shard = HashShard::new();
    /// 
    /// for i in 0..100 {
    ///     hash_shard.insert(i.to_string(), StorageValue::String(i.to_string()));
    /// }
    /// assert_eq!(hash_shard.len(), 100);
    /// 
    /// hash_shard.clear();
    /// assert_eq!(hash_shard.len(), 0);
    /// ```
    pub fn clear(&mut self) {
        let data = self.data.lock().unwrap();
        data.iter().for_each(|h| {
            let mut h = h.lock().unwrap();
            h.clear();
        });
    }

    /// Returns the number of elements in the hash shard.
    /// # Examples
    /// Basic Usage:
    /// ```
    /// let mut hash_shard = HashShard::new();
    /// 
    /// for i in 0..100 {
    ///     hash_shard.insert(i.to_string(), StorageValue::String(i.to_string()));
    /// }
    /// assert_eq!(hash_shard.len(), 100);
    /// ```
    pub fn len(&self) -> usize {
        let data = self.data.lock().unwrap();
        let mut len = 0;
        data.iter().for_each(|h| {
            let h = h.lock().unwrap();
            len += h.len();
        });

        len
    }

    /// Returns true if the hash shard contains a value for the specified key, false
    /// otherwise.
    /// # Examples
    /// Basic Usage:
    /// ```
    /// let mut hash_shard = HashShard::new();
    /// assert_eq!(hash_shard.contains_key(KEY_2), false);
    /// 
    /// hash_shard.insert(KEY_2.to_string(), StorageValue::String(VALUE_2.to_string()));
    /// assert_eq!(hash_shard.contains_key(KEY_2), true);
    /// ```
    pub fn contains_key(&self, key: &str) -> bool {
        let atomic_hash = self.get_atomic_hash(key);
        let guard = atomic_hash.lock().unwrap();
        guard.contains_key(key)
    }

    /// Removes a key from the hash shard, returning the value at the key if the key was 
    /// previously in the hash shard, None otherwise.
    /// # Examples
    /// Basic Usage:
    /// ```
    /// let mut hash_shard = HashShard::new();
    /// assert!(!hash_shard.contains_key(KEY_2));
    /// assert!(hash_shard.remove(KEY_2).is_none());
    /// 
    /// hash_shard.insert(KEY_2.to_string(), StorageValue::String(VALUE_2.to_string()));
    /// assert!(hash_shard.contains_key(KEY_2));
    /// 
    /// if let Some(StorageValue::String(value_removed)) = hash_shard.remove(KEY_2) {
    ///     assert_eq!(value_removed, VALUE_2);
    /// }
    /// assert!(!hash_shard.contains_key(KEY_2));
    /// ```
    pub fn remove(&mut self, key: &str) -> Option<StorageValue> {
        let atomic_hash = self.get_atomic_hash(key);
        let mut guard = atomic_hash.lock().unwrap();
        let r = guard.remove(key);
        r.map(|(v, _)| v)
    }

    /// Obtains a list of tuples key-value from all the elements of the hash shard.
    /// # Examples
    /// Basic Usage:
    /// ```
    /// let mut hash_shard = HashShard::new();
    ///
    /// for i in 0..10 {
    ///     hash_shard.insert(i.to_string(), StorageValue::String(i.to_string()));
    /// }
    /// 
    /// let key_values = hash_shard.key_value();
    /// for (key, value) in key_values {
    ///     println!("{}: {:?}", key, value);
    /// }
    /// ```
    pub fn key_value(&self) -> Vec<(String, StorageValue)> {
        let data = self.data.lock().unwrap();
        let mut result: Vec<(String, StorageValue)> = Vec::new();
        for hash in data.iter() {
            let hash = hash.lock().unwrap();
            let mut vec = hash
                .iter()
                .map(|(k, v)| {
                    let k = k.to_string();
                    let v = v.clone();
                    (k, v.0)
                })
                .collect::<Vec<(String, StorageValue)>>();
            result.append(&mut vec);
        }

        result
    }

    /// Obtains a list of all keys in the hash shard.
    /// # Examples
    /// Basic Usage:
    /// ```
    /// let mut hash_shard = HashShard::new();
    ///
    /// for i in 0..5 {
    ///     hash_shard.insert(i.to_string(), StorageValue::String(i.to_string()));
    /// }
    /// 
    /// let mut keys = hash_shard.keys();
    /// keys.sort_by(|a, b| {
    ///     let a = a.parse::<i32>().unwrap();
    ///     let b = b.parse::<i32>().unwrap();
    ///     a.cmp(&b)
    /// });
    ///
    /// assert_eq!(keys, vec!["0", "1", "2", "3", "4"]);
    /// ```
    pub fn keys(&self) -> Vec<String> {
        let data = self.data.lock().unwrap();
        let mut result: Vec<String> = Vec::new();
        for hash in data.iter() {
            let hash = hash.lock().unwrap();
            let mut vec = hash.keys().map(|k| k.to_string()).collect::<Vec<String>>();
            result.append(&mut vec);
        }

        result
    }
}

#[doc(hidden)]
fn hash_funcion(key: &str) -> usize {
    key.len() % HASH_NUMBER
}

impl<'a> Clone for HashShard {
    fn clone(&self) -> Self {
        HashShard::new_from_hs(self.data.clone())
    }
}

#[cfg(test)]
mod hash_shard_test {
    use std::thread::sleep;

    use super::*;

    const KEY_1: &str = "key1";
    const KEY_2: &str = "key2";

    const VALUE_1: &str = "value1";
    const VALUE_2: &str = "value2";

    #[test]
    fn touch_updates_last_access_properly() {
        let mut hash_shard = HashShard::new();

        let last_access = SystemTime::now();
        hash_shard.insert(KEY_1.to_string(), StorageValue::String(VALUE_1.to_string()));

        sleep(std::time::Duration::from_secs(2));
        let time_passed = SystemTime::now()
            .duration_since(last_access)
            .expect("Clock may have gone backwards")
            .as_secs();
        let r = hash_shard.touch(KEY_1).unwrap();

        assert_eq!(r, time_passed);
    }

    #[test]
    fn get_atomic_hash_works_properly() {
        let mut hash_shard = HashShard::new();

        hash_shard.insert(KEY_1.to_string(), StorageValue::String(VALUE_1.to_string()));

        let atomic_hash = hash_shard.get_atomic_hash(KEY_1);
        let atomic_hash = atomic_hash.lock().unwrap();

        assert!(atomic_hash.contains_key(KEY_1));
    }

    #[test]
    fn inserts_works_properly() {
        let mut hash_shard = HashShard::new();

        assert!(hash_shard
            .insert(KEY_1.to_string(), StorageValue::String(VALUE_1.to_string()))
            .is_none());
        assert!(hash_shard.contains_key(KEY_1));

        if let Some(StorageValue::String(old_value)) =
            hash_shard.insert(KEY_1.to_string(), StorageValue::String(VALUE_2.to_string()))
        {
            assert_eq!(old_value, VALUE_1);
        }
    }

    #[test]
    fn insert_100_elements_then_len_obtains_100() {
        let mut hash_shard = HashShard::new();

        for i in 0..100 {
            hash_shard.insert(i.to_string(), StorageValue::String(i.to_string()));
        }
        assert_eq!(hash_shard.len(), 100);
    }

    #[test]
    fn clear_then_there_are_no_keys() {
        let mut hash_shard = HashShard::new();

        for i in 0..100 {
            hash_shard.insert(i.to_string(), StorageValue::String(i.to_string()));
        }
        assert_eq!(hash_shard.len(), 100);

        hash_shard.clear();
        assert_eq!(hash_shard.len(), 0);
    }

    #[test]
    fn contains_key_gets_flase_then_adding_gets_true() {
        let mut hash_shard = HashShard::new();
        assert_eq!(hash_shard.contains_key(KEY_2), false);

        hash_shard.insert(KEY_2.to_string(), StorageValue::String(VALUE_2.to_string()));
        assert_eq!(hash_shard.contains_key(KEY_2), true);
        assert_eq!(hash_shard.contains_key(KEY_1), false);
    }

    #[test]
    fn remove_inexistent_key_gets_none_then_adding_it_gets_its_old_value_and_remove_the_key() {
        let mut hash_shard = HashShard::new();
        assert!(!hash_shard.contains_key(KEY_2));
        assert!(hash_shard.remove(KEY_2).is_none());

        hash_shard.insert(KEY_2.to_string(), StorageValue::String(VALUE_2.to_string()));
        assert!(hash_shard.contains_key(KEY_2));

        if let Some(StorageValue::String(value_removed)) = hash_shard.remove(KEY_2) {
            assert_eq!(value_removed, VALUE_2);
        }
        assert!(!hash_shard.contains_key(KEY_2));
    }

    #[test]
    fn get_keys_works_properly() {
        let mut hash_shard = HashShard::new();

        for i in 0..5 {
            hash_shard.insert(i.to_string(), StorageValue::String(i.to_string()));
        }
        
        let mut keys = hash_shard.keys();
        keys.sort_by(|a, b| {
            let a = a.parse::<i32>().unwrap();
            let b = b.parse::<i32>().unwrap();
            a.cmp(&b)
        });

        assert_eq!(keys, vec!["0", "1", "2", "3", "4"]);
    }
}
