use std::fs;
use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread;

const COMMANDS_ALOWED: [&str; 40] = [
    // server (monitor, config, info not alowed)
    "flushdb",
    "dbsize",
    // keys
    "copy",
    "del",
    "exists",
    "expire",
    "expireat",
    "keys",
    "persist",
    "rename",
    "sort",
    "touch",
    "ttl",
    "type",
    // strings
    "append",
    "decrby",
    "get",
    "getdel",
    "getset",
    "incrby",
    "mget",
    "mset",
    "set",
    "strlen",
    // lists
    "lindex",
    "llen",
    "lpop",
    "lpush",
    "lpushx",
    "lrange",
    "lrem",
    "lset",
    "rpop",
    "rpush",
    "rpushx",
    // sets
    "sadd",
    "scard",
    "sismember",
    "smembers",
    "srem",
    // pubsub commands not alowed
];

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

    let css_contents = fs::read_to_string("src/css/style.css").unwrap();
    let css = build_css(css_contents);

    let (status_line, contents) = if buffer.starts_with(b"POST / HTTP/1.1\r\n") {
        let command = get_command(request);
        let response = handle_redis_connection(&redis_stream, &command);

        let mut records_guard = records.lock().unwrap();

        records_guard.push((command, response));

        let contents = fs::read_to_string("src/index.html").unwrap();
        let answer = build_answer(&records_guard);
        let contents = contents.replace(r#"<div id="answer"></div>"#, &answer);
        let contents = contents.replace(r#"<style type="text/css"></style>"#, &css);

        ("HTTP/1.1 201 OK\r\n\r\n", contents)
    } else {
        let contents = fs::read_to_string("src/index.html").unwrap();
        let contents = contents.replace(r#"<style type="text/css"></style>"#, &css);
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
    let c: Vec<&str> = command.split_whitespace().collect();
    if let Some(command_name) = c.get(0) {
        if !COMMANDS_ALOWED.contains(command_name) {
            return build_non_existent_command_response(command);
        }
    } else {
        return build_non_existent_command_response(command);
    }

    stream.write_all(command.as_bytes()).unwrap();
    stream.flush().unwrap();
    secure_read(&stream)
}

fn build_non_existent_command_response(command: &str) -> String {
    format!("Error: Non existent Request On: {}", command)
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
    let input = r#"class="input""#;
    let response = r#"class="response""#;
    let prompt = r#"class="prompt""#;
    let nopad = r#"class="nopad""#;
    let mut list_elements: Vec<String> = records
        .iter()
        .map(|(req, res)| format!("<div {}><div {}><span {}>> </span>{}</div></div><div {}><div {}><span {}></span>{}</div></div>", input, nopad, prompt, req, response, nopad, prompt, res))
        .collect();

    list_elements.insert(0, "</div>".to_string());
    list_elements.insert(0, r#"<div id="answer">"#.to_string());

    list_elements.join("")
}

fn build_css(css_contents: String) -> String {
    let mut head = String::from(r#"<style type="text/css">"#);
    head.push_str(&css_contents);
    head.push_str("</style>");

    head
}
