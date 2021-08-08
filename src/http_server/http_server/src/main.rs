use std::fs;
use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:8080").unwrap();
    let redis_stream = TcpStream::connect("127.0.0.1:8888").unwrap();

    let records: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));

    for stream in listener.incoming() {
        match stream {
            Err(e) => eprintln!("failed: {}", e),
            Ok(stream) => {
                let new_records = records.clone();
                let redis_stream = redis_stream.try_clone().unwrap();
                thread::spawn(move || {
                    handle_connection(stream, new_records, redis_stream);
                });
            }
        }
    }
}

fn handle_connection(
    mut stream: TcpStream,
    records: Arc<Mutex<Vec<(String, String)>>>,
    redis_stream: TcpStream,
) {
    let mut buffer = [0; 2048];

    let bytes_read = stream.read(&mut buffer).unwrap();

    let request = std::str::from_utf8(&buffer[..bytes_read]).unwrap();

    let (status_line, contents) = if buffer.starts_with(b"POST / HTTP/1.1\r\n") {
        let command = get_command(request);
        let response = handle_redis_connection(&redis_stream, &command);

        let mut records_guard = records.lock().unwrap();

        records_guard.push((command, response));

        let contents = fs::read_to_string("src/index.html").unwrap();
        let answer = build_answer(&records_guard);
        let contents = contents.replace(r#"<div id="answer"></div>"#, &answer);

        ("HTTP/1.1 201 OK\r\n\r\n", contents)
    } else {
        let contents = fs::read_to_string("src/index.html").unwrap();

        ("HTTP/1.1 200 OK\r\n\r\n", contents)
    };

    let response = format!("{}{}", status_line, contents);

    stream.write_all(response.as_bytes()).unwrap();
    stream.flush().unwrap();
}

fn get_command(request: &str) -> String {
    let request: Vec<&str> = request.split_whitespace().collect();
    let body_pos = request.iter().position(|x| x.contains("to")).unwrap();
    let body = request[body_pos];
    body[3..].replace('+', " ")
}

fn handle_redis_connection(mut stream: &TcpStream, command: &str) -> String {
    stream.write_all(command.as_bytes()).unwrap();
    stream.flush().unwrap();
    secure_read(&stream)
}

fn secure_read(mut stream: &TcpStream) -> String {
    let mut buffer_respond = [0; 1024];
    let mut response = String::new();
    while !response.ends_with('\n') {
        let bytes_read = stream.read(&mut buffer_respond).unwrap();
        let r = std::str::from_utf8(&buffer_respond[..bytes_read]).unwrap();
        response.push_str(r);
    }

    response
}

fn build_answer(records: &[(String, String)]) -> String {
    let mut list_elements: Vec<String> = records
        .iter()
        .map(|(req, res)| format!("<li>Request: {} Reponse: {}</li>", req, res))
        .collect();

    list_elements.insert(0, "<ol>".to_string());
    list_elements.push("</ol>".to_string());

    list_elements.insert(0, r#"<div id="answer">"#.to_string());
    list_elements.insert(0, "</div>".to_string());

    list_elements.join("")
}
