pub enum Command<'a> {
    Append(&'a str, &'a str),
    Incrby(&'a str, &'a str),
    Decrby(&'a str, &'a str),
    Get(&'a str),
    Getdel(&'a str),
    Getset(&'a str, &'a str),
    Set(&'a str, &'a str),
    Copy(&'a str, &'a str),
    Del(&'a str),
    Exists(&'a str),
    Keys(&'a str),
    Rename(&'a str, &'a str),
    Print,
    None,
}

impl<'a> Command<'a> {
    pub fn new(command: &str) -> Command {
        let command: Vec<&str> = command.trim().split_whitespace().collect();

        match &command[..] {
            ["append", key, value] => Command::Append(key, value),
            ["incrby", key, number_of_incr] => Command::Incrby(key, number_of_incr),
            ["decrby", key, number_of_decr] => Command::Decrby(key, number_of_decr),
            ["get", key] => Command::Get(key),
            ["getdel", key] => Command::Getdel(key),
            ["getset", key, value] => Command::Getset(key, value),
            ["set", key, value] => Command::Set(key, value),
            ["copy", key, to_key] => Command::Copy(key, to_key),
            ["del", key] => Command::Del(key),
            ["exists", key] => Command::Exists(key),
            ["keys", pattern] => Command::Keys(pattern),
            ["rename", old_key, new_key] => Command::Rename(old_key, new_key),
            ["print"] => Command::Print,
            _ => Command::None,
        }
    }
}
