mod commands;
mod database;
mod server;
mod storagevalue;
mod redis;
mod config_parser;

use std::env;
use redis::Redis;

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
