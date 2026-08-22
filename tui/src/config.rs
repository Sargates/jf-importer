// This handle loading a 
use jfi::config::*;

use std::fs;
use etcetera::*;
use std::path::PathBuf;

use serde::{Serialize, Deserialize};


/* TODO: Be able to edit the config
 * This would happen at the app level. Creating a `jfi::Config` is relatively
 * cheap and can be done dynamically. i.e. you edit the `TuiConfig` object,
 * generate a `jfi::Config` type and pass that to one or multiple `CatalogBuilder`s
 * 
 * keeping
 *   /// Is `None` iff the Default configuration is unable to locate the user's
 *   /// home directory. This only affects writing to the file as there's no way
 *   /// to know where it should write to.
 *   #[serde(skip_serializing, skip_deserializing)]
 *   pub source_file: Option<PathBuf>,
 */

#[derive(Debug)]
pub enum TuiConfigCreationError {
    FailedToFindHomeDir,
    FileDoesNotExist,
    FailedToReadFile,
    FailedToDeserialize,
}

// Config matching schema of config file
#[derive(Serialize, Deserialize)]
pub struct JfiConfig {
    VideoFileExtensions: String,
    CacheDir: Option<PathBuf>,
    IngestBaseDir: Option<PathBuf>,
    IngestMovieDir: PathBuf,
    IngestShowDir: PathBuf,
    JfBaseDir: Option<PathBuf>,
    JfMovieDir: PathBuf,
    JfShowDir: PathBuf,
}

impl JfiConfig {
    pub fn load_config() -> Result<JfiConfig, TuiConfigCreationError> {
        let base = choose_base_strategy().map_err(|_| TuiConfigCreationError::FailedToFindHomeDir)?;

        // $HOME/.config/jf-importer
        let cfg_file = base.config_dir().join("jf-importer").join("config.toml");
        cfg_file.exists().ok_or(TuiConfigCreationError::FileDoesNotExist)?;

        // $HOME/.cache/jf-importer
        let cache_dir = base.cache_dir().join("jf-importer");

        let content = fs::read_to_string(&cfg_file).map_err(|_| TuiConfigCreationError::FailedToReadFile)?;
        let mut cfg = toml::from_str::<JfiConfig>(&content).map_err(|_| TuiConfigCreationError::FailedToDeserialize)?;

        //* We check if the config file's `IngestBaseDir` and `JfBaseDir` are
        //* populated and, if so, concatenate it with the respective movie and
        //* show dirs. Otherwise, we interpret the respective movie and show
        //* dirs as absolute paths.

        Ok(cfg)
    }

    /// The only way this returns None is if we fail to locate the
    /// home directory. Otherwise, it's a default config that will
    /// likely fail if used to generate a config.
    pub fn default_config() -> Option<JfiConfig> {
        let default = PathBuf::new();
        let base = etcetera::choose_base_strategy().ok()?;
        let VideoFileExtensions = "(webm|mp4|mov|mkv|m4v|avi)".to_string();
        let SrcBaseDir = PathBuf::from("/path/to/sources");
        let DstBaseDir = PathBuf::from("/path/to/jellyfin");

        let CacheDir = Some(base.cache_dir().join("jf-importer"));
        // $HOME/.config/jf-importer/config.toml
        let cfg_file = base.config_dir().join("jf-importer").join("config.toml");

        Some(JfiConfig {
            VideoFileExtensions,
            CacheDir,
            IngestBaseDir: Some(SrcBaseDir),         JfBaseDir: Some(DstBaseDir),
            IngestMovieDir: PathBuf::from("movies"), IngestShowDir: PathBuf::from("shows"),
            JfMovieDir: PathBuf::from("movies"),     JfShowDir: PathBuf::from("tvseries"),
        })
    }

    // * Since we're changing how the config file is loaded, this need to be 
    // * rewritten to accommodate
    // pub fn write_to_file(&self) -> io::Result<()> {
    //     // if let Some(file) = self.source_file {
    //     //     // fs::write(file.as_path(), toml::to_string(&self.).unwrap());
    //     // }
    //     // Ok(())
    // }
}

impl TryInto<jfi::config::Config> for JfiConfig {
    type Error = jfi::config::ConfigCreationError;
    fn try_into(self) -> Result<jfi::config::Config, Self::Error> {

        // Assemble the source movies directory.
        let ingest_movies = match &self.IngestBaseDir {
            Some(buf) => buf.join(&self.IngestMovieDir).exists().then_some(buf.join(&self.IngestMovieDir)),
            None => self.IngestMovieDir.exists().then_some(self.IngestMovieDir)
        }.ok_or(ConfigCreationError::IngestMovieDirDoesntExist)?;

        // Assemble the source shows directory.
        let ingest_shows = match &self.IngestBaseDir {
            Some(buf) => buf.join(&self.IngestShowDir).exists().then_some(buf.join(&self.IngestShowDir)),
            None => self.IngestShowDir.exists().then_some(self.IngestShowDir)
        }.ok_or(ConfigCreationError::IngestShowDirDoesntExist)?;

        // Assemble the jellyfin movies directory.
        let jf_movies = match &self.JfBaseDir {
            Some(buf) => buf.join(&self.JfMovieDir).exists().then_some(buf.join(&self.JfMovieDir)),
            None => self.JfMovieDir.exists().then_some(self.JfMovieDir)
        }.ok_or(ConfigCreationError::DstMovieDirDoesntExist)?;

        // Assemble the jellyfin shows directory.
        let jf_shows = match &self.JfBaseDir {
            Some(buf) => buf.join(&self.JfShowDir).exists().then_some(buf.join(&self.JfShowDir)),
            None => self.JfShowDir.exists().then_some(self.JfShowDir)
        }.ok_or(ConfigCreationError::DstShowDirDoesntExist)?;

        let cfg = Config {
            CacheDir: self.CacheDir,
            VideoFileExtensions: self.VideoFileExtensions,
            IngestMovieDir: ingest_movies, IngestShowDir: ingest_shows,
            JfMovieDir: jf_movies, JfShowDir: jf_shows,
        };

        Ok(cfg)
    }
}
