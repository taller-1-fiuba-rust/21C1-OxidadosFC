use std::fs::File;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::mpsc::Receiver;

pub struct Logger<'a> {
    file_path: &'a Path,
    reciver: Receiver<String>,
}

impl<'a> Logger<'a> {
    pub fn new(file_path: &Path, reciver: Receiver<String>) -> Logger {
        Logger { file_path, reciver }
    }

    pub fn run(&mut self) {
        let mut logger = open_logger(self.file_path).unwrap();

        for recived in self.reciver.iter() {
            if let Err(e) = writeln!(logger, "{}", &recived) {
                eprintln!("No se pudo escribir al : {}", e);
            }
        }
    }
}

fn open_logger(path: &Path) -> Result<File, String> {
    match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(path)
    {
        Err(why) => Err(format!("No se pudo abrir el archivo: {}", why)),
        Ok(file) => Ok(file),
    }
}

#[cfg(test)]
mod logger_test {
    use super::*;
    use std::fs;
    use std::io::Read;
    use std::sync::mpsc;
    use std::{thread, time};

    #[test]
    fn test_logger_recive_message() {
        let (sen, rec) = mpsc::channel();
        let path = Path::new("log_testA.txt");
        let mut logger = Logger::new(path, rec);

        thread::spawn(move || {
            logger.run();
        });

        sen.send("Message".to_owned()).unwrap();

        thread::sleep(time::Duration::from_millis(10));

        let mut log_file = open_logger(path).unwrap();
        let mut data = String::new();

        log_file
            .read_to_string(&mut data)
            .expect("Unable to read string");

        assert_eq!("Message\n", data);

        drop(log_file);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_logger_recive_two_message() {
        let (sen, rec) = mpsc::channel();
        let path = Path::new("log_testB.txt");
        let mut logger = Logger::new(path, rec);
        thread::spawn(move || {
            logger.run();
        });

        sen.send("MessageA".to_owned()).unwrap();
        sen.send("MessageB".to_owned()).unwrap();

        thread::sleep(time::Duration::from_millis(10));

        let mut log_file = open_logger(path).unwrap();
        let mut data = String::new();

        log_file
            .read_to_string(&mut data)
            .expect("Unable to read string");

        let data = data.split('\n').collect::<Vec<&str>>();

        assert!(data.contains(&"MessageA"));
        assert!(data.contains(&"MessageB"));

        drop(log_file);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_logger_recive_message_from_two_senders() {
        let (sen, rec) = mpsc::channel();
        let path = Path::new("log_testC.txt");
        let mut logger = Logger::new(path, rec);

        let sen1 = sen.clone();
        let sen2 = sen.clone();

        thread::spawn(move || {
            logger.run();
        });

        sen1.send("MessageA".to_owned()).unwrap();
        sen2.send("MessageB".to_owned()).unwrap();

        thread::sleep(time::Duration::from_millis(10));

        let mut log_file = open_logger(path).unwrap();
        let mut data = String::new();

        log_file
            .read_to_string(&mut data)
            .expect("Unable to read string");

        let data = data.split('\n').collect::<Vec<&str>>();

        assert!(data.contains(&"MessageA"));
        assert!(data.contains(&"MessageB"));

        drop(log_file);
        drop(sen);
        fs::remove_file(path).unwrap();
    }
}
