use crate::matcher::matcher;
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::sync::Arc;
use std::sync::Mutex;

const VERBOSE: &str = "verbose";
const PORT: &str = "port";
const TIMEOUT: &str = "timeout";
const DBFILENAME: &str = "dbfilename";
const LOGFILE: &str = "logfile";
const DEFAULT_VERBOSE: u64 = 0;
const DEFAULT_PORT: u64 = 8888;
const DEFAULT_TIMEOUT: u64 = 0;
const DEFAULT_DBFILENAME: &str = "dump.txt";
const DEFAULT_LOGFILE: &str = "lf.log";
const NUMERIC_KEYS: [&str; 2] = [VERBOSE, TIMEOUT];
const INVALID_SETEABLE: [&str; 3] = [LOGFILE, PORT, DBFILENAME];
const MIN_PORT: i64 = 1024;
const MAX_PORT: i64 = 49151;

pub struct ServerConf {
    conf: Arc<Mutex<HashMap<String, String>>>,
}

#[derive(Debug, PartialEq)]
pub enum SuccessServerRequest {
    Success,
    String(String),
    List(Vec<SuccessServerRequest>),
}

#[derive(Debug, PartialEq)]
pub enum ServerError {
    NonExistentConfigOption,
    NotAnInteger,
    InvalidPortValue,
    NoSeteableOption(String),
}

impl Clone for ServerConf {
    fn clone(&self) -> Self {
        ServerConf::new_from_conf(self.conf.clone())
    }
}

impl ServerConf {
    pub fn new_from_conf(conf: Arc<Mutex<HashMap<String, String>>>) -> ServerConf {
        ServerConf { conf }
    }

    pub fn new(config_file: &str) -> Result<ServerConf, String> {
        let conf = default_values();
        let file = match File::open(config_file) {
            Ok(f) => f,
            _ => return Err("Problem open file".to_string()),
        };

        let file = BufReader::new(file);

        let mut guard = conf.lock().unwrap();

        for line in file.lines() {
            let line = line.unwrap();
            let element: Vec<String> = line.split('=').map(|s| s.trim().to_string()).collect();

            match &element[..] {
                [key, value] => {
                    guard.insert(key.to_string(), value.to_string());
                }
                _ => return Err("Invalid format config file".to_string()),
            }
        }

        drop(guard);

        Ok(ServerConf { conf })
    }

    pub fn port(&self) -> u64 {
        let conf = self.conf.lock().unwrap();

        if let Some(value) = conf.get(PORT) {
            if let Ok(v) = value.parse::<u64>() {
                return v;
            }
        }

        DEFAULT_PORT
    }

    pub fn addr(&self) -> String {
        let port = self.port();
        "0.0.0.0:".to_owned() + &port.to_string()
    }

    pub fn logfile(&self) -> String {
        let conf = self.conf.lock().unwrap();

        match conf.get(LOGFILE) {
            Some(value) => value.to_string(),
            None => DEFAULT_LOGFILE.to_string(),
        }
    }

    pub fn get_config(&self, pattern: &str) -> Result<SuccessServerRequest, ServerError> {
        let conf = self.conf.lock().unwrap();
        let mut list = Vec::new();
        for (k, v) in conf.iter() {
            if !matcher(k, pattern) {
                continue;
            }

            list.push(SuccessServerRequest::String(format!("{}: {}", k, v)));
        }

        if list.is_empty() {
            return Err(ServerError::NonExistentConfigOption);
        }

        Ok(SuccessServerRequest::List(list))
    }

    pub fn set_config(
        &mut self,
        option: &str,
        new_value: &str,
    ) -> Result<SuccessServerRequest, ServerError> {
        let mut conf = self.conf.lock().unwrap();

        if NUMERIC_KEYS.contains(&option) && new_value.parse::<i64>().is_err() {
            return Err(ServerError::NotAnInteger);
        }

        if INVALID_SETEABLE.contains(&option) {
            return Err(ServerError::NoSeteableOption(option.to_string()));
        }

        if conf.contains_key(option) {
            if option == PORT {
                let value = new_value.parse::<i64>().unwrap();
                if (MIN_PORT..=MAX_PORT).contains(&value) {
                    conf.insert(option.to_string(), new_value.to_string());
                    Ok(SuccessServerRequest::Success)
                } else {
                    Err(ServerError::InvalidPortValue)
                }
            } else {
                conf.insert(option.to_string(), new_value.to_string());
                Ok(SuccessServerRequest::Success)
            }
        } else {
            Err(ServerError::NonExistentConfigOption)
        }
    }

    pub fn time_out(&self) -> u64 {
        let conf = self.conf.lock().unwrap();

        if let Some(value) = conf.get(TIMEOUT) {
            if let Ok(v) = value.parse::<u64>() {
                return v;
            }
        }

        DEFAULT_TIMEOUT
    }

    pub fn dbfilename(&self) -> String {
        let conf = self.conf.lock().unwrap();
        match conf.get(DBFILENAME) {
            Some(value) => value.to_string(),
            None => DEFAULT_DBFILENAME.to_string(),
        }
    }

    pub fn verbose(&self) -> bool {
        if let Some(value) = self.conf.lock().unwrap().get(VERBOSE) {
            if let Ok(v) = value.parse::<u64>() {
                return v != 0;
            }
        }

        DEFAULT_VERBOSE != 0
    }
}

impl fmt::Display for SuccessServerRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SuccessServerRequest::Success => write!(f, "Ok"),
            SuccessServerRequest::String(val) => write!(f, "{}", val),
            SuccessServerRequest::List(list) => {
                if list.is_empty() {
                    write!(f, "(empty list or set)")
                } else {
                    let mut hash_set_string = String::new();

                    for elem in list {
                        hash_set_string.push_str(&elem.to_string());
                        hash_set_string.push(' ');
                    }

                    write!(f, "{}", hash_set_string)
                }
            }
        }
    }
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServerError::NonExistentConfigOption => write!(f, "Non-existent config option"),
            ServerError::NotAnInteger => write!(f, "Value isn't an Integer"),
            ServerError::InvalidPortValue => write!(
                f,
                "Port mus be a value betwen {} and {}",
                MIN_PORT, MAX_PORT
            ),
            ServerError::NoSeteableOption(option) => {
                write!(f, "ERR Unsupported CONFIG parameter: {}", option)
            }
        }
    }
}

fn default_values() -> Arc<Mutex<HashMap<String, String>>> {
    let d = Arc::new(Mutex::new(HashMap::new()));

    let mut guard = d.lock().unwrap();

    // Inserting default values
    guard.insert(VERBOSE.to_string(), DEFAULT_VERBOSE.to_string());
    guard.insert(PORT.to_string(), DEFAULT_PORT.to_string());
    guard.insert(TIMEOUT.to_string(), DEFAULT_TIMEOUT.to_string());
    guard.insert(DBFILENAME.to_string(), DEFAULT_DBFILENAME.to_string());
    guard.insert(LOGFILE.to_string(), DEFAULT_LOGFILE.to_string());

    drop(guard);

    d
}

#[cfg(test)]
mod config_parser_tests {
    use super::*;
    const FILE: &str = "redis.conf";
    const ADDR_VALUE: &str = "0.0.0.0:8888";
    const DEFAULT_VERBOSE_TO_BOOLEAN: bool = false;

    fn create_config_parser() -> ServerConf {
        ServerConf::new(FILE).unwrap()
    }

    #[test]
    #[should_panic]
    fn fails_open_correctly() {
        ServerConf::new("reds.conf").unwrap();
    }

    mod get_tests {
        use super::*;

        #[test]
        fn file_save_correctly() {
            let cp = create_config_parser();

            assert_eq!(cp.verbose(), DEFAULT_VERBOSE_TO_BOOLEAN);
            assert_eq!(cp.addr(), ADDR_VALUE);
            assert_eq!(cp.time_out(), DEFAULT_TIMEOUT);
            assert_eq!(cp.dbfilename(), DEFAULT_DBFILENAME);
            assert_eq!(cp.logfile(), DEFAULT_LOGFILE);
        }
    }

    mod get_config_tests {
        use super::*;

        #[test]
        fn get_all_config() {
            let cp = create_config_parser();
            if let Ok(SuccessServerRequest::List(list)) = cp.get_config("*") {
                let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();

                assert!(list.contains(&format!("{}: {}", VERBOSE, DEFAULT_VERBOSE)));
                assert!(list.contains(&format!("{}: {}", PORT, DEFAULT_PORT)));
                assert!(list.contains(&format!("{}: {}", TIMEOUT, DEFAULT_TIMEOUT)));
                assert!(list.contains(&format!("{}: {}", DBFILENAME, DEFAULT_DBFILENAME)));
                assert!(list.contains(&format!("{}: {}", LOGFILE, DEFAULT_LOGFILE)));
            }
        }

        #[test]
        fn get_config_verbose() {
            let cp = create_config_parser();
            if let Ok(SuccessServerRequest::List(list)) = cp.get_config(VERBOSE) {
                let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();

                assert!(list.contains(&format!("{}: {}", VERBOSE, DEFAULT_VERBOSE)));
            }
        }

        #[test]
        fn get_config_dbfilename() {
            let cp = create_config_parser();
            if let Ok(SuccessServerRequest::List(list)) = cp.get_config(DBFILENAME) {
                let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();

                assert!(list.contains(&format!("{}: {}", DBFILENAME, DEFAULT_DBFILENAME)));
            }
        }
    }

    mod set_config_tests {
        use super::*;

        #[test]
        fn set_verbose_correctly() {
            let mut cp = create_config_parser();
            assert_eq!(cp.verbose(), DEFAULT_VERBOSE_TO_BOOLEAN);

            let r = cp.set_config(VERBOSE, "1").unwrap();
            assert_eq!(r, SuccessServerRequest::Success);

            assert_eq!(cp.verbose(), true);
        }

        #[test]
        fn set_verbose_with_a_non_integer() {
            let mut cp = create_config_parser();
            assert_eq!(cp.verbose(), DEFAULT_VERBOSE_TO_BOOLEAN);

            let r = cp.set_config(VERBOSE, "non-number").unwrap_err();
            assert_eq!(r, ServerError::NotAnInteger);

            assert_eq!(cp.verbose(), DEFAULT_VERBOSE_TO_BOOLEAN);
        }

        #[test]
        fn set_non_existent_option() {
            let mut cp = create_config_parser();

            let r = cp.set_config("non-option", "no-value").unwrap_err();
            assert_eq!(r, ServerError::NonExistentConfigOption);

            assert_eq!(
                cp.get_config("non-option").unwrap_err(),
                ServerError::NonExistentConfigOption
            );
        }
    }
}
