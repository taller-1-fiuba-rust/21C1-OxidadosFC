use core::fmt::{Display, Formatter, Result};

pub enum StorageValue {
    String(String),
    Integer(i64),
    List(Vec<String>),
}

impl Display for StorageValue {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            StorageValue::String(val) => write!(f, "{}", val),
            StorageValue::Integer(val) => write!(f, "{}", val),
            StorageValue::List(val) => write!(f, "{:?}", val),
        }
    }
}
