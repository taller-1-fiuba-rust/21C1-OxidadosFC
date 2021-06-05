use core::fmt::{Display, Formatter, Result};

#[derive(Clone)]
pub enum StorageValue {
    String(String),
    List(Vec<String>),
}

impl Display for StorageValue {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            StorageValue::String(val) => write!(f, "{}", val),
            StorageValue::List(val) => {
                for elem in val {
                    write!(f, "{}", elem);
                }
                Ok(())
            }
        }
    }
}
