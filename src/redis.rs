use crate::{config_parser::ConfigParser, server::Server};

pub struct Redis {
    verbose: bool,
    timeout: u32,
    dbfilename: String,
    logfile: String,
    server: Server
}

impl Redis {
    pub fn new(config_file: &str) -> Result<Redis, String> {
        let cp = ConfigParser::new(config_file)?;
        let verbose = match cp.get("verbose") {
            Some(value) => match value.parse::<u8>() {
                Ok(v) => v != 0,
                _ => return Err("Invalid value for verbose".to_string())
            },
            None => return Err("There's no verbose field".to_string())
        };

        let port = match cp.get("port") {
            Some(value) => match value.parse::<u32>() {
                Ok(v) => v,
                _ => return Err("Invalid value for port".to_string())
            },
            None => return Err("There's no port field".to_string())
        };

        let timeout = match cp.get("timeout") {
            Some(value) => match value.parse::<u32>() {
                Ok(v) => v,
                _ => return Err("Invalid value for timeout".to_string())
            },
            None => return Err("There's no port timeout".to_string())
        };

        let dbfilename = match cp.get("dbfilename") {
            Some(value) => value.to_string(),
            None => return Err("There's no port dbfilename".to_string())
        };

        let logfile = match cp.get("logfile") {
            Some(value) => value.to_string(),
            None => return Err("There's no port logfile".to_string())
        };

        let addr = "0.0.0.0:".to_owned() + &port.to_string();
        let server = Server::new(&addr);
        Ok(Redis {verbose, timeout, dbfilename, logfile, server})
    }

    pub fn run(self) {
        self.server.run();
    }
    
}

#[test]
fn verbose_get_correctly() {
    let redis = Redis::new("redis.conf").unwrap();

    assert_eq!(redis.verbose, true);
}

#[test]
fn timeout_get_correctly() {
    let redis = Redis::new("redis.conf").unwrap();

    assert_eq!(redis.timeout, 0);
}

#[test]
fn dbfilename_get_correctly() {
    let redis = Redis::new("redis.conf").unwrap();

    assert_eq!(redis.dbfilename, "dump.rdb");
}

#[test]
fn logfile_get_correctly() {
    let redis = Redis::new("redis.conf").unwrap();

    assert_eq!(redis.logfile, "lf.log");
}

#[test]
#[should_panic]
fn fails_open_correctly() {
    Redis::new("reds.conf").unwrap();
}