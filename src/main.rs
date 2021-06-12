mod config_parser;
mod database;
mod databasehelper;
mod logger;
mod request;
mod server;
mod matcher;

use server::Server;
use std::env;

fn get_path() -> Result<String, String> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        return Err("Invalid number of arguments".to_string());
    }

    Ok(args.get(1).unwrap().to_string())
}

fn main() {
    let config_path = get_path().unwrap();
    let server = Server::new(&config_path).unwrap();
    server.run();
}
