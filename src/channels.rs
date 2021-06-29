use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

use crate::matcher::matcher;

pub const MONITOR: &str = "Monitor";
pub const MONITOR_ID: u32 = 0;

type Dictionary = Arc<Mutex<HashMap<String, Vec<(u32, Sender<String>)>>>>;
type SendersLog = Arc<Mutex<Vec<Sender<(String, bool)>>>>;

pub struct Channels {
    channels: Dictionary,
    log_chanel: SendersLog
}

impl<'a> Clone for Channels {
    fn clone(&self) -> Self {
        Channels::new_from_channels(self.channels.clone(), self.log_chanel.clone())
    }
}

impl Channels {
    fn new_from_channels(channels: Dictionary, log_chanel: SendersLog) -> Self {
        Channels { channels, log_chanel}
    }

    pub fn new() -> Channels {
        let mut hash = HashMap::new();
        hash.insert(MONITOR.to_string(), Vec::new());
        let channels = Arc::new(Mutex::new(hash));
        let log_chanel = Arc::new(Mutex::new(Vec::new()));
        Channels { channels, log_chanel }
    }

    pub fn add_to_channel(&mut self, channel: &str, sender: Sender<String>, id: u32) {
        let mut guard = self.channels.lock().unwrap();
        match guard.get_mut(channel) {
            Some(l) => l.push((id, sender)),
            None => {
                guard.insert(channel.to_string(), vec![(id, sender)]);
            }
        }
    }

    pub fn add_logger(&mut self, logger_sender: Sender<(String, bool)>) {
        let mut guard = self.log_chanel.lock().unwrap();
        guard.push(logger_sender)
    }

    pub fn unsubscribe(&mut self, channel: &str, id: u32) {
        let mut guard = self.channels.lock().unwrap();
        if let Some(l) = guard.get_mut(channel) {
            l.retain(|x| x.0 != id);
            if l.is_empty() {
                guard.remove(channel);
            }
        }
    }

    pub fn send(&mut self, channel: &str, msg: &str) -> i32 {
        let guard = self.channels.lock().unwrap();
        match guard.get(channel) {
            Some(listeners) => {
                listeners.iter().for_each(|x| {
                    x.1.send(msg.to_string()).unwrap();
                });
                listeners.len() as i32
            }
            None => 0,
        }
    }

    pub fn send_logger(&mut self, id: u32, msg: &str, verbose: bool) {
        let msg: String = id.to_string() + " " + " " + msg;
        let msg = (msg, verbose);
        let guard = self.log_chanel.lock().unwrap();
        guard.iter().for_each(|x| {
            x.send(msg.clone()).unwrap();
        });
    }

    pub fn send_monitor(&mut self, id: u32, msg: &str) {
        let msg: String = "Client: ".to_string() + &id.to_string() + " " + msg;
        self.send(MONITOR, &msg);
    }

    pub fn add_monitor(&mut self) -> Receiver<String> {
        let (s, r) = channel();
        let mut guard = self.channels.lock().unwrap();

        let list = guard.get_mut(MONITOR).unwrap();
        list.push((MONITOR_ID, s));

        r
    }

    pub fn get_channels(&self, pattern: &str) -> Vec<String> {
        let guard = self.channels.lock().unwrap();
        guard
            .keys()
            .filter(|x| matcher(x, pattern) && *x != MONITOR)
            .map(|item| item.to_string())
            .collect()
    }

    pub fn subcriptors_number(&self, channel: &str) -> usize {
        let guard = self.channels.lock().unwrap();
        match guard.get(channel) {
            Some(l) => l.len(),
            None => 0,
        }
    }
}
