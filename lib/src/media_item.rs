use std::fmt;
use std::path::Path;

trait ApiQuery {

}

//# I don't want to have to deal with invalid Movie or Episode objects
// TODO: Guarantee a Movie/Episode is valid before creating one
// fn new(path: &str) -> Result(Movie, Self::Error);
// Check that `new` succeeded and skip if it didn't
pub struct Movie {
    pub title: String,
    pub year: String,
    pub imdb: String,
    pub src: String,
}
impl Movie {
    pub fn new(src: String) -> Self {

        Movie {
            title: String::new(),
            year: String::new(),
            imdb: String::new(),
            src
        }
    }
    pub fn is_queried(&self) {

    }
}
impl fmt::Debug for Movie {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} ({}) [{}] - ({})", self.title, self.year, self.imdb, self.src)
    }
}

pub struct Show {
    pub title: String,
    pub src: String,
    pub start_year: String,
    pub tmdb: String,
}
impl fmt::Debug for Show {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{} ({}) [{}]", self.title, self.start_year, self.tmdb)?;
        // if self.title.is_empty() { // Can't return `Err` in this case. See https://doc.rust-lang.org/std/fmt/trait.Debug.html#errors
        //     writeln!(f, "{} [Initialization Failure]", self.working_title);
        // } else {
        //     for e in self.episodes.iter() { writeln!(f, "{:#?}", e); }
        // }
        Ok(())
    }
}

pub struct Episode {
    pub id: String, // Ex: S01E12
    pub show: String, // Parent Show Title (dup)
    pub src: String,
}
impl fmt::Debug for Episode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} -> {}", self.id, self.src)?; 
        Ok(())
    }
}

pub trait Movable {
    type Error;
    /// `move` wave taken :/
    fn relocate(&self) -> Result<(),Self::Error>;
}

#[derive(Debug, Clone)]
pub enum MappingError {
    FailedToMapPath
}

pub trait Mappable {
    type Error;
    fn mapped_path(&self, src: &str, dst_dir: &str) -> Result<String,Self::Error>;
}

impl Mappable for Movie {
    type Error = MappingError;
    fn mapped_path(&self, src: &str, dst_dir: &str) -> Result<String,Self::Error> {
        todo!()
    }
}

impl Mappable for Episode {
    type Error = MappingError;
    fn mapped_path(&self, src: &str, dst_dir: &str) -> Result<String,Self::Error> {
        todo!()
    }
}
