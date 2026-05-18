use std::{ fmt, fs, io };
use ignore::*;
use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use regex::Regex;
use std::path::{ Path, PathBuf };

use crate::api_query::{ Queryable, QueryResponse };
use crate::media_item::{ Movie, Show, Episode, Mappable };


//? What if we just expose library functions to query the ENV-defined movies and shows directories and returns owned vectors of their respective types?
//? Or a `generate_catalogue_tree()` function? 
#[derive(Debug)]
pub enum TreeNode {
    Root(Vec<TreeNode>),
    Category {
        name: String,
        children: Vec<TreeNode>
    },
    Movie(Movie),
    Show {
        show: Show,
        children: Vec<TreeNode>
    },
    Episode(Episode),
    Fail {
        err: CreateError, 
        buf: PathBuf
    },
}
impl TreeNode {
    pub fn children_mut(&mut self) -> &mut Vec<TreeNode> {
        match self {
            TreeNode::Root(children) => { children }
            TreeNode::Category{ name, children } => { children }
            TreeNode::Show{ show, children } => { children }
            _ => panic!("Expected variant with children")
        }
    }
    pub fn push_child(&mut self, child: TreeNode) {
        match self {
            TreeNode::Root(children) => { children.push(child); }
            TreeNode::Category{ name, children } => { children.push(child); }
            TreeNode::Show{ show, children } => { children.push(child); }
            _ => panic!("Expected variant with children")
        }
    }
    pub fn is_queried(&self) -> bool {
        match self {
            TreeNode::Show{ show, .. } => { show.query != QueryResponse::None }
            TreeNode::Movie(movie)     => { movie.query != QueryResponse::None }
            TreeNode::Episode(episode) => { episode.query != QueryResponse::None }
            _ => panic!("Expected Movie, Show, or Episode!")
        }
    }
}
impl fmt::Display for TreeNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TreeNode::Root(children) => {
                write!(f, ".")
            }
            TreeNode::Category{ name, children } => {
                write!(f, "{name}")
            }
            TreeNode::Show{ show, children } => {
                match &show.query {
                    QueryResponse::Show{title, start_year, tmdb} => 
                        write!(f, "{title} ({start_year}) [tmdbid-{tmdb}]"),
                    QueryResponse::None => write!(f, "{}", show.src),
                    _ => panic!("Show has invalid query response!")
                }
            }
            TreeNode::Movie(movie) => {
                match &movie.query {
                    QueryResponse::Movie{title, year, imdb} => 
                        write!(f, "{title} ({year}) [imdbid-{imdb}]"),
                    QueryResponse::None => write!(f, "{}", movie.src),
                    _ => panic!("Show has invalid query response!")
                }
            }
            TreeNode::Episode(ep) => {
                match &ep.query {
                    QueryResponse::Episode{show_title, id} => 
                        write!(f, "{show_title} {id}"),
                    QueryResponse::None => write!(f, "{}", ep.src),
                    _ => panic!("Show has invalid query response!")
                }
            }
            TreeNode::Fail{ err, buf } => {
                write!(f, "Parse Failure: {buf:?}")
            }
        }
    }
}

#[derive(Debug)]
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
            env::VarError::NotUnicode(os_string) => Self::EnvVarNotUnicode(os_string),
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

#[derive(Debug)]
pub enum CreateError {
    FailedToCreateMediaItem, // TODO: do more specific
    PathNotUnicode,
    IncorrectFileTypeSupplied,
    EpisodeIncorrectFormat,
    EpisodeFailedToParseSeason,
}

// TODO: Move these to the actual `new` constructor; no need for these to be separate
/// Assumes `movies_dir` exists and is structured correctly
/// Benefit of doing it this way is that tests are easier to write
fn create_movie(path: &Path) -> Result<TreeNode, CreateError> {
    let opt = path.to_str();
    if let None = opt   { return Err(CreateError::PathNotUnicode); }
    if ! path.is_file() { return Err(CreateError::IncorrectFileTypeSupplied); }
    let src = opt.unwrap().to_string();
    let out = Movie::new(src);
    
    Ok(TreeNode::Movie(out))
}
fn create_show(path: &Path) -> Result<TreeNode, CreateError> {
    let opt = path.to_str();
    if let None = opt  { return Err(CreateError::PathNotUnicode); }
    if ! path.is_dir() { return Err(CreateError::IncorrectFileTypeSupplied) }
    let src = opt.unwrap().to_string();
    let out = Show::new(src);
    
    Ok(TreeNode::Show{ show: out, children: Vec::new() })
}
fn create_episode(path: &Path) -> Result<TreeNode, CreateError> {
    // Most of this is grandfathered from previous version. This code may be shit
    let opt = path.to_str();
    if let None = opt   { return Err(CreateError::PathNotUnicode); }
    if ! path.is_file() { return Err(CreateError::IncorrectFileTypeSupplied); }

    // We know these are good
    let SE_match_re = Regex::new(r"[sS][0-9]+[eE][0-9]+").unwrap();
    let SE_number_re = Regex::new(r"[0-9]+").unwrap(); // multipurpose regex for season and episode number
    let is_match = move |x: &String, re: &Regex| re.is_match(x);

    let file_path = path.to_str().unwrap().to_string();

    // TODO: Support Anime numbering
    if !SE_match_re.is_match(&file_path) { return Err(CreateError::EpisodeIncorrectFormat); }

    let first_match = SE_match_re.find(&file_path);
    if first_match.is_none() { return Err(CreateError::EpisodeIncorrectFormat); } // just in case, too lazy to scour docs
    let episode_ident = first_match.unwrap().as_str();

    if SE_number_re.find_iter(&episode_ident).count() != 2 {
        // ! Use `tracing`
        eprintln!("Invalid episode identifier for path: {}", &file_path);
        return Err(CreateError::EpisodeIncorrectFormat);
    }

    let mut iterator = SE_number_re.find_iter(&episode_ident);
    let season_num = iterator.next().unwrap().as_str();
    let episode_num = iterator.next().unwrap().as_str();
    let episode_string = format!("S{}E{}", season_num, episode_num);

    let src = opt.unwrap().to_string();
    let mut out = Episode::new(src);
    
    Ok(TreeNode::Episode(out))
}

pub fn generate_catalog_tree() -> Result<TreeNode, TreeGenError> {
    let movies_dir = env::var("SRC_DIR")? + "/movies"; // TODO: don't hardcode these subdirectories
    let shows_dir  = env::var("SRC_DIR")? + "/shows";  // TODO: don't hardcode these subdirectories

    if ! Path::new(&movies_dir).exists()
        { return Err(TreeGenError::MoviesDirDoesntExist); }
    if ! Path::new(&shows_dir).exists()
        { return Err(TreeGenError::ShowsDirDoesntExist); }

    // I HAVE NEVER GOTTEN TO USE CURRYING BEFORE!!! LET'S GO
    let currying_match = |re: Regex, invert: bool| 
        move |x: &DirEntry| { invert ^ re.is_match(x.file_name().to_str().unwrap()) };

    let ignore_re = Regex::new(r".*(ignore|temp).*")?;
    let video_re = Regex::new(&(env::var("VIDEO_FILE_EXTENTIONS")?))?;
    
    // VIDEO_FILE_EXTENTIONS='(webm|mp4|mov|mkv|m4v|avi)'
    println!("{}", video_re.is_match("gaming.mp4"));
    println!("{}", video_re.is_match("gaming.mov"));
    println!("{}", video_re.is_match("gaming.webm"));
    println!("{}", video_re.is_match("gaming.sigma"));
    println!("{}", video_re.is_match("gaming.avi"));
    println!("{}", video_re.is_match("gaming.m4v"));

    let SE_match_re = Regex::new(r"[sS][0-9]+[eE][0-9]+")?;
    let SE_number_re = Regex::new(r"[0-9]+")?; // multipurpose regex for season and episode number
    let is_match = move |x: &String, re: &Regex| re.is_match(x);

    let mut root = TreeNode::Root(Vec::new());
    let mut movies_cat: TreeNode = TreeNode::Category { name: String::from("Movies"), children: Vec::new()};
    let mut shows_cat:  TreeNode = TreeNode::Category { name: String::from("Shows"),  children: Vec::new()};
    let mut fails_cat:  TreeNode = TreeNode::Category { name: String::from("Failures"),  children: Vec::new()};

    let mut builder = WalkBuilder::new(movies_dir);
    builder.min_depth(Some(1))
        .max_depth(Some(1))
        .filter_entry(currying_match(ignore_re.clone(), true))
        .filter_entry(currying_match(video_re.clone(), false));
    for result in builder.build() {
        if let Err(err) = result {
            // ! Use `tracing` for logging
            //* Going to be weird. 
            //* Needs a setup call, but we can't do that in the library because then that locks the end user into using exactly that.
            //* Also can't not call the setup function in the library because then we're calling random log functions in the library for no reason.
            //* IDK
            println!("Bad Dir Entry {}", err);
            continue;
        }
        let dir_entry = result.unwrap();
        if ! dir_entry.path().is_file() { continue; }

        let movies = movies_cat.children_mut();
        let create_result = create_movie(dir_entry.path());
        if let Err(err) = create_result {
            let fails = fails_cat.children_mut();
            let buf = dir_entry.path().to_path_buf();
            fails.push(TreeNode::Fail{err, buf});
            continue;
        }
        let movie_node = create_result.unwrap();
        movies.push(movie_node);
    }
    drop(builder);


    let mut builder = WalkBuilder::new(shows_dir);
    builder.min_depth(Some(1))
        .max_depth(Some(1))
        .filter_entry(currying_match(ignore_re.clone(), true));
    for result in builder.build() {
        if let Err(err) = result {
            // ! Use `tracing` for logging
            //* Going to be weird. 
            //* Needs a setup call, but we can't do that in the library because then that locks the end user into using exactly that.
            //* Also can't not call the setup function in the library because then we're calling random log functions in the library for no reason.
            //* IDK
            println!("Bad Dir Entry {}", err);
            continue;
        }
        let dir_entry = result.unwrap();
        if ! dir_entry.path().is_dir() { continue; }

        let shows = shows_cat.children_mut();
        let create_result = create_show(dir_entry.path());
        if let Err(err) = create_result {
            let fails = fails_cat.children_mut();
            let buf = dir_entry.path().to_path_buf();
            fails.push(TreeNode::Fail{err, buf});
            continue;
        }
        let show_node = create_result.unwrap();
        shows.push(show_node);
    }
    drop(builder);

    for show_node in shows_cat.children_mut() {
        if let TreeNode::Show{show, children: episode_nodes} = show_node {
            println!("Processing Show: {}", &show.src);
            let mut builder = WalkBuilder::new(&show.src);
            builder.min_depth(Some(1))
                .sort_by_file_path(|a, b| a.cmp(b)); // a < b
            for result in builder.build() {
                if let Err(err) = result {
                    // ! Use `tracing` for logging
                    //* Going to be weird. 
                    //* Needs a setup call, but we can't do that in the library because then that locks the end user into using exactly that.
                    //* Also can't not call the setup function in the library because then we're calling random log functions in the library for no reason.
                    //* IDK
                    println!("Bad Dir Entry {}", err);
                    continue;
                }
                let dir_entry = result.unwrap();
                if ! dir_entry.path().is_file() { continue; } // skip directories
                
                let create_result = create_episode(dir_entry.path());
                if let Err(err) = create_result {
                    let fails = fails_cat.children_mut();
                    let buf = dir_entry.path().to_path_buf();
                    fails.push(TreeNode::Fail{err, buf});
                    continue;
                }
                let episode_node = create_result.unwrap();
                episode_nodes.push(episode_node);
            }
        }
    }

    if let TreeNode::Root(children) = &mut root {
        children.push(movies_cat);
        children.push(shows_cat);
        children.push(fails_cat);
    }

    Ok(root)
}
