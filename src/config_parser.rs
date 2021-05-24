use std::collections::HashMap;
use std::io::BufReader;
use std::io::prelude::*;
use std::fs::File;

pub struct ConfigParser {
    data: HashMap<String, String>,
}

impl ConfigParser {
    pub fn new(config_file: &str) -> Result<ConfigParser, String> {
        let mut d: HashMap<String, String> = HashMap::new();
        let file = match File::open(config_file){
            Ok(f) => f,
            _ => return Err("Problem open file".to_string()),
        };
        let file = BufReader::new(file);
        
        for line in file.lines() {
            let line = line.unwrap();
            let element: Vec<String> = line.split("=").map(
                |s| s.trim().to_string()
            ).collect();

            match &element[..] {
                [key, value] => {
                    d.insert(key.to_string(), value.to_string());
                },
                _ => return Err("Invalid format config file".to_string()),
            }
        }

        Ok(ConfigParser {
            data: d,
        })
    }

    // Use for log_file and dbfilename
    // pub fn get(&self, k: &str) -> Option<&String> {
    //     self.data.get(k)
    // }

    pub fn getu32(&self, k: &str) -> Result<u32, String> {
        match self.data.get(k) {
            Some(value) => match value.parse::<u32>() {
                Ok(v) => Ok(v),
                _ => return Err("Invalid value for port".to_string())
            },
            None => return Err("There's no port field".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn verbose_save_correctly() {
        let cp = ConfigParser::new("redis.conf").unwrap();
    
        assert_eq!(cp.data.get("verbose").unwrap(), "1");
    }
    
    #[test]
    fn port_save_correctly() {
        let cp = ConfigParser::new("redis.conf").unwrap();
    
        assert_eq!(cp.data.get("port").unwrap(), "8888");
    }
    
    #[test]
    fn getu32_port_8888() {
        let cp = ConfigParser::new("redis.conf").unwrap();
    
        assert_eq!(cp.getu32("port").unwrap(), 8888);
    }
    
    #[test]
    fn timeout_save_correctly() {
        let cp = ConfigParser::new("redis.conf").unwrap();
    
        assert_eq!(cp.data.get("timeout").unwrap(), "0");
    }
    
    #[test]
    fn dbfilename_save_correctly() {
        let cp = ConfigParser::new("redis.conf").unwrap();
    
        assert_eq!(cp.data.get("dbfilename").unwrap(), "dump.rdb");
    }
    
    #[test]
    fn logfile_save_correctly() {
        let cp = ConfigParser::new("redis.conf").unwrap();
    
        assert_eq!(cp.data.get("logfile").unwrap(), "lf.log");
    }
    
    #[test]
    #[should_panic]
    fn fails_open_correctly() {
        ConfigParser::new("reds.conf").unwrap();
    }
    
    #[test]
    #[should_panic]
    fn getu32_dbfilename_panic() {
        let cp = ConfigParser::new("redis.conf").unwrap();
    
        // should panic here
        cp.getu32("dbfilename").unwrap();
    }
}