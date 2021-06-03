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
    KeyAlredyExist,
    KeyNotExistsInDatabaseRename,
    NoMatch,
}

impl fmt::Display for MensajeErroresDataBase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            MensajeErroresDataBase::KeyNotExistsInDatabase => {
                write!(f, "(nil)")
            }
            MensajeErroresDataBase::ValueNotIsAnString => {
                write!(f, "value of the key not is an String")
            }
            MensajeErroresDataBase::ParseIntError => write!(f, "the value cannot be parsed to int"),
            MensajeErroresDataBase::NumberOfParamsIsIncorrectly => {
                write!(f, "number of parameters is incorrectly")
            }
            MensajeErroresDataBase::KeyNotExistsInDatabaseRename => {
                write!(f, "ERR ERR no such key")
            }
            MensajeErroresDataBase::KeyAlredyExist => {
                write!(f, "the key alredy exist in the database")
            }
            MensajeErroresDataBase::NoMatch => {
                write!(f, "(empty list or set)")
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

    pub fn rename(
        &mut self,
        old_key: &str,
        new_key: &str,
    ) -> Result<String, MensajeErroresDataBase> {
        match self.dictionary.remove(old_key) {
            Some(value) => {
                self.dictionary.insert(new_key.to_string(), value);
                Ok("Ok".to_string())
            }
            None => Err(MensajeErroresDataBase::KeyNotExistsInDatabaseRename),
        }
    }

    pub fn keys(&mut self, pattern: &str) -> Result<String, MensajeErroresDataBase> {
        let result: String = self
            .dictionary
            .keys()
            .filter(|x| x.contains(pattern))
            .map(|x| x.to_string() + "\r\n")
            .collect();

        match result.strip_suffix("\r\n") {
            Some(s) => Ok(s.to_string()),
            None => Err(MensajeErroresDataBase::NoMatch),
        }
    }

    pub fn exists(&mut self, key: &str) -> Result<String, MensajeErroresDataBase> {
        Ok((self.dictionary.contains_key(key) as i8).to_string())
    }

    pub fn copy(&mut self, key: &str, to_key: &str) -> Result<String, MensajeErroresDataBase> {
        let value = match self.dictionary.get(key) {
            Some(StorageValue::String(val)) => {
                if self.dictionary.contains_key(to_key) {
                    return Err(MensajeErroresDataBase::KeyAlredyExist);
                } else {
                    val.clone()
                }
            }
            None => return Err(MensajeErroresDataBase::KeyNotExistsInDatabase),
        };
        self.dictionary
            .insert(String::from(to_key), StorageValue::String(value));
        Ok(String::from("1"))
    }

    pub fn del(&mut self, key: &str) -> Result<String, MensajeErroresDataBase> {
        match self.dictionary.remove(key) {
            Some(_) => Ok(String::from("1")),
            None => Ok(String::from("0")),
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
mod group_string {

    use crate::database::Database;
    
    #[test]
    fn test_append_new_key_return_lenght_of_the_value() {
        let mut database = Database::new();

        let result = database.append("key", "value");

        assert_eq!(result.unwrap(), String::from("5"));
    }

    #[test]
    fn test_append_key_with_valueold_return_lenght_of_the_total_value() {
        let mut database = Database::new();

        database
            .append("key", "value")
            .expect("falla en pasaje de parametros");

        let result = database.append("key", "_concat");

        assert_eq!(result.unwrap(), String::from("12"));
    }

    #[test]
    fn test_append_return_err_if_value_is_not_an_string() {
        //no se puede probar dado que append, recibe un string y deberia recibir un Tipo generico.
        /*let mut database = Database::new();
        let vector = vec![1;2;3];

        let result = database.append("key", );

        assert_eq!(result.unwrap() , String::from("(integer) 12"));
        */
    }

    #[test]
    fn test_get_returns_value_of_key() {
        let mut database = Database::new();

        database.set("key", "value").expect("error");

        let result = database.get("key");

        assert_eq!(result.unwrap(), String::from("value"));
    }

    #[test]
    fn test_get_returns_error_if_the_key_does_not_exist() {
        let mut database = Database::new();
        let result = database.get("key_no_exist");

        assert_eq!(result.unwrap_err().to_string(), "(nil)")
    }

    #[test]
    fn test_set_returns_ok() {
        let mut database = Database::new();
        let result = database.set("key", "1");
        let value = database.get("key");

        assert_eq!(result.unwrap(), "OK".to_string());
        assert_eq!(value.unwrap(), "1".to_string())
    }

    #[test]
    fn test_getdel_returns_old_value_of_key_and_after_value_of_key_what_not_exists_in_database_is_nul(
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
    fn test_getdel_returns_nil_value_if_key_not_exist_in_database() {
        let mut database = Database::new();

        let value = database.getdel("key");

        assert_eq!(value.unwrap_err().to_string(), "(nil)".to_string())
    }

    #[test]
    fn test_incrby_returns_lenght_of_the_resulting_value_after_increment() {
        let mut database = Database::new();

        database.set("key", "1").expect("error");

        let result = database.incrby("key", "4");

        assert_eq!(result.unwrap(), "5".to_string());
    }

    #[test]
    fn test_incrby_returns_err_if_the_value_can_not_be_parse_to_int() {
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
    fn test_incrby_returns_err_if_the_value_is_not_an_string() {
        /*let mut database = Database::new();

        database.rpush("key", "1").expect("error");

        let result = database.incrby("key", "4");

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "value of the key not is an String".to_string());
        */
    }

    #[test]
    fn test_decrby_returns_lenght_of_the_resulting_value_after_increment() {
        let mut database = Database::new();

        database.set("key", "5").expect("error");

        let result = database.decrby("key", "4");

        assert_eq!(result.unwrap(), "1".to_string());

        //assert_eq!(value.unwrap_err().to_string(), "the value cannot be parsed to int".to_string())
    }

    #[test]
    fn test_decrby_returns_err_if_the_value_can_not_be_parse_to_int() {
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
    fn test_decrby_returns_err_if_the_value_of_key_is_not_an_string() {
        /*let mut database = Database::new();

        //database.rpush("key", "1").expect("error");

        let result = database.decrby("key", "4");

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "value of the key not is an String".to_string());
        */
    }

    #[test]
    fn test_strlen_returns_the_lenght_of_value_key() {
        let mut database = Database::new();

        database.set("key", "abcd").expect("error");

        let result = database.strlen("key");

        assert_eq!(result.unwrap(), String::from("4"));
    }

    #[test]
    fn test_strlen_returns_zero_if_the_key_not_exists_in_database() {
        let mut database = Database::new();

        let result = database.strlen("no_exists_key");

        assert_eq!(result.unwrap(), String::from("0"));
    }

    #[test]
    fn test_strlen_returns_err_if_value_of_key_is_not_an_string() {
        /*let mut database = Database::new();

        database.rpush("key", "abcd").expect("error");

        let result = database.strlen("key");

        assert_eq!(result.unwrap_err().to_string(), String::from("value of the key not is an String"));
        */
    }

    #[test]
    fn test_mset_set_multiple_key_and_value_ok() {
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
    fn test_mset_returns_err_if_multiple_key_and_value_are_inconsistent_in_number() {
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
    fn test_mget_return_all_values_of_keys_if_all_keys_are_in_database() {
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
    fn test_mget_returns_nil_for_some_key_that_no_exists_in_database() {
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

#[cfg(test)]
mod group_keys{
    use crate::database::Database;
    use std::collections::HashSet;

    #[test]
    fn test_copy_set_dolly_sheep_then_copy_to_clone() {
        let mut database = Database::new();
        let _ = database.set("dolly", "sheep");

        let result = database.copy("dolly", "clone");
        assert_eq!(result.unwrap(), "1");
        assert_eq!(database.get("clone").unwrap(), "sheep");
    }

    #[test]
    fn test_copy_set_dolly_sheep_then_copy_to_clone_when_clone_exist() {
        let mut database = Database::new();
        let _ = database.set("dolly", "sheep");
        let _ = database.set("clone", "whatever");

        let result = database.copy("dolly", "clone");
        assert_eq!(
            result.unwrap_err().to_string(),
            "the key alredy exist in the database"
        );
    }

    #[test]
    fn test_copy_try_to_copy_a_key_does_not_exist() {
        let mut database = Database::new();

        let result = database.copy("dolly", "clone");
        assert_eq!(result.unwrap_err().to_string(), "(nil)");
    }

    #[test]
    fn test_del_key_hello_returns_1() {
        let mut database = Database::new();
        let _ = database.set("key", "hello");

        let result = database.del("key");
        assert_eq!(result.unwrap(), "1");
        assert_eq!(database.get("key").unwrap_err().to_string(), "(nil)");
    }

    #[test]
    fn test_del_key_non_exist_returns_0() {
        let mut database = Database::new();

        let result = database.del("key");
        assert_eq!(result.unwrap(), "0");
        assert_eq!(database.get("key").unwrap_err().to_string(), "(nil)");
    }

    #[test]
    fn test_exists_key_non_exist_returns_0() {
        let mut database = Database::new();

        let result = database.exists("key");
        assert_eq!(result.unwrap(), "0");
        assert_eq!(database.get("key").unwrap_err().to_string(), "(nil)");
    }

    #[test]
    fn test_exists_key_hello_returns_0() {
        let mut database = Database::new();
        let _ = database.set("key", "hello");

        let result = database.exists("key");
        assert_eq!(result.unwrap(), "1");
        assert_eq!(database.get("key").unwrap(), "hello");
    }

    #[test]
    fn test_keys_obtain_keys_with_name() {
        let mut database = Database::new();
        let _ = database.set("firstname", "Alex");
        let _ = database.set("lastname", "Arbieto");
        let _ = database.set("age", "22");

        let result = database.keys("name").unwrap();
        let result: HashSet<_> = result.split("\r\n").collect();
        assert_eq!(result, ["firstname", "lastname"].iter().cloned().collect());
    }

    #[test]
    fn test_keys_obtain_keys_with_nomatch_returns_empty_string() {
        let mut database = Database::new();
        let _ = database.set("firstname", "Alex");
        let _ = database.set("lastname", "Arbieto");
        let _ = database.set("age", "22");

        let result = database.keys("nomatch");
        assert_eq!(result.unwrap_err().to_string(), "(empty list or set)");
    }

    #[test]
    fn test_rename_key_with_mykey_get_hello() {
        let mut database = Database::new();
        let _ = database.set("key", "hello");

        let result = database.rename("key", "mykey").unwrap();
        assert_eq!(result, "Ok");

        let result = database.get("mykey").unwrap();
        assert_eq!(result, "hello");

        let result = database.get("key").unwrap_err().to_string();
        assert_eq!(result, "(nil)");
    }

    #[test]
    fn test_rename_key_non_exists_error() {
        let mut database = Database::new();

        let result = database.rename("key", "mykey").unwrap_err().to_string();
        assert_eq!(result, "ERR ERR no such key");
    }
}
