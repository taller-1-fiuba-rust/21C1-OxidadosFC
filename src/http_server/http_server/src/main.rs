use std::fs;
use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;
use std::thread;
fn main() {
    let listener = TcpListener::bind("127.0.0.1:8080").unwrap();

    for stream in listener.incoming() {
        match stream {
            Err(e) => eprintln!("failed: {}", e),
            Ok(stream) => {
                thread::spawn(move || {
                    handle_connection(stream);
                });
            }
        }
    }
}

fn handle_connection(mut stream: TcpStream) {
    let mut redis_stream = TcpStream::connect("127.0.0.1:8888").unwrap();

    loop {
        let mut buffer = [0; 1024];

        let bytes_read = stream.read(&mut buffer).unwrap();
        let request = std::str::from_utf8(&buffer[..bytes_read]).unwrap();
        let post = b"POST / HTTP/1.1\r\n";
        let (status_line, filename) = if buffer.starts_with(post) {
            let command = get_command(request);
            let response = handle_redis_connection(&redis_stream, command);
        
            ("HTTP/1.1 200 OK\r\n\r\n", "src/home.html")
        } else {
            ("HTTP/1.1 404 NOT FOUND\r\n\r\n", "src/home.html")
        };
        let contents = fs::read_to_string(filename).unwrap();
        let response = format!("{}{}", status_line, contents);
        stream.write(response.as_bytes()).unwrap();
        stream.flush().unwrap();
    }
}

fn get_command(request: &str) -> String {
    let request: Vec<&str> = request.split_whitespace().collect();
    let body_pos = request.iter().position(|x| x.contains("to")).unwrap();
    let body = request[body_pos];
    let command = body[3..].replace('+', " ");
    command
}

fn handle_redis_connection(mut stream: &TcpStream, command: String) -> String{
    stream.write(command.as_bytes()).unwrap();
    let mut buffer_respond = [0; 1024];
    let bytes_read = stream.read(&mut buffer_respond).unwrap();
    let response = std::str::from_utf8(&buffer_respond[..bytes_read]).unwrap();
    response.to_string()
}
