use std::{ fs, io, env };
use dotenv_parser::parse_dotenv;

//? How does ENV parsing the library work?
// TODO: FITFO
pub enum EnvParseError {
    FailedToRead(io::Result<String>), // io::error::Error is private, so idk how to define this with the error type
    FailedToParse,
}
fn read_file(path: String) -> Result<String, EnvParseError> {
    let res = fs::read_to_string(&path);
    
    if res.is_err() {
        return Err(EnvParseError::FailedToRead(res));
    }
    Ok(res.unwrap())
}
pub fn load_env() -> Result<(), EnvParseError> {
    let base_env_source = read_file("base.env".to_string())?;
    let env_source = read_file(".env".to_string())?;

    // I have no fucking idea if this is how you're supposed to do it. It doesn't make sense why we're adding a 
    // String and a &str type especially when Rust is extremely stingy about ownership. This language is weird
    // I am not downloading another dependency just to concatenate fucking strings
    let full_env_source = base_env_source + "\n" + &env_source;

    // cannot get try operator to work because parse_dotenv has a strange error type, unwrap_or is safer anyway
    let parse_result = parse_dotenv(&full_env_source);

    if let Err(e) = parse_result {
        return Err(EnvParseError::FailedToParse);
    }
    let parsed = parse_result.unwrap();

    println!("{:#?}", parsed);

    unsafe {
        for (k, v) in parsed {
            env::set_var(k, v)
        };
    }
    Ok(())
}
