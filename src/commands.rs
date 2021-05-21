pub enum Command<'a> {
    Append(&'a str, &'a str),
    Print,
    None,
}

impl<'a> Command<'a> {
    pub fn new(command: &str) -> Command {
        let command: Vec<&str> = command.trim().split_whitespace().collect();

        match &command[..] {
            ["append", key, value] => Command::Append(key, value),
            ["print"] => Command::Print,
            _ => Command::None,
        }
    }
}

