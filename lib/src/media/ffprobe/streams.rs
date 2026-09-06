use serde_flat_path;
use serde_json;
use serde::{Deserialize};

use size::Size;

#[serde_flat_path::flat_path]
#[derive(Debug, Deserialize)]
pub struct VideoStream {
    pub codec_name: String,
    pub width: u32,
    pub height: u32,

    /// Implied per second 
    /// Needs to be an `Option<T>` to avoid deserialization errors. There's
    /// no real standard and there's no guarantee that every video file has
    /// a universal `bit_rate` field.
    /// Extra processing is done at deserialization time.
    #[flat_path(path = ["tags", "bit_rate"])]
    #[serde(alias = "bit_rate")]
    pub bit_rate: Option<Size>
}

#[serde_flat_path::flat_path]
#[derive(Debug, Deserialize)]
pub struct AudioStream {
    pub codec_name: String,
    pub bit_rate: Option<Size>, // implied per second
    pub sample_rate: Option<Size>, // implied per second

    #[flat_path("tags.language")]
    #[serde(alias = "language")]
    pub language: Option<crate::media::ffprobe::Language>,
}

#[serde_flat_path::flat_path]
#[derive(Debug, Deserialize)]
pub struct SubtitleStream {
    pub codec_name: String,

    #[flat_path("tags.language")]
    #[serde(alias = "language")]
    pub language: Option<crate::media::ffprobe::Language>,
}

/// not currently unsupported
#[derive(Debug, Deserialize)]
pub struct DataStream {}

