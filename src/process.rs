//* Shitty temporary name for file
use std::error::Error;
use regex::Regex;
use std::path::Path;
use std::ops::DerefMut;
use std::rc::Rc;

use crate::types::*;

pub struct Move { // Represents the act of "moving" a movie/episode
    src: String,
    dst: String,
    r#type: MoveType,
}
pub enum MoveType { Invalid, Episode, Movie }
pub trait VideoFile { // polymorphism for Movie and Episode
    fn src(&self) -> String;
    fn r#type(&self) -> MoveType;
    fn mapped_path(&self, dst_dir: &String) -> Result<String,Box<dyn Error>>;
}
impl VideoFile for Episode {
    fn src(&self) -> String { self.src.clone() }
    fn r#type(&self) -> MoveType { MoveType::Episode }
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
impl VideoFile for Movie {
    fn src(&self) -> String { self.src.clone() }
    fn r#type(&self) -> MoveType { MoveType::Movie }
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

impl Move {
    pub fn from<T: VideoFile>(e: &dyn VideoFile, dst: String) -> Self {
        Move { src: e.src(),
               dst: dst.clone(),
               r#type: e.r#type() }
    }
}
impl App {
    pub fn move_file() {
        // fs::rename(src, dst);
    }
}
