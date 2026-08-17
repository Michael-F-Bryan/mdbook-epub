//! A `mdbook` backend for generating a book in the `EPUB` format.

use std::fs::{File, create_dir_all};
use std::path::{Path, PathBuf};

use ::mdbook_core;
use ::semver;
use ::thiserror::Error;
use mdbook_core::config::Config as MdConfig;
use mdbook_renderer::RenderContext;
use semver::{Version, VersionReq};
use tracing::{debug, info};

use errors::Error;

pub use crate::config::Config;
pub use crate::generator::Generator;
use crate::validation::validate_config_title_file_name;

mod config;
pub mod errors;
mod filters;
mod generator;
pub mod init_trace;
mod preprocessor;
mod resources;
mod utils;
mod validation;

// Reexport function
use crate::preprocessor::run_link_preprocessor;
pub use init_trace::init_tracing;

/// The default stylesheet used to make the rendered document pretty.
pub const DEFAULT_CSS: &str = include_str!("master.css");

/// The exact version of `mdbook` this crate is compiled against.
pub const MDBOOK_VERSION: &str = mdbook_core::MDBOOK_VERSION;

/// Check that the version of `mdbook` we're called by is compatible with this
/// backend.
fn version_check(ctx: &RenderContext) -> Result<(), Error> {
    let provided_version = Version::parse(&ctx.version)?;
    let required_version = VersionReq::parse(&format!("~{MDBOOK_VERSION}"))?;

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
    validate_config_title_file_name(&ctx.config)?;

    let outfile = output_filename(&ctx.destination, &ctx.config)?;
    debug!("Output File: {}", outfile.display());

    if !ctx.destination.exists() {
        debug!(
            "Creating destination directory '{}')",
            ctx.destination.display()
        );
        create_dir_all(&ctx.destination)?;
    }

    debug!(
        "Before writing to file. Path to epub file: '{:?}'",
        outfile.display()
    );
    let f = File::create(&outfile)?;
    debug!("Path to epub file: '{:?}'", f);

    // mdbook runs its built-in preprocessors (like the `LinkPreprocessor`,
    // which expands the `{{#include}}`, `{{#rustdoc_include}}`, `{{#playground}}`
    // and `{{#title}}` shortcodes) before handing the book to a renderer.
    // When used as a mdbook plugin the `RenderContext` already contains the
    // preprocessed book. In case a standalone invocations (`mdbook-epub -s`) and
    // programmatic calls (e.g. the integration tests) it bypasses mdbook, so we run
    // the `LinkPreprocessor` here beforehand. We do that just like in mdbook itself
    // only when it is enabled via `[preprocessor.links]` in `book.toml`.
    let ctx = match run_link_preprocessor(ctx)? {
        Some(preprocessed_ctx) => preprocessed_ctx,
        None => ctx.clone(),
    };

    Generator::new(&ctx)?.generate(f)?;

    Ok(())
}

/// Calculate the output filename using the `mdbook` config.
pub fn output_filename(dest: &Path, config: &MdConfig) -> Result<PathBuf, Error> {
    match config.book.title {
        Some(ref title) => {
            validate_config_title_file_name(config)?;
            Ok(dest.join(title).with_extension("epub"))
        }
        None => Ok(dest.join("book.epub")),
    }
}

// IO helper functions to make Errors more clear on debug
pub fn file_io<T>(
    result: std::io::Result<T>,
    action: &str,
    path: impl Into<PathBuf>,
) -> Result<T, Error> {
    result.map_err(|e| Error::AssetFileIo {
        action: action.to_string(),
        path: path.into(),
        source: e,
    })
}

pub fn path_io<T>(result: std::io::Result<T>, path: impl Into<PathBuf>) -> Result<T, Error> {
    result.map_err(|e| Error::AssetPathIo {
        path: path.into(),
        source: e,
    })
}
