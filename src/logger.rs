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
            logger.write_all(recived.as_bytes());
            logger.write_all("\n".as_bytes());
       
       
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
        Err(why) => Err(format!("No se pudo abrir: {}", why)),
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
        let path = Path::new("log.txt");
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

        assert_eq!("Message", data);

        drop(log_file);
        fs::remove_file(path).unwrap();
    }
}