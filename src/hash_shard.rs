use crate::databasehelper::StorageValue;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::SystemTime,
};

/// A HashShard implemented with the simplest hash function in a multithreading 
/// context.
///
/// HashShard uses Arc and Mutex to be shared safety in a multithreading context
/// implementing clone.
/// It is the one in charge of dividing all the data in shorter pieces.
///
#[doc(hidden)]
type Dictionary = Arc<Mutex<HashMap<String, (StorageValue, SystemTime)>>>;
#[doc(hidden)]
const HASH_NUMBER: usize = 10;

pub struct HashShard {
    #[doc(hidden)]
    pub data: Arc<Mutex<Vec<Dictionary>>>,
}

impl HashShard {
    #[doc(hidden)]
    pub fn new_from_hs(data: Arc<Mutex<Vec<Dictionary>>>) -> HashShard {
        HashShard { data }
    }

    /// Creates a new HashShard.
    /// # Examples
    /// Basic Usage:
    /// ```
    /// let mut hashShard = HashShard::new();
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

    pub fn get_atomic_hash(&self, key: &str) -> Dictionary {
        let mut d = self.data.lock().unwrap();
        let atomic_hash = d.get_mut(hash_funcion(key)).unwrap();
        atomic_hash.clone()
    }

    pub fn insert(&mut self, key: String, value: StorageValue) -> Option<StorageValue> {
        let atomic_hash = self.get_atomic_hash(&key);
        let mut atomic_hash = atomic_hash.lock().unwrap();
        let r = atomic_hash.insert(key, (value, SystemTime::now()));
        r.map(|(value, _)| value)
    }

    pub fn clear(&mut self) {
        let data = self.data.lock().unwrap();
        data.iter().for_each(|h| {
            let mut h = h.lock().unwrap();
            h.clear();
        });
    }

    pub fn len(&self) -> usize {
        let data = self.data.lock().unwrap();
        let mut len = 0;
        data.iter().for_each(|h| {
            let h = h.lock().unwrap();
            len += h.len();
        });

        len
    }

    pub fn contains_key(&self, key: &str) -> bool {
        let atomic_hash = self.get_atomic_hash(key);
        let guard = atomic_hash.lock().unwrap();
        guard.contains_key(key)
    }

    pub fn remove(&mut self, key: &str) -> Option<StorageValue> {
        let atomic_hash = self.get_atomic_hash(key);
        let mut guard = atomic_hash.lock().unwrap();
        let r = guard.remove(key);
        r.map(|(v, _)| v)
    }

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
