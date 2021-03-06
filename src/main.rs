mod channels;
mod client;
mod database;
mod databasehelper;
mod hash_shard;
mod logger;
mod matcher;
mod request;
mod server;
mod server_conf;

use server::Server;
use std::env;

#[doc(hidden)]
fn get_path() -> Result<String, String> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        return Err("Invalid number of arguments".to_string());
    }

    Ok(args.get(1).unwrap().to_string())
}

#[doc(hidden)]
fn main() {
    let config_path = get_path().unwrap();
    let server = Server::new(&config_path).unwrap();
    server.run();
}
