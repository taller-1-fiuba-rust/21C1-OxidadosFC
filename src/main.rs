mod config_parser;
mod database;
mod database_errors;
mod logger;
mod redis;
mod request;
mod server;
mod storagevalue;

use redis::Redis;
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
    let redis = Redis::new(&config_path).unwrap();
    redis.run();
}
