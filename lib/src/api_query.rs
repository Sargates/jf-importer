use reqwest::blocking::{
    Client,
    RequestBuilder,
    Response,
};

use std::{fs, io};
use regex::Regex;
// use std::env;
use urlencoding;

use crate::config::CONFIG;
use crate::config::SECRETS;
// use crate::env::FileIoError;
use crate::media_item::{Movie, Show, Episode, Mappable, MappingError};

#[derive(Debug, Clone)]
pub enum QueryError {
    /// For TreeNode variants that aren't Movie, Show, Episode
    CantQueryOnType, 

    /// Could be VarError::NotPresent or VarError::NotUnicode
    UnsetApiKey,

    /// Mappings from `reqwest::Error`
    ReqwestBuilder,
    ReqwestRedirect,
    ReqwestStatus,
    ReqwestTimeout,
    ReqwestRequest,
    ReqwestConnect,
    ReqwestBody,
    ReqwestDecode,
    ReqwestUpgrade,
    ReqwestUnknown,

    JsonParseError,

    InvalidApiKey,
    FailedToQueryApi,
    NoSearchResultsFromApi,
    FailedToExtractApiData(String),
    // TODO: Reference actual API responses
    //? Create unified enum for different APIs?
}
impl From<reqwest::Error> for QueryError {
    fn from(value: reqwest::Error) -> Self {
        if      value.is_builder()  { QueryError::ReqwestBuilder  }
        else if value.is_redirect() { QueryError::ReqwestRedirect } 
        else if value.is_status()   { QueryError::ReqwestStatus  } 
        else if value.is_timeout()  { QueryError::ReqwestTimeout  } 
        else if value.is_request()  { QueryError::ReqwestRequest  } 
        else if value.is_connect()  { QueryError::ReqwestConnect  } 
        else if value.is_body()     { QueryError::ReqwestBody     } 
        else if value.is_decode()   { QueryError::ReqwestDecode   } 
        else if value.is_upgrade()  { QueryError::ReqwestUpgrade } 
        else                        { QueryError::ReqwestUnknown }
    }
}
impl From<json::Error> for QueryError {
    fn from(value: json::Error) -> Self {

        QueryError::JsonParseError
        // UnexpectedCharacter { ch, line, column },
        // UnexpectedEndOfJson,
        // ExceededDepthLimit,
        // FailedUtf8Parsing,
        // WrongType(String),
        // todo!()
    }
}

pub enum FileIoError {
    FailedToRead(io::Result<String>), // io::error::Error is private, so idk how to define this with the error type
    FailedToParse,
}

#[derive(Debug, PartialEq)]
pub enum QueryResponse {
    Movie {
        title: String,
        year: String,
        imdb: String,
    },
    Show {
        title: String,
        start_year: String,
        tmdb: String,
    },
    Episode {
        show_title: String, // duplicate
        // id: String,
    },
    None
}

/// Some movie mappings can be manually override inside `manual-fixes.json`. Some movie file names
/// may not result in the IMDB result that is expected, so this is what this 
pub enum OverrideStatus {
    NotHardcoded,
    Overriden(String),
}
pub fn check_override(movie: &String) -> Result<OverrideStatus, FileIoError> {
    let contents = fs::read_to_string(String::from("manual-fixes.json")).map_err(|e| FileIoError::FailedToRead(Err(e)))?;

    let json = json::parse(&contents).map_err(|_| FileIoError::FailedToParse)?;
    // eprintln!("{} {} {}", json, movie, json[movie]);
    if json[movie].is_null() { return Ok(OverrideStatus::NotHardcoded); }
    let id = json[movie].to_string();
    Ok(OverrideStatus::Overriden(id))
}

pub trait Queryable {
    type Error;
    fn query_api(&mut self, client: &Client) -> Result<Response, Self::Error>;
    fn process_response(&mut self, response: Response) -> Result<(), Self::Error>;
}
impl Queryable for Movie {
    type Error = QueryError;
    fn query_api(&mut self, client: &Client) -> Result<Response, Self::Error> {
        let omdb_key = SECRETS.clone().OMDB_KEY;
        if omdb_key.is_empty() { return Err(QueryError::UnsetApiKey) }

        // It should be safe to call the first `unwrap` because we never call `query_api` unless
        // `Path::is_file` is true, so `file_stem` should never return `None`. We check that the
        // full filepath is unicode before we ever call `Movie::new` in `create_movie`.
        let file_stem = self.src.file_stem().unwrap().to_str().unwrap().to_string();

        // Need to differentiate between searching with a raw string or by its imdb_id
        let imdb_id_re = Regex::new(r"^tt[0-9]+").unwrap(); // TODO: add error checking to make sure the regex pattern is valid
        let search_param = match check_override(&file_stem) {
            Err(err) => file_stem,
            Ok(OverrideStatus::NotHardcoded) => file_stem,
            Ok(OverrideStatus::Overriden(value)) => value,
        };

        let url = if imdb_id_re.is_match(&search_param) { 
               format!("https://www.omdbapi.com/?apikey={}&type=movie&i={}",  omdb_key, search_param) }
        else { format!("https://www.omdbapi.com/?apikey={}&type=movie&s={}*", omdb_key, urlencoding::encode(&search_param)) };
        println!("Movie Url: {url}");
        Ok(client.get(&url).send()?)
    }
    fn process_response(&mut self, response: Response) -> Result<(), Self::Error> {
        let year_re = Regex::new(r"^[0-9]+").unwrap();

        let search_term = self.src.file_stem().unwrap().to_str().unwrap().to_string();

        let raw_json = json::parse(&response.text()?)?;

        if raw_json["Response"] == "False" {
            //? Rate limiting here?
            // TODO: FITFO
            return Err(QueryError::NoSearchResultsFromApi)
        }

        // If the movie was hardcoded, we used a differen route when querying the API, so we parse
        // the json differenly
        let res = check_override(&search_term);
        let json = match res {
            Err(err) => &raw_json["Search"][0],
            Ok(OverrideStatus::NotHardcoded) => &raw_json["Search"][0],
            Ok(OverrideStatus::Overriden(value)) => &raw_json,
        };

        // TODO: rewrite this like `Show::process_response`
        let title = match &json["Title"] {
            json::JsonValue::Null => String::new(),
            _ => json["Title"].to_string()
        };
        let imdb = match &json["imdbID"] {
            json::JsonValue::Null => String::new(),
            _ => json["imdbID"].to_string()
        };
        let year = match &json["Year"] {
            json::JsonValue::Null => String::new(),
            _ => format!("{}", year_re.find_iter(&json["Year"].to_string()).next().unwrap().as_str()) // this is fucking stupid
        };

        if title.is_empty() || imdb.is_empty() || year.is_empty() {
            return Err(QueryError::FailedToQueryApi);
        }

        self.query = QueryResponse::Movie { title, imdb, year, };
        Ok(())
    }
}
impl Queryable for Show {
    type Error = QueryError;
    fn query_api(&mut self, client: &Client) -> Result<Response, Self::Error> {
        let tmdb_key = SECRETS.clone().TMDB_KEY;
        if tmdb_key.is_empty() { return Err(QueryError::UnsetApiKey) }

        let search_query = self.src.file_name().unwrap().to_str().unwrap().to_string();

        let encoded = urlencoding::encode(&search_query);
        let url = format!("https://api.themoviedb.org/3/search/tv?query={}", encoded);
        println!("Show Url:  {url}");
        let builder = client.get(&url)
            .header("Authorization", format!("Bearer {}", tmdb_key))
            .header("accept", "application/json");

        Ok(builder.send()?)
    }
    fn process_response(&mut self, response: Response) -> Result<(), Self::Error> {
        // TMDB response
        let year_re = Regex::new(r"^[0-9]+").unwrap();

        let file_name = self.src.file_name().unwrap().to_str().unwrap().to_string(); 

        let raw_json = json::parse(&response.text()?)?;
        if raw_json["success"].as_bool().unwrap_or(false) {
            return Err(QueryError::InvalidApiKey);
        }
        if raw_json["total_results"].as_u64().unwrap_or(0) == 0 {
            //? Rate limiting here?
            // TODO: FITFO
            return Err(QueryError::NoSearchResultsFromApi);
        }

        let json = &raw_json["results"][0];

        let title = json["name"].to_string();
        let tmdb = json["id"].to_string();
        let start_year = format!("{}", year_re.find_iter(&json["first_air_date"].to_string()).next().unwrap().as_str());

        if title.is_empty() || tmdb.is_empty() || start_year.is_empty() {
            return Err(QueryError::FailedToExtractApiData(raw_json.pretty(2).clone()));
        }

        for iterator in self.episodes.iter() {
            let mut episode = iterator.borrow_mut();
            episode.query = QueryResponse::Episode { show_title: title.clone() }
        }
        self.query = QueryResponse::Show { title, start_year, tmdb };

        Ok(())
    }
}
// impl Queryable for Episode {
//     type Error = QueryError;
//     fn query_api(&mut self, client: &Client) -> Result<Response, Self::Error> {
//         todo!()
//     }
//
//     // fn process_response(&mut self, response: Response) -> Result<(), Self::Error> {
//     //     // We know these are good
//     //     let SE_match_re = Regex::new(r"[sS][0-9]+[eE][0-9]+").unwrap();
//     //     let SE_number_re = Regex::new(r"[0-9]+").unwrap(); // multipurpose regex for season and episode number
//     //     let is_match = move |x: &String, re: &Regex| re.is_match(x);
//     //
//     //     // TODO: Clean up this shit!
//     //     // this is duplicated from `create_episode`
//     //     let path = &self.src;
//     //     let file_path = path.to_str().unwrap().to_string();
//     //
//     //     // TODO: Support Anime numbering
//     //     if !SE_match_re.is_match(&file_path) { return Err(QueryError::FailedToQueryApi); }
//     //
//     //     let first_match = SE_match_re.find(&file_path);
//     //     if first_match.is_none() { return Err(QueryError::FailedToQueryApi); } // just in case, too lazy to scour docs
//     //     let episode_ident = first_match.unwrap().as_str();
//     //
//     //     if SE_number_re.find_iter(&episode_ident).count() != 2 {
//     //         // ! Use `tracing`
//     //         eprintln!("Invalid episode identifier for path: {}", &file_path);
//     //         return Err(QueryError::FailedToQueryApi);
//     //     }
//     //
//     //     let mut iterator = SE_number_re.find_iter(&episode_ident);
//     //     let season_num = iterator.next().unwrap().as_str();
//     //     let episode_num = iterator.next().unwrap().as_str();
//     //     let episode_string = format!("S{}E{}", season_num, episode_num);
//     //
//     //     self.query = QueryResponse::Episode {
//     //         show_title: String::new(),
//     //     };
//     //
//     //     Ok(())
//     // }
// }
