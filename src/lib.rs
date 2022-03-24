//! A `mdbook` backend for generating a book in the `EPUB` format.

use ::epub_builder;
use ::handlebars;
use ::thiserror::Error;
#[macro_use]
extern crate log;
use ::mdbook;
use ::semver;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;

use mdbook::config::Config as MdConfig;
use mdbook::renderer::RenderContext;
use semver::{Version, VersionReq};
use std::fs::{create_dir_all, File};
use std::path::{Path, PathBuf};

mod config;
mod generator;
mod resources;

pub use crate::config::Config;
pub use crate::generator::Generator;

/// The default stylesheet used to make the rendered document pretty.
pub const DEFAULT_CSS: &str = include_str!("master.css");

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

    #[error("Epub check failed, ensure the epubcheck program is installed")]
    EpubCheck,

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Book(#[from] mdbook::errors::Error),
    #[error(transparent)]
    Semver(#[from] semver::SemVerError),
    #[error(transparent)]
    SemverReqParse(#[from] semver::ReqParseError),
    #[error(transparent)]
    EpubBuilder(#[from] epub_builder::Error),
    #[error(transparent)]
    Render(#[from] handlebars::RenderError),
    #[error(transparent)]
    TomlDeser(#[from] toml::de::Error),
}

/// The exact version of `mdbook` this crate is compiled against.
pub const MDBOOK_VERSION: &str = mdbook::MDBOOK_VERSION;

/// Check that the version of `mdbook` we're called by is compatible with this
/// backend.
fn version_check(ctx: &RenderContext) -> Result<(), Error> {
    let provided_version = Version::parse(&ctx.version)?;
    let required_version = VersionReq::parse(&format!("~{}", MDBOOK_VERSION))?;

    if !required_version.matches(&provided_version) {
        Err(Error::IncompatibleVersion(
            MDBOOK_VERSION.to_string(),
            ctx.version.clone(),
        ))
    } else {
        Ok(())
    }
}

/// Generate an `EPUB` version of the provided book.
pub fn generate(ctx: &RenderContext) -> Result<(), Error> {
    info!("Starting the EPUB generator");
    version_check(ctx)?;

    let outfile = output_filename(&ctx.destination, &ctx.config);
    trace!("Output File: {}", outfile.display());

    if !ctx.destination.exists() {
        debug!(
            "Creating destination directory ({})",
            ctx.destination.display()
        );
        create_dir_all(&ctx.destination)?;
    }

    let f = File::create(&outfile)?;
    Generator::new(ctx)?.generate(f)?;

    Ok(())
}

/// Calculate the output filename using the `mdbook` config.
pub fn output_filename(dest: &Path, config: &MdConfig) -> PathBuf {
    match config.book.title {
        Some(ref title) => dest.join(title).with_extension("epub"),
        None => dest.join("book.epub"),
    }
}
