pub enum Command<'a> {
    Append(&'a str, &'a str),
    Incrby(&'a str, &'a str),
    Decrby(&'a  str, &'a str),
    Get(&'a str),

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
            ["print"] => Command::Print,
            _ => Command::None,
        }
    }
}
