use std::collections::HashMap;
use crate::databasehelper::SuccessQuery;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

pub const MONITOR: &'static str = "Monitor";
pub const LOGGER: &'static str = "Logger";

pub struct Channels {
    channels: Arc<Mutex<HashMap<String, Vec<Sender<String>>>>>
}

impl<'a> Clone for Channels {
    fn clone(&self) -> Self {
        Channels::new_from_channels(self.channels.clone())
    }
}

impl Channels {
    fn new_from_channels(channels: Arc<Mutex<HashMap<String, Vec<Sender<String>>>>>) -> Self {
        Channels { channels }
    }

    pub fn new() -> Channels {
        let mut hash = HashMap::new();
        hash.insert(MONITOR.to_string(), Vec::new());
        let channels = Arc::new(Mutex::new(hash));
        Channels {
            channels
        }
    }

    pub fn try_add_channel(&mut self, channel: &str, senders: Vec<Sender<String>>) {
        let mut guard = self.channels.lock().unwrap();
        if guard.contains_key(channel) {
            return
        }

        guard.insert(channel.to_string(), senders);
    }

    pub fn send(&mut self, channel: &str, msg: &str) -> i32 {
        let guard = self.channels.lock().unwrap();
        match guard.get(channel){
            Some(listeners) => {
                listeners.iter().for_each(|x| {
                    x.send(msg.to_string()).unwrap();
                });
                listeners.len() as i32
            }
            None => 0
        }
    }

    pub fn subscribe(&mut self, channels: Vec<&str>) -> (Receiver<String>, String) {
        let (s, r) = channel();
        let mut guard = self.channels.lock().unwrap();
        
        let mut subscriptions = String::new();
        let mut cont = 0;
        let mut suscriptions_added = Vec::new();
        for sus in channels {
            let list = match guard.get_mut(sus) {
                Some(l) => l,
                None => {
                    guard.insert(sus.to_string(), Vec::new());
                    guard.get_mut(sus).unwrap()
                },
            };

            if !suscriptions_added.contains(&sus) {
                list.push(s.clone());
                cont = cont + 1;
                suscriptions_added.push(sus);
            }
            
            let subscription = SuccessQuery::List(
                vec![
                    SuccessQuery::String("subscribe".to_string()),
                    SuccessQuery::String(sus.to_string()),
                    SuccessQuery::Integer(cont),
                ]
            ).to_string();

            subscriptions = format!("{}\n{}", subscriptions, subscription);
        }

        (r, subscriptions)
    }

    pub fn add_monitor(&mut self) -> Receiver<String> {
        let (s, r) = channel();
        let mut guard = self.channels.lock().unwrap();

        let list = guard.get_mut(MONITOR).unwrap();
        list.push(s);

        r
    }
}
