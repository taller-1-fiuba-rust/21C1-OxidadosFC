use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;

use crate::databasehelper::DataBaseError;
use crate::databasehelper::SuccessQuery;
use crate::matcher::matcher;

pub struct ConfigParser {
    data: HashMap<String, String>,
}

impl ConfigParser {
    pub fn new(config_file: &str) -> Result<ConfigParser, String> {
        let mut d: HashMap<String, String> = HashMap::new();
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
                    d.insert(key.to_string(), value.to_string());
                }
                _ => return Err("Invalid format config file".to_string()),
            }
        }

        Ok(ConfigParser { data: d })
    }

    pub fn get(&self, k: &str) -> Result<String, String> {
        match self.data.get(k) {
            Some(value) => Ok(value.to_string()),
            None => return Err(format!("There's no {} field", k))
        }
    }

    pub fn getu32(&self, k: &str) -> Result<u32, String> {
        match self.data.get(k) {
            Some(value) => match value.parse::<u32>() {
                Ok(v) => Ok(v),
                _ => Err(format!("Invalid value for {}", k)),
            },
            None => Err(format!("There's no {} field", k)),
        }
    }

    pub fn get_config(&self, pattern: &str) -> Result<SuccessQuery, DataBaseError> {
        let mut list = Vec::new();
        for (k, v) in &self.data {
            if !matcher(k, pattern) {
                continue;
            }

            list.push(
                SuccessQuery::String(
                    format!("{}: {}", k, v)
                )
            );
        }

        if list.is_empty() {
            return Err(DataBaseError::NonExistentConfigOption);
        }

        Ok(SuccessQuery::List(list))
    }

    pub fn set_config(&mut self, option: &str, new_value: &str) -> Result<SuccessQuery, DataBaseError> {
        if self.data.contains_key(option) {
            self.data.insert(option.to_string(), new_value.to_string());
            return Ok(SuccessQuery::Success);
        }

        Err(DataBaseError::NonExistentConfigOption)
    }
}

#[cfg(test)]
mod config_parser_tests {
    use super::*;
    const FILE: &str = "redis.conf";
    const VERBOSE: &str = "verbose";
    const PORT: &str = "port";
    const TIMEOUT: &str = "timeout";
    const DBFILENAME: &str = "dbfilename";
    const LOGFILE: &str = "logfile";
    const VERBOSE_VALUE: &str = "0";
    const PORT_VALUE: &str = "8888";
    const TIMEOUT_VALUE: &str = "0";
    const DBFILENAME_VALUE: &str = "dump.rdb";
    const LOGFILE_VALUE: &str = "lf.log";

    fn create_config_parser() -> ConfigParser {
        ConfigParser::new(FILE).unwrap()
    }

    #[test]
    #[should_panic]
    fn fails_open_correctly() {
        ConfigParser::new("reds.conf").unwrap();
    }

    #[test]
    #[should_panic]
    fn getu32_dbfilename_panic() {
        let cp = ConfigParser::new(FILE).unwrap();

        // should panic here
        cp.getu32(DBFILENAME).unwrap();
    }

    mod get_tests {
        use super::*;

        #[test]
        fn verbose_save_correctly() {
            let cp = create_config_parser();
    
            assert_eq!(cp.get(VERBOSE).unwrap(), VERBOSE_VALUE);
        }
    
        #[test]
        fn port_save_correctly() {
            let cp = create_config_parser();
    
            assert_eq!(cp.get(PORT).unwrap(), PORT_VALUE);
        }
    
        #[test]
        fn getu32_port_8888() {
            let cp = create_config_parser();
    
            assert_eq!(cp.getu32(PORT).unwrap(), PORT_VALUE.parse().unwrap());
        }
        
        #[test]
        fn timeout_save_correctly() {
            let cp = create_config_parser();
    
            assert_eq!(cp.get(TIMEOUT).unwrap(), TIMEOUT_VALUE);
        }
    
        #[test]
        fn dbfilename_save_correctly() {
            let cp = create_config_parser();
            
            assert_eq!(cp.get(DBFILENAME).unwrap(), DBFILENAME_VALUE);
        }

        #[test]
        fn logfile_save_correctly() {
            let cp = create_config_parser();
    
            assert_eq!(cp.get(LOGFILE).unwrap(), LOGFILE_VALUE);
        }
    }
    
    mod get_config_tests{
        use super::*;

        #[test]
        fn get_all_config() {
            let cp = create_config_parser();
            if let Ok(SuccessQuery::List(list)) = cp.get_config("*") {
                let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();
    
                assert!(list.contains(&format!("{}: {}", VERBOSE, VERBOSE_VALUE)));
                assert!(list.contains(&format!("{}: {}", PORT, PORT_VALUE)));
                assert!(list.contains(&format!("{}: {}", TIMEOUT, TIMEOUT_VALUE)));
                assert!(list.contains(&format!("{}: {}", DBFILENAME, DBFILENAME_VALUE)));
                assert!(list.contains(&format!("{}: {}", LOGFILE, LOGFILE_VALUE)));
            }
        }

        #[test]
        fn get_verbose_config() {
            let cp = create_config_parser();
            if let Ok(SuccessQuery::List(list)) = cp.get_config(VERBOSE) {
                let list: Vec<String> = list.iter().map(|x| x.to_string()).collect();
    
                assert!(list.contains(&format!("{}: {}", VERBOSE, VERBOSE_VALUE)));
            }
        }
    }

    mod set_config_tests {
        use super::*;

        #[test]
        fn set_verbose_correctly() {
            let mut cp = create_config_parser();
            assert_eq!(cp.get(VERBOSE).unwrap(), "0");

            let r = cp.set_config(VERBOSE, "1").unwrap();
            assert_eq!(r, SuccessQuery::Success);
            
            assert_eq!(cp.get(VERBOSE).unwrap(), "1");
        }

        #[test]
        fn set_non_existent_option() {
            let mut cp = create_config_parser();

            let r = cp.set_config("non-option", "no-value").unwrap_err();
            assert_eq!(r, DataBaseError::NonExistentConfigOption);
            
            assert_eq!(cp.get_config("non-option").unwrap_err(), DataBaseError::NonExistentConfigOption);
        }
    }
}
