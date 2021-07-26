use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

use crate::matcher::matcher;

#[doc(hidden)]
pub const MONITOR: &str = "Monitor";
#[doc(hidden)]
pub const LOGGER: &str = "Logger";
#[doc(hidden)]
pub const SPECIAL_CHANNELS_ID: u32 = 0;

#[doc(hidden)]
type Dictionary = Arc<Mutex<HashMap<String, Vec<(u32, Sender<String>)>>>>;

/// A Channels implemented in a multithreading context.
///
/// Channels uses Arc and Mutex to be shared safety in a multithreading context
/// implementing clone.
/// It is the one in charge of subscribe or unsubscribe clients to specific channels,
/// it also has two special channels: Monitor and Logger.
/// 
pub struct Channels {
    #[doc(hidden)]
    channels: Dictionary,
}

impl<'a> Clone for Channels {
    fn clone(&self) -> Self {
        Channels::new_from_channels(self.channels.clone())
    }
}

impl Channels {
    #[doc(hidden)]
    fn new_from_channels(channels: Dictionary) -> Self {
        Channels { channels }
    }

    /// Creates a new Channels with 2 channels: Logger and Monitor.
    /// # Examples
    /// Basic Usage:
    /// ```
    /// let channels = Channels::new();
    /// ```
    pub fn new() -> Channels {
        let mut hash = HashMap::new();
        hash.insert(MONITOR.to_string(), Vec::new());
        hash.insert(LOGGER.to_string(), Vec::new());
        let channels = Arc::new(Mutex::new(hash));
        Channels { channels }
    }

    /// Subscribes a client with his sender and id in the corresponding channel.
    /// # Examples
    /// Basic Usage:
    /// ```
    /// let mut channels = Channels::new();
    /// let (s, _) = channel();
    /// channels.subscribe("channel", s, 1);
    /// ```
    pub fn subscribe(&mut self, channel: &str, sender: Sender<String>, id: u32) {
        let mut guard = self.channels.lock().unwrap();
        match guard.get_mut(channel) {
            Some(l) => l.push((id, sender)),
            None => {
                guard.insert(channel.to_string(), vec![(id, sender)]);
            }
        }
    }

    /// Unsubscribes a client with that id of the corresponding channel.
    /// and return the number of subscriptors that received the message.
    /// # Examples
    /// Basic Usage:
    /// ```
    /// let mut channels = Channels::new();
    /// let (s, r) = channel();
    /// channels.subscribe("channel", s, 1);
    /// 
    /// let number = channels.send("channel", "hola");
    /// assert_eq!(number, 1);
    ///
    /// let r = r.recv().unwrap();
    /// assert_eq!(r, "hola");
    /// 
    /// channels.unsubscribe("channel", 1);
    /// let number = channels.send("channel", "hola");
    /// assert_eq!(number, 0);
    /// ```
    pub fn unsubscribe(&mut self, channel: &str, id: u32) {
        let mut guard = self.channels.lock().unwrap();
        if let Some(l) = guard.get_mut(channel) {
            l.retain(|x| x.0 != id);
            if l.is_empty() {
                guard.remove(channel);
            }
        }
    }

    /// Adds a new Logger with his sender.
    /// # Examples
    /// Basic Usage:
    /// ```
    /// let mut channels = Channels::new();
    /// let mut logger = Logger::new("log_file.log", false);
    /// let log_sender = logger.run();
    /// channels.add_logger(log_sender);
    /// ```
    pub fn add_logger(&mut self, logger_sender: Sender<String>) {
        self.subscribe(LOGGER, logger_sender, SPECIAL_CHANNELS_ID);
    }

    /// Adds a new monitor and rerturn his receiver to listen.
    /// # Examples
    /// Basic Usage:
    /// ```
    /// let mut channels = Channels::new();
    /// let r = channels.add_monitor();
    /// for msg in r.iter() {
    ///     println!("{}", msg);
    /// }
    /// ```
    pub fn add_monitor(&mut self) -> Receiver<String> {
        let (s, r) = channel();
        self.subscribe(MONITOR, s, SPECIAL_CHANNELS_ID);

        r
    }

    /// Sends a message to all the subscriptors in the corresponding channel
    /// and return the number of subscriptors that received the message.
    /// # Examples
    /// Basic Usage:
    /// ```
    /// let mut channels = Channels::new();
    /// let (s, r) = channel();
    /// channels.subscribe("channel_1", s, 1);
    /// 
    /// let number = channels.send("channel_1", "hola");
    /// assert_eq!(number, 1);
    ///
    /// let r = r.recv().unwrap();
    /// assert_eq!(r, "hola");
    /// ```
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

    /// Sends a message to the logger, if there's anyone.
    /// 
    /// It's a rapper from send to the special channel Logger, that means 
    /// you can use in the same way that other channel.
    pub fn send_logger(&mut self, id: u32, msg: &str) {
        let msg: String = id.to_string() + " " + " " + msg;
        self.send(LOGGER, &msg);
    }

    /// Sends a message all the active monitors.
    /// 
    /// It's a rapper from send to the special channel Monitor, that means 
    /// you can use in the same way that other channel.
    pub fn send_monitor(&mut self, id: u32, msg: &str) {
        let msg: String = "Client: ".to_string() + &id.to_string() + " " + msg;
        self.send(MONITOR, &msg);
    }

    /// Get all channels that matches with the pattern passed in a list of strings 
    /// except Logger and Monitor.
    /// # Examples
    /// Basic Usage:
    /// ```
    /// let mut channels = Channels::new();
    /// add_channels(&mut channels);
    ///
    /// let mut c = channels.get_channels("*");
    /// for i in 1..6 {
    ///     let (s, _) = channel();
    ///     channels.subscribe(&i.to_string(), s, i);
    /// }
    /// 
    /// c.sort_by(|a, b| {
    ///     let a = a.parse::<i32>().unwrap();
    ///     let b = b.parse::<i32>().unwrap();
    ///     a.cmp(&b)
    /// });
    /// 
    /// assert_eq!(
    ///    c,
    ///    vec!["1", "2", "3", "4", "5"]
    /// );
    /// ```
    pub fn get_channels(&self, pattern: &str) -> Vec<String> {
        let guard = self.channels.lock().unwrap();
        guard
            .keys()
            .filter(|x| matcher(x, pattern) && *x != MONITOR && *x != LOGGER)
            .map(|item| item.to_string())
            .collect()
    }

    /// Gets the number of subscriptors in the corresponding channel.
    /// # Examples
    /// Basic Usage:
    /// ```
    /// let mut channels = Channels::new();
    /// let (s, _) = channel();
    /// 
    /// for i in 1..6 {
    ///     channels.subscribe("channel", s.clone(), i);
    ///     let number = channels.subcriptors_number("channel");
    ///     assert_eq!(number, i as usize);
    /// }
    /// ```
    pub fn subcriptors_number(&self, channel: &str) -> usize {
        let guard = self.channels.lock().unwrap();
        match guard.get(channel) {
            Some(l) => l.len(),
            None => 0,
        }
    }
}

#[cfg(test)]
mod channels_test {
    use super::*;

    const CHANNEL_1: &str = "1";
    const CHANNEL_2: &str = "2";
    const CHANNEL_3: &str = "3";
    const CHANNEL_4: &str = "4";
    const CHANNEL_5: &str = "5";

    const CHANNELS: [&str; 5] = [CHANNEL_1, CHANNEL_2, CHANNEL_3, CHANNEL_4, CHANNEL_5];

    const ID_1: u32 = 1;
    const ID_2: u32 = 2;
    const ID_3: u32 = 3;
    const ID_4: u32 = 4;
    const ID_5: u32 = 5;
    const IDS: [u32; 5] = [ID_1, ID_2, ID_3, ID_4, ID_5];

    const MSG: &str = "hola";

    fn add_channels(channels: &mut Channels) -> Vec<Receiver<String>> {
        let mut receivers = Vec::new();
        for i in 1..6 {
            let (s, r) = channel();
            channels.subscribe(&i.to_string(), s, i);
            receivers.push(r);
        }

        receivers
    }

    fn add_all_channels_in_all_clients(channels: &mut Channels) -> Vec<Receiver<String>> {
        let mut receivers = Vec::new();
        for id in &IDS {
            let (s, r) = channel();
            receivers.push(r);
            for c in &CHANNELS {
                channels.subscribe(c, s.clone(), *id);
            }
        }

        receivers
    }

    #[test]
    fn subscribe_to_different_channels() {
        let mut channels = Channels::new();
        add_channels(&mut channels);

        let guard = channels.channels.lock().unwrap();

        for i in 1..6 {
            assert!(guard.contains_key(&i.to_string()));
        }
    }

    #[test]
    fn subscribe_to_different_channels_then_get_channels_all_channels() {
        let mut channels = Channels::new();
        add_channels(&mut channels);

        let mut c = channels.get_channels("*");
        c.sort_by(|a, b| {
            let a = a.parse::<i32>().unwrap();
            let b = b.parse::<i32>().unwrap();
            a.cmp(&b)
        });
        assert_eq!(
            c,
            vec![CHANNEL_1, CHANNEL_2, CHANNEL_3, CHANNEL_4, CHANNEL_5]
        );
    }

    #[test]
    fn subscribe_and_send_a_msg() {
        let mut channels = Channels::new();
        let (s, r) = channel();
        channels.subscribe(CHANNEL_1, s, ID_1);

        let number = channels.send(CHANNEL_1, MSG);
        assert_eq!(number, 1);

        let r = r.recv().unwrap();
        assert_eq!(r, MSG);
    }

    #[test]
    fn subscribe_all_channels_and_send_a_msg() {
        let mut channels = Channels::new();
        let receivers = add_channels(&mut channels);

        for c in &CHANNELS {
            let n = channels.send(c, MSG);
            assert_eq!(n, 1);
        }

        for r in receivers {
            let r = r.recv().unwrap();
            assert_eq!(r, MSG);
        }
    }

    #[test]
    fn subscribe_a_client_to_different_channels_and_send_a_msg() {
        let mut channels = Channels::new();
        let (s, r) = channel();
        for c in &CHANNELS {
            channels.subscribe(c, s.clone(), ID_2);
        }

        for c in &CHANNELS {
            let n = channels.send(c, MSG);
            assert_eq!(n, 1);
        }

        let mut msgs = Vec::new();
        for _ in 1..6 {
            let msg = r.recv().unwrap();
            msgs.push(msg);
        }

        assert_eq!(msgs, vec![MSG; 5]);
    }

    #[test]
    fn subscribe_different_clients_to_different_channeles_and_get_subscriptors_num() {
        let mut channels = Channels::new();
        add_all_channels_in_all_clients(&mut channels);

        for c in &CHANNELS {
            let number = channels.subcriptors_number(c);
            assert_eq!(number, CHANNELS.len());
        }
    }

    #[test]
    fn subscribe_number_works_properly() {
        let mut channels = Channels::new();
        let (s, _) = channel();

        for i in 1..6 {
            channels.subscribe(CHANNEL_1, s.clone(), i);
            let number = channels.subcriptors_number(CHANNEL_1);
            assert_eq!(number, i as usize);
        }
    }

    #[test]
    fn unsubscribe_works_properly() {
        let mut channels = Channels::new();
        let (s, r) = channel();
        
        channels.subscribe(CHANNEL_1, s, ID_1);
        let number = channels.send(CHANNEL_1, MSG);
        assert_eq!(number, 1);

        let r = r.recv().unwrap();
        assert_eq!(r, "hola");

        channels.unsubscribe(CHANNEL_1, ID_1);
        let number = channels.send(CHANNEL_1, MSG);
        assert_eq!(number, 0);
    }

    #[test]
    fn subscribe_and_unsubscribe_different_clients_sending_msgs() {
        let mut channels = Channels::new();
        let receivers = add_all_channels_in_all_clients(&mut channels);

        for c in &CHANNELS {
            let n = channels.send(c, MSG);
            assert_eq!(n, 5);
        }

        for r in &receivers {
            let mut msgs = Vec::new();
            for _ in 1..6 {
                let msg = r.recv().unwrap();
                msgs.push(msg);
            }

            assert_eq!(msgs, vec![MSG; 5]);
        }

        channels.unsubscribe(CHANNEL_5, ID_1);

        channels.unsubscribe(CHANNEL_4, ID_1);
        channels.unsubscribe(CHANNEL_4, ID_2);

        channels.unsubscribe(CHANNEL_3, ID_1);
        channels.unsubscribe(CHANNEL_3, ID_2);
        channels.unsubscribe(CHANNEL_3, ID_3);

        channels.unsubscribe(CHANNEL_2, ID_1);
        channels.unsubscribe(CHANNEL_2, ID_2);
        channels.unsubscribe(CHANNEL_2, ID_3);
        channels.unsubscribe(CHANNEL_2, ID_4);

        channels.unsubscribe(CHANNEL_1, ID_1);
        channels.unsubscribe(CHANNEL_1, ID_2);
        channels.unsubscribe(CHANNEL_1, ID_3);
        channels.unsubscribe(CHANNEL_1, ID_4);
        channels.unsubscribe(CHANNEL_1, ID_5);

        for c in &CHANNELS {
            channels.send(c, MSG);
        }

        for i in 0..5 {
            let r = receivers.get(i).unwrap();
            let mut msgs = Vec::new();
            for _ in 0..i {
                let msg = r.recv().unwrap();
                msgs.push(msg);
            }

            assert_eq!(msgs, vec![MSG; i]);
        }
    }
}
