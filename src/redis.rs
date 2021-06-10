use crate::{config_parser::ConfigParser, server::Server};

pub struct Redis {
    server: Server,
}

impl Redis {
    pub fn new(config_file: &str) -> Result<Redis, String> {
        let cp = ConfigParser::new(config_file)?;

        let port = cp.getu32("port")?;
        let addr = "0.0.0.0:".to_owned() + &port.to_string();
        let lf = cp.get("logfile")?;
        let server = Server::new(&addr);

        Ok(Redis { server })
    }

    pub fn run(self) {
        self.server.run();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[should_panic]
    fn fails_open_correctly() {
        Redis::new("reds.conf").unwrap();
    }
}
