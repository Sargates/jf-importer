use serde::{Serialize, Deserialize};
use std::fs::{self, Permissions, read};
use std::path::PathBuf;
use std::io;
use etcetera::{self, BaseStrategy};
use toml;

use once_cell::sync::Lazy;

// pub static CONFIG: Lazy<Config> = Lazy::new(|| {
//     Config::load_config()
// });
pub static SECRETS: Lazy<Secrets> = Lazy::new(|| {
    Secrets::load_secrets()
});

// I'm implementing PartialEq for these are all `unit-like`
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum SecretsLoadError {
    Success,

    /// Not necessarily an error
    #[default]
    RevertedToDefault,

    FailedToFindHomeDir,
    FileDoesNotExist,
    FailedToReadFile,
    FailedToDeserialize
}

#[derive(Clone, Debug)]
pub enum ConfigCreationError {
    IngestBaseDirDoesntExist,
    IngestMovieDirDoesntExist,
    IngestShowDirDoesntExist,

    DstBaseDirDoesntExist,
    DstMovieDirDoesntExist,
    DstShowDirDoesntExist,
}

/// Represents a valid configuration file. All checks are performed 
/// eagerly at config creation. Thus, if a `Config` exists, it is valid.
///
/// # Implementors
/// There is a danger of overwriting the configuration if `Config::load_config`
/// returns `Err(FailedToDeserialize)` and as a result, you create a default
/// configuration and call `write_to_file` on the default config. Be aware
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Config {
    /// Assuming Linux, this is `$HOME/.cache/jf-importer`.
    /// Windows should (untest) be at `C:\Users\<you>\Appdata\Roaming\jf-importer`
    /// Like `source_file`, this is `None` iff we can't find the user's home directory.
    // TODO: properly support non-linux
    pub CacheDir: Option<PathBuf>,

    /// Regex pattern for match any and all video files; movies and shows
    /// Configurable via the config in case I missed something.
    pub VideoFileExtensions: String,

    /// Source directory name for Movies
    pub IngestMovieDir: PathBuf,

    /// Source directory name for TV Shows
    pub IngestShowDir: PathBuf,

    /// Movie directory for the jellyfin library:
    pub JfMovieDir: PathBuf,

    /// Show directory for the jellyfin library:
    pub JfShowDir: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Secrets {
    #[serde(skip_serializing, skip_deserializing)]
    pub source_file: PathBuf,

    #[serde(skip_serializing, skip_deserializing)]
    pub load_error: SecretsLoadError,

    pub OMDB_KEY: String,
    pub TMDB_KEY: String,
}
impl Secrets {
    pub fn load_secrets() -> Secrets {
        let base = etcetera::choose_base_strategy().map_err(|_| SecretsLoadError::FailedToFindHomeDir);
        if let Err(e) = base {
            let mut secrets: Secrets = Secrets::default();
            secrets.load_error = e;
            return secrets;
        }
        let base = base.unwrap();
        let secrets_file = base.config_dir().join("jf-importer").join("secrets.toml");
        if ! secrets_file.exists() { return Err(SecretsLoadError::FileDoesNotExist).into(); }
        
        let content = match fs::read_to_string(secrets_file) {
            Err(e) => return Err(SecretsLoadError::FailedToReadFile).into(),
            Ok(content) => content
        };

        let mut secrets = match toml::from_str::<Secrets>(&content) {
            Err(err) => return Err(SecretsLoadError::FailedToDeserialize).into(),
            Ok(cfg) => cfg
        };
        secrets.load_error = SecretsLoadError::Success;
        secrets
    }
    pub fn default() -> Secrets {
        let base = etcetera::choose_base_strategy().map_err(|_| SecretsLoadError::FailedToFindHomeDir);
        if let Err(e) = base {
            let mut cfg: Secrets = Secrets::default();
            cfg.load_error = e;
            return cfg;
        }
        let base = base.unwrap();
        let secrets_file = base.config_dir().join("jf-importer").join("secrets.toml");
        if ! secrets_file.exists() { return Err(SecretsLoadError::FileDoesNotExist).into(); }
        Secrets{
            source_file: secrets_file,
            load_error: SecretsLoadError::RevertedToDefault,
            OMDB_KEY: "".to_string(),
            TMDB_KEY: "".to_string()
        } 
    }
}
impl From<Result<Secrets, SecretsLoadError>> for Secrets {
    fn from(value: Result<Secrets, SecretsLoadError>) -> Self {
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

