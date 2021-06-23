use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

pub const MONITOR: &str = "Monitor";
pub const LOGGER: &str = "Logger";

type Dictionary = Arc<Mutex<HashMap<String, Vec<(u32, Sender<String>)>>>>;

pub struct Channels {
    channels: Dictionary,
}

impl<'a> Clone for Channels {
    fn clone(&self) -> Self {
        Channels::new_from_channels(self.channels.clone())
    }
}

impl Channels {
    fn new_from_channels(channels: Dictionary) -> Self {
        Channels { channels }
    }

    pub fn new() -> Channels {
        let mut hash = HashMap::new();
        hash.insert(MONITOR.to_string(), Vec::new());
        let channels = Arc::new(Mutex::new(hash));
        Channels { channels }
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

    pub fn add_logger(&mut self, logger_sender: Sender<String>) {
        self.add_to_channel(LOGGER, logger_sender, 0);
    }

    pub fn unsubscribe(&mut self, channel: &str, id: u32) {
        let mut guard = self.channels.lock().unwrap();
        if let Some(l) = guard.get_mut(channel) {
            l.retain(|x| x.0 != id);
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

    pub fn send_logger(&mut self, id: u32, msg: &str) {
        let msg: String = id.to_string() + " " + " " + msg;
        self.send(LOGGER, &msg);
    }

    pub fn send_monitor(&mut self, id: u32, msg: &str) {
        let msg: String = "Client: ".to_string() + &id.to_string() + " " + msg;
        self.send(MONITOR, &msg);
    }

    pub fn add_monitor(&mut self) -> Receiver<String> {
        let (s, r) = channel();
        let mut guard = self.channels.lock().unwrap();

        let list = guard.get_mut(MONITOR).unwrap();
        list.push((0, s));

        r
    }
}
