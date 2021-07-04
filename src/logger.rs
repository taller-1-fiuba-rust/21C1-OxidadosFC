use std::fs::File;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

pub struct Logger {
    file_path: String,
}

impl Logger {
    pub fn new(file_path: &str) -> Logger {
        let file_path = file_path.to_string();
        Logger { file_path }
    }

    pub fn run(&mut self) -> Sender<String> {
        let (log_sender, log_rec): (Sender<String>, Receiver<String>) = mpsc::channel();
        let path = self.file_path.clone();

        thread::spawn(move || {
            let mut logger = open_logger(&path).unwrap();

            for msg in log_rec.iter() {
                if let Err(e) = writeln!(logger, "{}", &msg) {
                    eprintln!("Couldn't write: {}", e);
                }
            }
        });

        log_sender
    }
}

fn open_logger(path: &str) -> Result<File, String> {
    match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(path)
    {
        Err(why) => Err(format!("Couldn't open file: {}", why)),
        Ok(file) => Ok(file),
    }
}

#[cfg(test)]
mod logger_test {
    use super::*;
    use std::fs;

    use std::io::Read;
    use std::{thread, time};

    const MSGA: &str = "MessageA";
    const MSGB: &str = "MessageB";

    #[test]
    fn test_logger_recive_message() {
        let mut logger = Logger::new("log_testA.log");
        let sen = logger.run();

        sen.send("Message".to_owned()).unwrap();

        thread::sleep(time::Duration::from_millis(10));

        let mut log_file = open_logger("log_testA.log").unwrap();
        let mut data = String::new();

        log_file
            .read_to_string(&mut data)
            .expect("Unable to read string");

        assert_eq!("Message\n", data);

        drop(log_file);
        fs::remove_file("log_testA.log").unwrap();
    }

    #[test]
    fn test_logger_recive_two_message() {
        let mut logger = Logger::new("log_testB.log");

        let sen = logger.run();

        sen.send(MSGA.to_owned()).unwrap();
        sen.send(MSGB.to_owned()).unwrap();

        thread::sleep(time::Duration::from_millis(10));

        let mut log_file = open_logger("log_testB.log").unwrap();
        let mut data = String::new();

        log_file
            .read_to_string(&mut data)
            .expect("Unable to read string");

        let data = data.split('\n').collect::<Vec<&str>>();

        assert!(data.contains(&MSGA));
        assert!(data.contains(&MSGB));

        drop(log_file);
        fs::remove_file("log_testB.log").unwrap();
    }

    #[test]
    fn test_logger_recive_message_from_two_senders() {
        let mut logger = Logger::new("log_testC.log");
        let sen = logger.run();

        let sen1 = sen.clone();
        let sen2 = sen.clone();

        thread::spawn(move || {
            logger.run();
        });

        sen1.send(MSGA.to_owned()).unwrap();
        sen2.send(MSGB.to_owned()).unwrap();

        thread::sleep(time::Duration::from_millis(10));

        let mut log_file = open_logger("log_testC.log").unwrap();
        let mut data = String::new();

        log_file
            .read_to_string(&mut data)
            .expect("Unable to read string");

        let data = data.split('\n').collect::<Vec<&str>>();

        assert!(data.contains(&MSGA));
        assert!(data.contains(&MSGB));

        drop(log_file);
        drop(sen);
        fs::remove_file("log_testC.log").unwrap();
    }
}
