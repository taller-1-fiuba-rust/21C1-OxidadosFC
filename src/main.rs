mod commands;
mod database;
mod server;
mod storagevalue;

use server::Server;

fn main() {
    let server = Server::new("0.0.0.0:8888");
    server.run();
}
