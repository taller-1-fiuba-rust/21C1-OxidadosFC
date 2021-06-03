use crate::storagevalue::StorageValue;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Formatter;

pub struct Database {
    dictionary: HashMap<String, StorageValue>,
}

#[derive(Debug)]
pub enum MensajeErroresDataBase {
    ValueNotIsAnString,
    KeyNotExistsInDatabase,
    ParseIntError,
    NumberOfParamsIsIncorrectly,
}

impl fmt::Display for MensajeErroresDataBase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            MensajeErroresDataBase::KeyNotExistsInDatabase => write!(f, "(nil)"),
            MensajeErroresDataBase::ValueNotIsAnString => {
                write!(f, "value of the key not is an String")
            }
            MensajeErroresDataBase::ParseIntError => write!(f, "the value cannot be parsed to int"),
            MensajeErroresDataBase::NumberOfParamsIsIncorrectly => {
                write!(f, "number of parameters is incorrectly")
            }
        }
    }
}

pub fn es_par(a_positive_value: &usize) -> bool {
    *a_positive_value % 2 == 0
}

impl Database {
    pub fn new() -> Database {
        Database {
            dictionary: HashMap::new(),
        }
    }

    pub fn append(&mut self, key: &str, value: &str) -> Result<String, MensajeErroresDataBase> {
        let len_value = value.len();
        if self.dictionary.contains_key(key) {
            if let Some(StorageValue::String(val)) = self.dictionary.get_mut(key) {
                val.push_str(value);
                let len_result = val.len();
                Ok(format!("{}", len_result))
                //Ok(String::from("(integer) ".to_owned() + &len_result.to_string()))
            } else {
                Err(MensajeErroresDataBase::ValueNotIsAnString)
            }
        } else {
            self.set(key, value).expect("error");
            Ok(format!("{}", len_value))
        }
    }

    pub fn get(&mut self, key: &str) -> Result<String, MensajeErroresDataBase> {
        if let Some(StorageValue::String(val)) = self.dictionary.get(key) {
            Ok(val.to_string())
        } else {
            Err(MensajeErroresDataBase::KeyNotExistsInDatabase)
        }
    }

    pub fn set(&mut self, key: &str, val: &str) -> Result<String, MensajeErroresDataBase> {
        self.dictionary
            .insert(String::from(key), StorageValue::String(val.to_string()));
        Ok(String::from("OK"))
    }

    // deberia retornar algun mensaje de Ok segun el protocolo Redis, o un Mensaje De error, para que haga un write el server.
    pub fn decrby(
        &mut self,
        key: &str,
        number_of_decr: &str,
    ) -> Result<String, MensajeErroresDataBase> {
        if let Some(StorageValue::String(val)) = self.dictionary.get_mut(key) {
            if let Ok(number_of_decr) = number_of_decr.parse::<i64>() {
                if let Ok(mut number) = val.parse::<i64>() {
                    number -= number_of_decr;
                    self.set(key, &number.to_string()).expect("error");
                    Ok(number.to_string())
                } else {
                    Err(MensajeErroresDataBase::ParseIntError)
                }
            } else {
                Err(MensajeErroresDataBase::ParseIntError)
            }
        } else {
            Err(MensajeErroresDataBase::ValueNotIsAnString)
        }
    }

    pub fn incrby(
        &mut self,
        key: &str,
        number_of_incr: &str,
    ) -> Result<String, MensajeErroresDataBase> {
        let mut number_incr = String::from(number_of_incr);
        number_incr.insert(0, '-');
        self.decrby(key, &number_incr)
    }

    pub fn getdel(&mut self, key: &str) -> Result<String, MensajeErroresDataBase> {
        if let Some(StorageValue::String(value)) = self.dictionary.remove(key) {
            Ok(value)
        } else {
            Err(MensajeErroresDataBase::KeyNotExistsInDatabase)
        }
    }

    //refactorizar para que cumpla su funcion.
    pub fn getset(&mut self, key: &str, _new_val: &str) -> Result<String, MensajeErroresDataBase> {
        let mut _copy_key = String::from(<&str>::clone(&key));
        if let Some(storage_value) = self.dictionary.get(&_copy_key) {
            match storage_value {
                StorageValue::String(old_value) => Ok(old_value.to_string()),
            }
            //self.dictionary.insert(String::from(key), StorageValue::String(_new_val.to_string()));
            /*if let StorageValue::String(old_value) = storage_value {
                //self.dictionary.insert(String::from(key), StorageValue::String(_new_val.to_string()));
                Ok(old_value.to_string())
            } else {
                Err(MensajeErroresDataBase::ValueNotIsAnString)
            }
            */
        } else {
            Err(MensajeErroresDataBase::KeyNotExistsInDatabase)
        }
    }

    pub fn mset(&mut self, params: &[&str]) -> Result<String, MensajeErroresDataBase> {
        if es_par(&params.len()) {
            for i in (0..params.len()).step_by(2) {
                let key = params[i];
                let value = params[i + 1];
                self.set(key, value).expect("error");
            }
            Ok("OK".to_string())
        } else {
            Err(MensajeErroresDataBase::NumberOfParamsIsIncorrectly)
        }
    }

    pub fn mget(&mut self, params: &[&str]) -> Result<String, MensajeErroresDataBase> {
        let mut result = String::from("");
        let mut count = 0;
        for key in params {
            count += 1;
            result.push_str((count.to_string() + &") ".to_string()).as_str());
            match self.get(key) {
                Ok(result_ok) => {
                    result.push_str(&result_ok);
                }
                Err(error_database) => {
                    result.push_str(&error_database.to_string());
                }
            }
            result.push_str(&'\n'.to_string());
        }
        Ok(result)
    }

    pub fn strlen(&mut self, key: &str) -> Result<String, MensajeErroresDataBase> {
        if self.dictionary.contains_key(key) {
            if let Some(StorageValue::String(val)) = self.dictionary.get_mut(key) {
                Ok(format!("{}", val.len()))
            } else {
                Err(MensajeErroresDataBase::ValueNotIsAnString)
            }
        } else {
            Ok("0".to_string())
        }
    }
}

impl fmt::Display for Database {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        for (key, value) in self.dictionary.iter() {
            writeln!(f, "key: {}, value: {}", key, value)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod commandtest {

    use crate::database::Database;

    #[test]
    fn test01_append_new_key_return_lenght_of_the_value() {
        let mut database = Database::new();

        let result = database.append("key", "value");

        assert_eq!(result.unwrap(), String::from("5"));
    }

    #[test]
    fn test02_append_key_with_valueold_return_lenght_of_the_total_value() {
        let mut database = Database::new();

        database
            .append("key", "value")
            .expect("falla en pasaje de parametros");

        let result = database.append("key", "_concat");

        assert_eq!(result.unwrap(), String::from("12"));
    }

    #[test]
    fn test03_append_return_err_if_value_is_not_an_string() {
        //no se puede probar dado que append, recibe un string y deberia recibir un Tipo generico.
        /*let mut database = Database::new();
        let vector = vec![1;2;3];

        let result = database.append("key", );

        assert_eq!(result.unwrap() , String::from("(integer) 12"));
        */
    }

    #[test]
    fn test04_get_returns_value_of_key() {
        let mut database = Database::new();

        database.set("key", "value").expect("error");

        let result = database.get("key");

        assert_eq!(result.unwrap(), String::from("value"));
    }

    #[test]
    fn test05_get_returns_error_if_the_key_does_not_exist() {
        let mut database = Database::new();
        let result = database.get("key_no_exist");

        assert_eq!(result.unwrap_err().to_string(), "(nil)")
    }

    #[test]
    fn test06_set_returns_ok() {
        let mut database = Database::new();
        let result = database.set("key", "1");
        let value = database.get("key");

        assert_eq!(result.unwrap(), "OK".to_string());
        assert_eq!(value.unwrap(), "1".to_string())
    }

    #[test]
    fn test07_getdel_returns_old_value_of_key_and_after_value_of_key_what_not_exists_in_database_is_nul(
    ) {
        let mut database = Database::new();
        database.set("key", "1").expect("error");
        let value = database.getdel("key");

        //now key, no exists in database.
        let _current_value_key = database.get("key");

        assert_eq!(value.unwrap(), "1".to_string());
        assert_eq!(
            _current_value_key.unwrap_err().to_string(),
            "(nil)".to_string()
        );
    }

    #[test]
    fn test08_getdel_returns_nil_value_if_key_not_exist_in_database() {
        let mut database = Database::new();

        let value = database.getdel("key");

        assert_eq!(value.unwrap_err().to_string(), "(nil)".to_string())
    }

    #[test]
    fn test09_incrby_returns_lenght_of_the_resulting_value_after_increment() {
        let mut database = Database::new();

        database.set("key", "1").expect("error");

        let result = database.incrby("key", "4");

        assert_eq!(result.unwrap(), "5".to_string());
    }

    #[test]
    fn test10_incrby_returns_err_if_the_value_can_not_be_parse_to_int() {
        let mut database = Database::new();

        database.set("key", "1").expect("error");

        let result = database.incrby("key", "4a");

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "the value cannot be parsed to int".to_string()
        );
    }

    #[test]
    fn test11_incrby_returns_err_if_the_value_is_not_an_string() {
        /*let mut database = Database::new();

        database.rpush("key", "1").expect("error");

        let result = database.incrby("key", "4");

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "value of the key not is an String".to_string());
        */
    }

    #[test]
    fn test12_decrby_returns_lenght_of_the_resulting_value_after_increment() {
        let mut database = Database::new();

        database.set("key", "5").expect("error");

        let result = database.decrby("key", "4");

        assert_eq!(result.unwrap(), "1".to_string());

        //assert_eq!(value.unwrap_err().to_string(), "the value cannot be parsed to int".to_string())
    }

    #[test]
    fn test13_decrby_returns_err_if_the_value_can_not_be_parse_to_int() {
        let mut database = Database::new();

        database.set("key", "1").expect("error");

        let result = database.decrby("key", "4a");

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "the value cannot be parsed to int".to_string()
        );
    }

    #[test]
    fn test14_decrby_returns_err_if_the_value_of_key_is_not_an_string() {
        /*let mut database = Database::new();

        //database.rpush("key", "1").expect("error");

        let result = database.decrby("key", "4");

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "value of the key not is an String".to_string());
        */
    }

    #[test]
    fn test15_strlen_returns_the_lenght_of_value_key() {
        let mut database = Database::new();

        database.set("key", "abcd").expect("error");

        let result = database.strlen("key");

        assert_eq!(result.unwrap(), String::from("4"));
    }

    #[test]
    fn test17_strlen_returns_zero_if_the_key_not_exists_in_database() {
        let mut database = Database::new();

        let result = database.strlen("no_exists_key");

        assert_eq!(result.unwrap(), String::from("0"));
    }

    #[test]
    fn test18_strlen_returns_err_if_value_of_key_is_not_an_string() {
        /*let mut database = Database::new();

        database.rpush("key", "abcd").expect("error");

        let result = database.strlen("key");

        assert_eq!(result.unwrap_err().to_string(), String::from("value of the key not is an String"));
        */
    }

    #[test]
    fn test19_mset_set_multiple_key_and_value_ok() {
        let vect_key_value = vec!["key1", "value1", "key2", "value2"];
        let mut database = Database::new();

        let result = database.mset(&vect_key_value);

        let result_get1 = database.get("key1");
        let result_get2 = database.get("key2");

        assert_eq!(result.unwrap().to_string(), String::from("OK"));
        assert_eq!(result_get1.unwrap().to_string(), String::from("value1"));
        assert_eq!(result_get2.unwrap().to_string(), String::from("value2"));
    }

    #[test]
    fn test20_mset_returns_err_if_multiple_key_and_value_are_inconsistent_in_number() {
        //without value for key2
        let vect_key_value = vec!["key1", "value1", "key2"];

        let mut database = Database::new();

        let result = database.mset(&vect_key_value);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            String::from("number of parameters is incorrectly")
        );
    }

    #[test]
    fn test21_mget_return_all_values_of_keys_if_all_keys_are_in_database() {
        //without value for key2
        let vect_key_value = vec!["key1", "key2"];

        let mut database = Database::new();

        database.set("key1", "value1").expect("error");
        database.set("key2", "value2").expect("error");

        let result = database.mget(&vect_key_value);

        //refactorizar para que devuelva sin el \n al final del string
        assert_eq!(result.unwrap(), "1) value1\n2) value2\n");
    }

    #[test]
    fn test22_mget_returns_nil_for_some_key_that_no_exists_in_database() {
        //without value for key2
        let vect_key_value = vec!["key1", "key2", "key3"];

        let mut database = Database::new();

        database.set("key1", "value1").expect("error");
        database.set("key3", "value3").expect("error");

        let result = database.mget(&vect_key_value);

        //refactorizar para que devuelva sin el \n al final del string
        assert_eq!(result.unwrap(), "1) value1\n2) (nil)\n3) value3\n");
    }
}
