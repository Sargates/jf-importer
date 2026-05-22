use serde::{Serialize, Deserialize};
use std::fs::{self, Permissions, read};
use std::path::PathBuf;
use std::io;
use etcetera::{self, BaseStrategy};
use toml;

use once_cell::sync::Lazy;

pub static CONFIG: Lazy<Config> = Lazy::new(|| {
    Config::load_config()
});
pub static SECRETS: Lazy<Secrets> = Lazy::new(|| {
    Secrets::load_secrets()
});

#[derive(Clone, Debug, PartialEq, Default)]
pub enum ConfigLoadError {
    Success,
    /// Not necessarily an error
    #[default]
    RevertedToDefault,
    FailedToFindHomeDir,
    FileDoesNotExist,
    FailedToReadFile,
    FailedToDeserialize
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(skip_serializing, skip_deserializing)]
    pub source_file: PathBuf,

    #[serde(skip_serializing, skip_deserializing)]
    pub load_error: ConfigLoadError,

    /// Regex pattern for match any and all video files; movies and shows
    /// Configurable via the config in case I missed something.
    pub VideoFileExtensions: String,

    /// Assuming Linux, this is `$HOME/.cache/jf-importer`.
    /// Windows should* be at `C:\Users\<you>\Appdata\Roaming\jf-importer`
    /// TODO: support non-linux
    pub CacheDir: PathBuf,

    /// Directory prefix for movie and show directory names
    /// As in:
    ///   Movies are found at `SrcDir.join(SrcMovieSubDir)`
    ///   Shows are found at `SrcDir.join(SrcShowsSubDir)`
    /// Both are absolute paths
    /// Default: Unset
    pub SrcBaseDir: PathBuf,

    /// Source subdirectory name for Movies
    /// Default: "movies"
    pub SrcMovieSubDir: String,

    /// Source subdirectory name for TV Shows
    /// Default: "shows"
    pub SrcShowSubDir: String,

    /// Directory prefix for Jellyfin root directory
    /// As in:
    ///   Movies are moved to `DstDir.join(JfMovieDir)`
    ///   Shows are moved to `DstDir.join(JfShowsDir)`
    /// Both are absolute paths
    /// Default: Unset
    pub DstBaseDir: PathBuf,

    /// Movie subdirectory for the destination:
    /// Default: "movies"
    pub JfMovieDir: String,

    /// Movie subdirectory for the destination:
    /// Default: "tvseries"
    pub JfShowsDir: String,
}
impl Config {
    fn load_config() -> Config {
        let base = etcetera::choose_base_strategy().map_err(|_| ConfigLoadError::FailedToFindHomeDir);
        if let Err(e) = base {
            let mut cfg: Config = Config::default_config();
            cfg.load_error = e;
            return cfg;
        }
        let base = base.unwrap();
        let default = PathBuf::new();
        let CacheDir = base.cache_dir().join("jf-importer");

        let cfg_file = base.config_dir().join("jf-importer").join("config.toml");
        if ! cfg_file.exists() { return Err(ConfigLoadError::FileDoesNotExist).into(); }

        // println!("Reading File: {}", cfg_file.to_string_lossy());
        let content = match fs::read_to_string(cfg_file) {
            Err(e) => return Err(ConfigLoadError::FailedToReadFile).into(),
            Ok(content) => content
        };

        let mut cfg = match toml::from_str::<Config>(&content) {
            Err(err) => Err(ConfigLoadError::FailedToDeserialize).into(),
            Ok(cfg) => cfg
        };
        cfg.load_error = ConfigLoadError::Success;
        cfg
    }
    fn default_config() -> Config {
        let default = PathBuf::new();
        let base = etcetera::choose_base_strategy().map_err(|_| ConfigLoadError::FailedToFindHomeDir);
        let VideoFileExtensions = "(webm|mp4|mov|mkv|m4v|avi)".to_string();
        let SrcBaseDir   = PathBuf::from("/path/to/sources");
        let DstBaseDir   = PathBuf::from("/path/to/jellyfin");

        if let Err(err) = base {
            let mut cfg: Config = Default::default();
            cfg.load_error = err;
            return cfg;
        }
        let base = base.unwrap();
        let CacheDir = base.cache_dir().join("jf-importer");
        let cfg_file = base.config_dir().join("jf-importer").join("config.toml");

        Config {
            source_file: cfg_file, load_error: ConfigLoadError::RevertedToDefault,
            VideoFileExtensions, CacheDir,
            SrcBaseDir, DstBaseDir,
            SrcMovieSubDir: "movies".to_string(), SrcShowSubDir: "shows".to_string(),
            JfMovieDir: "movies".to_string(),     JfShowsDir: "tvseries".to_string(),
        }
    }
    pub fn write_to_file(&self) -> io::Result<()> {
        fs::write(self.source_file.as_path(), toml::to_string(&self).unwrap());
        Ok(())
    }
}
impl From<Result<Config, ConfigLoadError>> for Config {
    fn from(value: Result<Config, ConfigLoadError>) -> Self {
        match value {
            Ok(cfg) => cfg,
            Err(err) => {
                let mut cfg: Config = Config::default_config();
                cfg.load_error = err;
                return cfg;
            }
        }
    }
}
impl PartialEq for Config {
    fn ne(&self, other: &Self) -> bool { !self.eq(other) }
    fn eq(&self, other: &Self) -> bool { self.VideoFileExtensions == other.VideoFileExtensions && self.CacheDir == other.CacheDir && self.SrcBaseDir == other.SrcBaseDir && self.SrcMovieSubDir == other.SrcMovieSubDir && self.SrcShowSubDir == other.SrcShowSubDir && self.DstBaseDir == other.DstBaseDir && self.JfMovieDir == other.JfMovieDir && self.JfShowsDir == other.JfShowsDir }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Secrets {
    #[serde(skip_serializing, skip_deserializing)]
    pub source_file: PathBuf,

    #[serde(skip_serializing, skip_deserializing)]
    pub load_error: ConfigLoadError,

    pub OMDB_KEY: String,
    pub TMDB_KEY: String,
}
impl Secrets {
    pub fn load_secrets() -> Secrets {
        let base = etcetera::choose_base_strategy().map_err(|_| ConfigLoadError::FailedToFindHomeDir);
        if let Err(e) = base {
            let mut secrets: Secrets = Secrets::default();
            secrets.load_error = e;
            return secrets;
        }
        let base = base.unwrap();
        let secrets_file = base.config_dir().join("jf-importer").join("secrets.toml");
        if ! secrets_file.exists() { return Err(ConfigLoadError::FileDoesNotExist).into(); }
        
        let content = match fs::read_to_string(secrets_file) {
            Err(e) => return Err(ConfigLoadError::FailedToReadFile).into(),
            Ok(content) => content
        };

        let mut secrets = match toml::from_str::<Secrets>(&content) {
            Err(err) => Err(ConfigLoadError::FailedToDeserialize).into(),
            Ok(cfg) => cfg
        };
        secrets.load_error = ConfigLoadError::Success;
        secrets
    }
    pub fn default() -> Secrets {
        let base = etcetera::choose_base_strategy().map_err(|_| ConfigLoadError::FailedToFindHomeDir);
        if let Err(e) = base {
            let mut cfg: Secrets = Secrets::default();
            cfg.load_error = e;
            return cfg;
        }
        let base = base.unwrap();
        let secrets_file = base.config_dir().join("jf-importer").join("secrets.toml");
        if ! secrets_file.exists() { return Err(ConfigLoadError::FileDoesNotExist).into(); }
        Secrets{
            source_file: secrets_file,
            load_error: ConfigLoadError::RevertedToDefault,
            OMDB_KEY: "".to_string(),
            TMDB_KEY: "".to_string()
        } 
    }
}
impl From<Result<Secrets, ConfigLoadError>> for Secrets {
    fn from(value: Result<Secrets, ConfigLoadError>) -> Self {
        match value {
            Ok(cfg) => cfg,
            Err(err) => {
                let mut cfg: Secrets = Secrets::default();
                cfg.load_error = err;
                return cfg;
            }
        }
    }
}

// fn new_api_cache_file() -> PathBuf {
//     CONFIG.CacheDir.join("NewFile")
// }

// fn main() {
//     let file = "Test.toml";
//     let conf = CONFIG.clone();
//     let string: String = toml::to_string(&conf).unwrap();
//     
//     if let Err(err) = fs::write(file, string) {
//         println!("Failed to write file! Error: {}", err);
//         return;
//     }
//
//     let content = fs::read_to_string(file).unwrap();
//     let from_file: Config = toml::from_str(&content).unwrap();
//
//     assert!(conf == from_file);
//     println!("{:#?}", from_file);
//     println!("New Cache File: {:?}", new_api_cache_file());
// }

