use std::{ fs, io };
use ignore::*;
use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use regex::Regex;
use std::path::Path;

use crate::api_query::Queryable;
use crate::media_item::{ Movie, Episode, Show, Mappable };


//? What if we just expose library functions to query the ENV-defined movies and shows directories and returns owned vectors of their respective types?
//? Or a `generate_catalogue_tree()` function? 
pub enum TreeNode {
    Root(Vec<TreeNode>),
    Category {
        name: String,
        children: Vec<TreeNode>
    },
    Movie(Movie),
    Show(Show),
    Episode(Episode),
}


pub enum TreeGenError {
    EnvVarNotSet,
    EnvVarNotUnicode(OsString),
    MoviesDirDoesntExist,
    ShowsDirDoesntExist,
    RegexSyntaxError(String),
    RegexTooBig(usize),
    RegexUnknown,
}
impl From<std::env::VarError> for TreeGenError {
    fn from(value: std::env::VarError) -> Self {
        match value {
            env::VarError::NotPresent => Self::EnvVarNotSet,
            env::VarError::NotUnicode(osStr) => Self::EnvVarNotUnicode(osStr),
        }
    }
}
impl From<regex::Error> for TreeGenError {
    fn from(value: regex::Error) -> Self {
        match value {
            regex::Error::Syntax(syntax) => Self::RegexSyntaxError(syntax),
            regex::Error::CompiledTooBig(size) => Self::RegexTooBig(size),
            _ => Self::RegexUnknown
        }
    }
}

enum CreateError {
    FailedToCreateMediaItem, // TODO: do more specific
    PathNotUnicode,
}
/// Assumes `movies_dir` exists and is structured correctly
fn create_movie(path: &Path) -> Result<Movie, CreateError> {
    let opt = path.to_str();
    if let None = opt {
        return Err(CreateError::PathNotUnicode);
    }
    let src = opt.unwrap().to_string();

    Err(CreateError::FailedToCreateMediaItem)
}

/// Assumes `shows_dir` exists and is structured correctly
fn create_episode(path: &Path) -> Result<Episode, CreateError> {

    Err(CreateError::FailedToCreateMediaItem)
}
pub fn generate_catalogue_tree() -> Result<TreeNode, TreeGenError> {

    let movies_dir = env::var("SRC_DIR")? + "/movies"; // TODO: don't hardcode these
    let shows_dir  = env::var("SRC_DIR")? + "/shows";  // TODO: don't hardcode these

    if ! Path::new(&movies_dir).exists()
        { return Err(TreeGenError::MoviesDirDoesntExist); }
    if ! Path::new(&shows_dir).exists()
        { return Err(TreeGenError::ShowsDirDoesntExist); }

    let currying_match = |re: Regex, invert: bool| move |x: &DirEntry| { invert ^ re.is_match(x.file_name().to_str().unwrap()) }; // I HAVE NEVER GOTTEN TO USE CURRYING BEFORE!!! LET'S GO
    let ignore_re = Regex::new(r".*(ignore|temp).*")?;
    let video_re = Regex::new(&(env::var("VIDEO_FILE_EXTENTIONS")?))?;
    let mut root = TreeNode::Root(Vec::new());
    
    if let TreeNode::Root(children) = &mut root {
        children.push(TreeNode::Category { name: "Movies".to_string(), children: Vec::new()});
        children.push(TreeNode::Category { name: "Shows".to_string(), children: Vec::new()});
    }

    let mut builder = WalkBuilder::new(movies_dir);
    builder.min_depth(Some(1))
        .max_depth(Some(1))
        .filter_entry(currying_match(ignore_re.clone(), true))
        .filter_entry(currying_match(video_re.clone(), false));
    for result in builder.build() {
        if (result.is_err()) {
            // ! Use `tracing` for logging
            //* Going to be weird. 
            //* Needs a setup call, but we can't do that in the library because then that locks the end user into using exactly that.
            //* Also can't not call it in the library because then we're calling random log functions in the library for no reason.
            //* IDK
            continue;
        }
        let movie = result.unwrap();
        if ! movie.path().is_file() { continue; }
        // let mut m = Movie::new();
        // m.src = movie.path().to_str().unwrap().to_string();
        // m.working_title = movie.file_name().to_str().unwrap().to_string();
        // self.db.movies.push(m);
    }
    drop(builder);

    let mut builder = WalkBuilder::new(shows_dir);
    builder.min_depth(Some(1))
        .max_depth(Some(1))
        .filter_entry(currying_match(ignore_re.clone(), true));

    // for result in builder.build() {
    //     let show = result.unwrap();
    //     if ! show.path().is_dir() { continue; }
    //     let mut out = Show::new();
    //     out.working_title = show.file_name().to_str().unwrap().to_string();
    //     out.root_dir = show.path().to_str().unwrap().to_string();
    //
    //     self.db.shows.push(Rc::new(RefCell::new(out)));
    // }

    Ok(root)
}
