use thiserror::Error;
use std::path::PathBuf;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Incompatible mdbook version got {0} expected {1}")]
    IncompatibleVersion(String, String),

    #[error("{0}")]
    EpubDocCreate(String),

    #[error("Could not parse the template")]
    TemplateParse,

    #[error("Content file was not found: \'{0}\'")]
    ContentFileNotFound(String),

    #[error("{0}")]
    AssetFileNotFound(String),

    #[error("Asset was not a file {0}")]
    AssetFile(PathBuf),

    #[error("Could not open css file {0}")]
    CssOpen(PathBuf),

    #[error("Unable to open template {0}")]
    OpenTemplate(PathBuf),

    #[error("Unable to parse render context")]
    RenderContext,

    #[error("Unable to open asset")]
    AssetOpen,

    #[error("Error reading stylesheet")]
    StylesheetRead,

    #[error("epubcheck has failed: {0}")]
    EpubCheck(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Book(#[from] mdbook::errors::Error),
    #[error(transparent)]
    Semver(#[from] semver::Error),
    #[error(transparent)]
    EpubBuilder(#[from] eyre::Report),
    #[error(transparent)]
    Render(#[from] handlebars::RenderError),
    #[error(transparent)]
    TomlDeser(#[from] toml::de::Error),
    #[error(transparent)]
    HttpError(#[from] Box<ureq::Error>),
}

impl From<ureq::Error> for Error {
    fn from(e: ureq::Error) -> Self {
        Error::HttpError(Box::new(e))
    }
}
