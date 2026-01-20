//* Shitty temporary name for file
use std::fs;
use std::error::Error;
use regex::Regex;
use std::path::Path;
use std::ops::DerefMut;
use std::rc::Rc;
use reqwest::blocking::{Response, Client, Request, RequestBuilder};

use std::{
    // error::Error,
    io::{Read, Write},
    process::{Command, Stdio},
};


use crate::types::*;

pub trait MoveableItem { // polymorphism for Movie and Episode
    fn src(&self) -> String;
    // fn r#type(&self) -> MoveType;
    fn mapped_path(&self, dst_dir: &String) -> Result<String,Box<dyn Error>>;
}
impl MoveableItem for Movie {
    fn src(&self) -> String { self.src.clone() }
    // fn r#type(&self) -> MoveType { MoveType::Movie }
    fn mapped_path(&self, dst_dir: &String) -> Result<String,Box<dyn Error>> {
        let path_obj = Path::new(&self.src); //? how does a `new` method return a reference? Where the fuck is it stored then???
                                             // TODO: FITFO
        let extension: String;
        if let Some(ext) = path_obj.extension() { extension = ext.to_string_lossy().to_string(); } 
        else                                    { extension = String::new(); }

        let mov_dir = format!("{} ({}) [{}]", self.title, self.year, self.imdb);
        let mov_file = format!("{} ({}) [{}].{}", self.title, self.year, self.imdb, extension); // TODO: Add tagging support
        
        let out = format!("{}/{}/{}", dst_dir, mov_dir, mov_file);
        
        Ok(out)
    }
}
impl MoveableItem for Episode {
    fn src(&self) -> String { self.src.clone() }
    // fn r#type(&self) -> MoveType { MoveType::Episode }
    fn mapped_path(&self, dst_dir: &String) -> Result<String,Box<dyn Error>> {
        let path_obj = Path::new(&self.src); //? how does a `new` method return a reference? Where the fuck is it stored then???
                                             // TODO: FITFO

        let extension: String;
        if let Some(ext) = path_obj.extension() { extension = ext.to_string_lossy().to_string(); } 
        else                                    { extension = String::new(); }


        let ident_re  = Regex::new("S[0-9][0-9]E[0-9][0-9]")?;
        let ep_ident: String;
        if let Some(m) = ident_re.find(&self.src) { ep_ident = m.as_str().into(); }
        else                                      { return Err(err!("Failed to find episode identifier `SXXEXX` in file name: {}", self.src)); }

        let season_re = Regex::new("[0-9][0-9]")?;
        // if let Ok(re) =  { season_re = re; } //* `regex` crate doesn't have support for lookarounds :(
        // else                                     { eprintln!("Invalid Regex");
                                                   // return Err(err!("Bad Regex")); }

        let season: String;
        if let Some(m) = season_re.find(&ep_ident) { season = m.as_str().into(); }
        else                                       { return Err(err!("Failed to find season info from `ep_ident`={}", ep_ident)); }

        let show_ref = self.parent.borrow();
        let show_dir = format!("{} ({}) [tbdbid-{}]", show_ref.title, show_ref.start_year, show_ref.tmdb);
        let show_season_dir = format!("Season {}", season);
        let episode_dst = format!("{} {}.{}", show_dir, self.id, extension);

        let out = format!("{}/{}/{}/{}", dst_dir, show_dir, show_season_dir, episode_dst);
        Ok(out)
    }
}
pub trait ApiItem { // TODO: Rename this
    fn make_api_call(&self, client: &Client, omdb_key: &String) -> reqwest::Result<Response>;
    fn update_info(&mut self, res: Response) -> Result<(), Box<dyn Error>>;
}
impl ApiItem for Movie {
    fn make_api_call(&self, client: &Client, omdb_key: &String) -> reqwest::Result<Response> {
        let search_query = Path::new(&self.src).file_stem().unwrap().to_str().unwrap().to_string();

        Api::query_for_movie(&search_query, client, omdb_key)
    }
    fn update_info(&mut self, res: Response) -> Result<(), Box<dyn Error>> {
        let year_re = Regex::new(r"^[0-9]+")?;

        let search_term = Path::new(&self.src).file_stem().unwrap().to_str().unwrap().to_string();
        let hardcoded: bool;
        if let Ok(res) = Api::check_override(&search_term) { hardcoded = true; }
        else                                          { hardcoded = false; }

        // let final_url = res.url().to_string().clone();

        let raw_json = json::parse(&res.text()?)?;

        if raw_json["Response"] == "False" {
            return Err(err!("API Failure for movie: {}", search_term))
        }

        let json;
        if hardcoded { json = &raw_json;              } // response changes if we search directly w/ an imdb id or a string
        else         { json = &raw_json["Search"][0]; }

        self.title = json["Title"].to_string();
        self.imdb = json["imdbID"].to_string();
        let year = json["Year"].to_string();
        self.year = format!("{}", year_re.find_iter(&year).next().unwrap().as_str()); // this is fucking stupid
        Ok(())
    }
}
impl ApiItem for Show {
    fn make_api_call(&self, client: &Client, tmdb_key: &String) -> reqwest::Result<Response> {
        let search_query = Path::new(&self.root_dir).file_name().unwrap().to_str().unwrap().to_string();
        Api::query_for_show(&search_query, &client, tmdb_key)
    }
    fn update_info(&mut self, res: Response) -> Result<(), Box<dyn Error>> {
        let year_re = Regex::new(r"^[0-9]+")?;

        let file_name = Path::new(&self.root_dir).file_name().unwrap(); 
        // TODO: Fix.
        //   panics if path terminates in `..`

        let raw_json = json::parse(&res.text()?)?;
        if raw_json["total_results"].as_u64().unwrap_or(0) == 0 {
            return Err(err!("Got no result for movie: {}", file_name.to_str().unwrap().to_string()));
        }

        let json = &raw_json["results"][0];
        self.title = json["name"].to_string();
        self.tmdb = json["id"].to_string();
        let year = json["first_air_date"].to_string();
        self.start_year = format!("{}", year_re.find_iter(&year).next().unwrap().as_str()); // this is fucking stupid
        Ok(())
    }
}

fn check_hostname() -> Result<(), Box<dyn Error>> {
    let mut proc = Command::new("hostname").spawn()?;
    let mut stdout = proc.stdout.take().unwrap();

    while proc.try_wait()?.is_none() {} // block until process is finished

    let mut buf = [0u8; 253]; // 253 is the max hostname allowed by DNS. source: https://www.reddit.com/r/linuxquestions/comments/k8y5ek/comment/gf108ft/   
    let len = stdout.read(&mut buf)?;

    let hostname = std::str::from_utf8(&buf[..len])?;

    match hostname {
        "homelab" => { Ok(()) }, // TODO: don't hardcode this
        _         => { Err(err!("Invalid host name. This is to prevent running on the wrong device for testing purposes.")) }
    }
}

impl App {
    pub fn move_file(&self) -> Result<i32, Box<dyn Error>> {
        check_hostname()?;
        // Assert that the dst_dir exists
        if let Err(err) = fs::exists(self.env.dst_dir.clone()) {
            return Err(BasicError::boxed(dbg!(err).to_string()));
        }

        // For every file:
        //   Make sure every src file exists
        //   fs::rename(src, dst);
        Ok(0)
    }
}
