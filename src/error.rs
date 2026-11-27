use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("failed to open default audio device: {0}")]
    DeviceInit(#[from] rodio::DeviceSinkError),
    #[error("failed to open audio file {path}: {source}")]
    FileOpen {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to decode FLAC stream: {0}")]
    Decode(#[from] rodio::decoder::DecoderError),
    #[error("seek failed: {0}")]
    Seek(#[from] rodio::source::SeekError),
}

#[derive(Debug, Error)]
pub enum LibraryError {
    #[error("music directory does not exist: {0}")]
    MissingDirectory(PathBuf),
    #[error("failed to read FLAC metadata from {path}: {source}")]
    Metadata {
        path: PathBuf,
        #[source]
        source: lofty::error::FileParseError,
    },
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to access config file: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse config file: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("failed to serialize config file: {0}")]
    Serialize(#[from] toml::ser::Error),
}
