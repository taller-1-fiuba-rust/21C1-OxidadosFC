pub enum Command<'a> {
    Append(&'a str, &'a str),
    Incrby(&'a str, &'a str),
    Decrby(&'a str, &'a str),
    Set(&'a str, &'a str),
    Get(&'a str),
    Getdel(&'a str),
    Getset(&'a str, &'a str),
    Strlen(&'a str),
    Mset(Vec<&'a str>),
    Mget(Vec<&'a str>),
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
            ["set", key, value] => Command::Set(key, value),
            ["get", key] => Command::Get(key),
            ["getdel", key] => Command::Getdel(key),
            ["getset", key, value] => Command::Getset(key, value),
            ["strlen", key] => Command::Strlen(key),
            ["mset", ..] => Command::Mset(command),
            ["mget", ..] => Command::Mget(command),

            ["print"] => Command::Print,
            _ => Command::None,
        }
    }
}
