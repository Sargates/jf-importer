// use std::{ fs, io, env };
// use dotenv_parser::parse_dotenv;
//
// //? How does ENV parsing the library work?
// // TODO: FITFO
// #[derive(Debug)]
// pub enum FileIoError {
//     FailedToRead(io::Result<String>), // io::error::Error is private, so idk how to define this with the error type
//     FailedToParse,
// }
// pub fn read_file(path: String) -> Result<String, FileIoError> {
//     let res = fs::read_to_string(&path);
//     
//     match &res {
//         Err(e) => Err(FileIoError::FailedToRead(res)),
//         Ok(s)  => Ok(s.clone())
//     }
// }
// pub fn load_env() -> Result<(), FileIoError> {
//     let base_env_source = read_file("base.env".to_string())?;
//     let env_source = read_file(".env".to_string())?;
//
//     let full_env_source = base_env_source + "\n" + &env_source;
//
//     // cannot get try operator to work because parse_dotenv has a strange error type, unwrap_or is safer anyway
//     let parse_result = parse_dotenv(&full_env_source);
//
//     if let Err(e) = parse_result {
//         return Err(FileIoError::FailedToParse);
//     }
//     let parsed = parse_result.unwrap();
//
//     println!("{:?}", parsed);
//
//     unsafe {
//         for (k, v) in parsed {
//             env::set_var(k, v)
//         };
//     }
//     Ok(())
// }
