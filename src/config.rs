use std::path::PathBuf;
use failure::Error;
use mdbook::renderer::RenderContext;

/// The configuration struct used to tweak how an EPUB document is generated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Config {
    /// A list of additional stylesheets to include in the document.
    pub additional_css: Vec<PathBuf>,
    /// Should we use the default stylesheet (default: true)?
    pub use_default_css: bool,
}

impl Config {
    /// Get the `output.epub` table from the provided `book.toml` config,
    /// falling back to the default if
    pub fn from_render_context(ctx: &RenderContext) -> Result<Config, Error> {
        match ctx.config.get("output.epub") {
            Some(table) => table.clone().try_into().map_err(|e| Error::from(e)),
            None => Ok(Config::default()),
        }
    }
}

impl Default for Config {
    fn default() -> Config {
        Config {
            use_default_css: true,
            additional_css: Vec::new(),
        }
    }
}
