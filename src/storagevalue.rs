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
                let mut parms_string = String::new();

                for elem in val {
                    parms_string.push_str(elem);
                    parms_string.push(' ');
                }

                write!(f, "{}", parms_string)
            }
        }
    }
}
