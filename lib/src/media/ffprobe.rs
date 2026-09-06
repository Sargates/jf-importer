mod streams;

use crate::media::ffprobe;

use std::path::PathBuf;

use std::process::{ExitStatus, Stdio};
use tokio::process::{Command};
use tokio::io::{AsyncReadExt, BufReader, AsyncBufReadExt};

use serde_json::{json, Value};
use serde::{Deserialize, Deserializer, Serialize};
use serde::de::Error;

use size::Size;

use self::streams::VideoStream;

/// Lanugages for various audio/subtitles
#[derive(Deserialize, Debug, Serialize, Clone)]
#[serde(from = "&str")]
pub enum Language {
    English, Japanese, German, French, Italian,
    Spanish, Portuguese, Portuguese_BR, Chinese, Chinese_Hans,
    Chinese_Hant, Korean, Russian, Ukrainian, Polish, Czech,
    Slovak, Dutch, Swedish, Norwegian, Danish, Finnish,
    Turkish, Greek, Hebrew, Arabic, Hindi, Thai, Vietnamese,
    Indonesian, Malay, Romanian, Hungarian, Bulgarian,
    Croatian, Serbian, Slovenian, Catalan,
    Undetermined, Other(String),
}
impl From<&str> for Language {
    fn from(value: &str) -> Self {
        use Language::*;
        // this list and its mappings are AI generated. I only speak
        // english so it's more accurate than anything I could make
        match value.to_ascii_lowercase().as_str() {
            // English
            "eng" | "en" | "en-us" | "en_us" |
            "en-br" | "en_br"                  => English,

            // Japanese
            "jpn" | "ja"                       => Japanese,

            // German
            "deu" | "ger" | "de"               => German,

            // French
            "fra" | "fre" | "fr"               => French,

            // Italian
            "ita" | "it"                       => Italian,

            // Spanish
            "spa" | "es"                       => Spanish,

            // Portuguese
            "por" | "pt"                       => Portuguese,
            "pt-br" | "pt_br"                  => Portuguese_BR,

            // Chinese
            "zho" | "chi" | "zh"               => Chinese,
            "zh-hans" | "zh_cn" | "zh-sg"      => Chinese_Hans,
            "zh-hant" | "zh_tw" | "zh-hk"      => Chinese_Hant,

            // Korean
            "kor" | "ko"                       => Korean,

            // Slavic languages
            "rus" | "ru"                       => Russian,
            "ukr" | "uk"                       => Ukrainian,
            "pol" | "pl"                       => Polish,
            "ces" | "cze" | "cs"               => Czech,
            "slk" | "slo" | "sk"               => Slovak,
            "bul" | "bg"                       => Bulgarian,
            "hrv" | "hr"                       => Croatian,
            "srp" | "sr"                       => Serbian,
            "slv" | "sl"                       => Slovenian,

            // Germanic languages
            "nld" | "dut" | "nl"               => Dutch,
            "swe" | "sv"                       => Swedish,
            "nor" | "no"                       => Norwegian,
            "dan" | "da"                       => Danish,
            "fin" | "fi"                       => Finnish,

            // Other European languages
            "tur" | "tr"                       => Turkish,
            "ell" | "gre" | "el"               => Greek,
            "heb" | "he" | "iw"                => Hebrew,
            "ron" | "rum" | "ro"               => Romanian,
            "hun" | "hu"                       => Hungarian,
            "cat" | "ca"                       => Catalan,

            // Middle Eastern and Asian langu  age
            "ara" | "ar"                       => Arabic,
            "hin" | "hi"                       => Hindi,
            "tha" | "th"                       => Thai,
            "vie" | "vi"                       => Vietnamese,
            "ind" | "id"                       => Indonesian,
            "msa" | "may" | "ms"               => Malay,
            
            "und"                              => Undetermined,
            other                              => Other(other.to_owned()),

        }
    }
}
impl Into<&str> for Language {
    fn into(self) -> &'static str {
        use Language::*;
        // again, generated. Should* follow ISO 639-1
        match self {
            English=> "en",

            // Japanese
            Japanese => "ja",

            // German
            German => "de",

            // French
            French => "fr",

            // Italian
            Italian => "it",

            // Spanish
            Spanish => "es",

            // Portuguese
            Portuguese => "pt",
            Portuguese_BR => "pt-br",

            // Chinese
            Chinese => "zh",
            Chinese_Hans => "zh-hans",
            Chinese_Hant => "zh-hant",

            // Korean
            Korean => "ko",

            // Slavic languages
            Russian => "ru",
            Ukrainian => "uk",
            Polish => "pl",
            Czech => "cs",
            Slovak => "sk",
            Bulgarian => "bg",
            Croatian => "hr",
            Serbian => "sr",
            Slovenian => "sl",

            // Germanic languages
            Dutch => "nl",
            Swedish => "sv",
            Norwegian => "no",
            Danish => "da",
            Finnish => "fi",

            // Other European languages
            Turkish => "tr",
            Greek => "el",
            Hebrew => "he",
            Romanian => "ro",
            Hungarian => "hu",
            Catalan => "ca",

            // Middle Eastern and Asian languages
            Arabic => "ar",
            Hindi => "hi",
            Thai => "th",
            Vietnamese => "vi",
            Indonesian => "id",
            Malay => "ms",

            Undetermined | Other(_) => "Unknown",
        }
    }
}

#[derive(Debug)]
pub enum FFprobeFailure {
    FFprobeExecutableNotFound,
    FailedToSpawnProcess,

    InputDoesNotExist,
    InputIsNotAFile,

    /// internal failures
    NoJsonOutput,
    FailedToParseJson(serde_json::Error),
}

#[derive(Deserialize, Debug)]
pub struct FFprobeMediaInfo {
    pub streams: Vec<FFprobeStream>,
    pub format: FFprobeFormat,
}
impl FFprobeMediaInfo {
    /// Run `ffprobe` on a file and parse the output
    // TODO: `tracing` logging
    pub async fn from_file(path: &PathBuf) -> Result<Self, FFprobeFailure> {
        use FFprobeFailure::*;

        if ! path.exists() { return Err(InputDoesNotExist); }
        if ! path.is_file() { return Err(InputIsNotAFile); }

        let mut cmd = Command::new("ffprobe");
        cmd
            .arg("-print_format")
            .arg("json")
            .arg("-show_format")
            .arg("-show_streams")
            .arg(path)
            .kill_on_drop(true)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
        ;
        tracing::info!("Executing command...\nFormatted command for convenience: ffprobe -print_format json -show_format -show_streams {:?}", path);

        let mut child = match cmd.spawn() {
            Ok(cmd) => cmd,
            Err(e) => {
                tracing::error!("Failed to spawn process\n{:#?}", cmd);
                std::process::exit(1);
            }
        };

        let stdout = child.stdout.take().unwrap();
        let mut reader = BufReader::new(stdout).lines();

        let mut buf = tokio::spawn(async move {
            let mut buf: Vec<char> = Vec::new();
            // tokio::Command hangs on `wait().await` if the command has a
            // large output and we don't read from the buffer line-by-line
            while let Some(line) = reader.next_line().await.unwrap() {
                for c in line.chars() {
                    buf.push(c)
                }
                buf.push('\n');
            }
            buf
        }).await.unwrap();

        let stdout: String = buf.iter().collect();

        if stdout.len() == 0 {
            tracing::error!("No JSON output from command: {:?}", cmd);
            return Err(NoJsonOutput);
        }

        match serde_json::from_str::<FFprobeMediaInfo>(&stdout) {
            Ok(json) => Ok(json),
            Err(e) => {
                tracing::error!("Failed to parse JSON\n{}", &stdout);
                Err(FailedToParseJson(e))
            },
        }
    }
}

#[derive(Debug)]
pub enum FFprobeStream {
    Video(streams::VideoStream),
    Audio(streams::AudioStream),
    Subtitle(streams::SubtitleStream),
    Data(streams::DataStream),
    Other(String),
}

impl<'de> Deserialize<'de> for FFprobeStream {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {

        // print!("Deserializing! ");
        let value = Value::deserialize(deserializer)?;
        // println!("{}", value);

        let codec = value
            .get("codec_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| D::Error::custom("missing or invalid codec field"))?;

        let mut out = match codec {
            "video" => streams::VideoStream::deserialize(&value)
                .map(FFprobeStream::Video)
                .map_err(D::Error::custom),
            "audio" => streams::AudioStream::deserialize(&value)
                .map(FFprobeStream::Audio)
                .map_err(D::Error::custom),
            "subtitle" => streams::SubtitleStream::deserialize(&value)
                .map(FFprobeStream::Subtitle)
                .map_err(D::Error::custom),
            "data" => streams::DataStream::deserialize(&value)
                .map(FFprobeStream::Data)
                .map_err(serde::de::Error::custom),
            other => Err(serde::de::Error::custom(format!("unknown codec: {}", other))),
        };

        // mkvmerge support for bit_rate
        // https://manpages.debian.org/stretch/mkvtoolnix/mkvmerge.1.en.html

        // We need to hold a mutable reference to the option itself. `Option<&mut T>`
        // won't do in the case that `bit_rate` is `None`; we won't be able to update it.
        // We create a dummy `None` value to maintain a valid exclusive reference
        let mut dummy = None;
        let mut bit_rate: &mut Option<Size> = if let Ok(ref mut out) = out {
            match out {
                FFprobeStream::Video(stream) => &mut stream.bit_rate,
                FFprobeStream::Audio(stream) => &mut stream.bit_rate,
                _ => &mut dummy,
            }
        } else { &mut dummy };

        let bps = value
            .get("tags")
            .map(|t| t.get("BPS"))
            .flatten()
            .cloned();

        if let None = bit_rate &&
           let Some(bps) = bps
        {
            let size = Size::deserialize(bps.clone())
                .map_err(D::Error::custom)?;
            *bit_rate = Some(size);
        }

        out
    }
}

#[derive(Deserialize, Debug)]
pub struct FFprobeFormat {
    size: Size,
    bit_rate: Size,
    // Likely "und"
    language: Option<Language>
}

