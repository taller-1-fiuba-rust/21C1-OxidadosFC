use regex::Regex;

pub fn matcher(text: &str, pattern: &str) -> bool {
    let patt: String = "^".to_owned() + pattern + "$";
    let patt: String = patt.replace("*", ".*");
    let patt: String = patt.replace("?", ".");
    match Regex::new(&patt) {
        Ok(re) => re.is_match(text),
        Err(_) => false,
    }
}
