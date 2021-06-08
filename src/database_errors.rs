use std::fmt;

#[derive(Debug)]
pub enum DataBaseError {
    NotAString,
    NonExistentKey,
    NotAnInteger,
    KeyAlredyExist,
    NoMatch,
    NumberOfParamsIsIncorrectly,
    NotASet,
}

impl fmt::Display for DataBaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            DataBaseError::NonExistentKey => write!(f, "Non-existent key"),
            DataBaseError::NotAString => write!(f, "Value isn't a String"),
            DataBaseError::NotAnInteger => write!(f, "Value isn't an Integer"),
            DataBaseError::KeyAlredyExist => write!(f, "the key alredy exist in the database"),
            DataBaseError::NoMatch => write!(f, "(empty list or set)"),
            DataBaseError::NumberOfParamsIsIncorrectly => {
                write!(f, "number of parameters is incorrectly")
            }
            DataBaseError::NotASet => write!(f, "element of key isn't a Set"),
        }
    }
}
