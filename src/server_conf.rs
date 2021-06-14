use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;

use crate::databasehelper::DataBaseError;
use crate::databasehelper::SuccessQuery;
use crate::matcher::matcher;

const VERBOSE: &str = "verbose";
const PORT: &str = "port";
const TIMEOUT: &str = "timeout";
const DBFILENAME: &str = "dbfilename";
const LOGFILE: &str = "logfile";
const DEFAULT_VERBOSE: u64 = 0;
const DEFAULT_PORT: &str = "8888";
const DEFAULT_TIMEOUT: u64 = 0;
const DEFAULT_DBFILENAME: &str = "dump.rdb";
const DEFAULT_LOGFILE: &str = "lf.log";
const NUMERIC_KEYS: [&str; 3] = [VERBOSE, PORT, TIMEOUT];

pub struct ServerConf {
    conf: HashMap<String, String>,
}

fn default_values() -> HashMap<String, String> {
    let mut d: HashMap<String, String> = HashMap::new();
    // Inserting default values
    d.insert(VERBOSE.to_string(), DEFAULT_VERBOSE.to_string());
    d.insert(PORT.to_string(), DEFAULT_PORT.to_string());
    d.insert(TIMEOUT.to_string(), DEFAULT_TIMEOUT.to_string());
    d.insert(DBFILENAME.to_string(), DEFAULT_DBFILENAME.to_string());
    d.insert(LOGFILE.to_string(), DEFAULT_LOGFILE.to_string());

    return d
}

impl ServerConf {
    pub fn new(config_file: &str) -> Result<ServerConf, String> {
        let mut conf = default_values();
        let file = match File::open(config_file) {
            Ok(f) => f,
            _ => return Err("Problem open file".to_string()),
        };

        let file = BufReader::new(file);

        for line in file.lines() {
            let line = line.unwrap();
            let element: Vec<String> = line.split('=').map(|s| s.trim().to_string()).collect();

            match &element[..] {
                [key, value] => {
                    conf.insert(key.to_string(), value.to_string());
                }
                _ => return Err("Invalid format config file".to_string()),
            }
        }

        Ok(ServerConf { conf })
    }

    // pub fn verbose(&self) -> u64 {
    //     if let Some(value) = self.conf.get(VERBOSE) {
    //         if let Ok(v) = value.parse::<u64>() {
    //             return v;
    //         }
    //     }

    //     DEFAULT_VERBOSE
    // }

    pub fn addr(&self) -> String {
        if let Some(value) = self.conf.get(PORT) {
            if let Ok(v) = value.parse::<u64>() {
                return "0.0.0.0:".to_owned() + &v.to_string();
            }
        }

        "0.0.0.0:".to_owned() + DEFAULT_PORT
    }

    // pub fn timeout(&self) -> u64 {
    //     if let Some(value) = self.conf.get(TIMEOUT) {
    //         if let Ok(v) = value.parse::<u64>() {
    //             return v;
    //         }
    //     }

    //     DEFAULT_TIMEOUT
    // }

    // pub fn dbfilename(&self) -> String {
    //     match self.conf.get(DBFILENAME) {
    //         Some(value) => value.to_string(),
    //         None => DEFAULT_DBFILENAME.to_string(),
    //     }
    // }

    pub fn logfile(&self) -> String {
        match self.conf.get(LOGFILE) {
            Some(value) => value.to_string(),
            None => DEFAULT_LOGFILE.to_string(),
        }
    }

    pub fn get_config(&self, pattern: &str) -> Result<SuccessQuery, DataBaseError> {
        let mut list = Vec::new();
        for (k, v) in &self.conf {
            if !matcher(k, pattern) {
                continue;
            }

            list.push(SuccessQuery::String(format!("{}: {}", k, v)));
        }

        if list.is_empty() {
            return Err(DataBaseError::NonExistentConfigOption);
        }

        Ok(SuccessQuery::List(list))
    }

    pub fn set_config(
        &mut self,
        option: &str,
        new_value: &str,
    ) -> Result<SuccessQuery, DataBaseError> {
        if NUMERIC_KEYS.contains(&option) && new_value.parse::<i64>().is_err() {
            return Err(DataBaseError::NotAnInteger);
        }

        if self.conf.contains_key(option) {
            self.conf.insert(option.to_string(), new_value.to_string());
            return Ok(SuccessQuery::Success);
        }

        Err(DataBaseError::NonExistentConfigOption)
    }
}

#[cfg(test)]
mod config_parser_tests {
    use super::*;
    const FILE: &str = "redis.conf";
    const ADDR_VALUE: &str = "0.0.0.0:8888";

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

            // assert_eq!(cp.verbose(), DEFAULT_VERBOSE);
            assert_eq!(cp.addr(), ADDR_VALUE);
            // assert_eq!(cp.timeout(), DEFAULT_TIMEOUT);
            // assert_eq!(cp.dbfilename(), DEFAULT_DBFILENAME);
            assert_eq!(cp.logfile(), DEFAULT_LOGFILE);
        }
    }

    mod get_config_tests {
        use super::*;

        #[test]
        fn get_all_config() {
            let cp = create_config_parser();
            if let Ok(SuccessQuery::List(list)) = cp.get_config("*") {
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
            if let Ok(SuccessQuery::List(list)) = cp.get_config(VERBOSE) {
                let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();

                assert!(list.contains(&format!("{}: {}", VERBOSE, DEFAULT_VERBOSE)));
            }
        }

        #[test]
        fn get_config_dbfilename() {
            let cp = create_config_parser();
            if let Ok(SuccessQuery::List(list)) = cp.get_config(DBFILENAME) {
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
            // assert_eq!(cp.verbose(), DEFAULT_VERBOSE);

            let r = cp.set_config(VERBOSE, "1").unwrap();
            assert_eq!(r, SuccessQuery::Success);

            // assert_eq!(cp.verbose(), 1);
        }

        #[test]
        fn set_verbose_with_a_non_integer() {
            let mut cp = create_config_parser();
            // assert_eq!(cp.verbose(), DEFAULT_VERBOSE);

            let r = cp.set_config(VERBOSE, "non-number").unwrap_err();
            assert_eq!(r, DataBaseError::NotAnInteger);

            // assert_eq!(cp.verbose(), DEFAULT_VERBOSE);
        }

        #[test]
        fn set_port_correctly() {
            let mut cp = create_config_parser();
            assert_eq!(cp.addr(), ADDR_VALUE);

            let r = cp.set_config(PORT, "6379").unwrap();
            assert_eq!(r, SuccessQuery::Success);

            assert_eq!(cp.addr(), "0.0.0.0:6379");
        }

        #[test]
        fn set_non_existent_option() {
            let mut cp = create_config_parser();

            let r = cp.set_config("non-option", "no-value").unwrap_err();
            assert_eq!(r, DataBaseError::NonExistentConfigOption);

            assert_eq!(
                cp.get_config("non-option").unwrap_err(),
                DataBaseError::NonExistentConfigOption
            );
        }
    }
}
