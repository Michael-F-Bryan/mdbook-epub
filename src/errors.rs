use mime_guess::mime::FromStrError;
use std::path::PathBuf;
use thiserror::Error;

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

    #[error("Failed to {action} '{path}': {source}")]
    AssetFileIo {
        action: String,  // string action type: "open", "create", "read", "write"
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Path error for '{path}': {source}")]
    AssetPathIo {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Unable to open asset")]
    AssetOpen(#[from] std::io::Error),

    #[error("Failed to find resource file (a root + path?): {0}")]
    ResourceNotFound(PathBuf),

    #[error("Error reading stylesheet")]
    StylesheetRead,

    #[error("epubcheck has failed: {0}")]
    EpubCheck(String),

    #[error(transparent)]
    AssetOutsideSrcDir(#[from] std::path::StripPrefixError),

    #[error(transparent)]
    Book(#[from] mdbook_core::errors::Error),
    #[error(transparent)]
    Semver(#[from] semver::Error),
    #[error(transparent)]
    EpubBuilder(#[from] epub_builder::Error),
    #[error(transparent)]
    Render(#[from] handlebars::RenderError),
    #[error(transparent)]
    TomlDeser(#[from] toml::de::Error),
    #[error(transparent)]
    HttpError(#[from] Box<ureq::Error>),
    #[error(transparent)]
    MimeTypeError(#[from] FromStrError),

    #[error("Incorrect book 'title', impossible to create file with name: '{0}'")]
    EpubBookNameOrPath(String),
}

impl From<ureq::Error> for Error {
    fn from(e: ureq::Error) -> Self {
        Error::HttpError(Box::new(e))
    }
}
