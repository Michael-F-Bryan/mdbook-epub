use std::path::PathBuf;
use super::Error;
use mdbook::renderer::RenderContext;

pub const DEFAULT_TEMPLATE: &str = include_str!("index.hbs");

/// The configuration struct used to tweak how an EPUB document is generated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Config {
    /// A list of additional stylesheets to include in the document.
    pub additional_css: Vec<PathBuf>,
    /// Should we use the default stylesheet (default: true)?
    pub use_default_css: bool,
    /// The template file to use when rendering individual chapters (relative
    /// to the book root).
    pub index_template: Option<PathBuf>,
    /// A cover image to use for the epub.
    pub cover_image: Option<PathBuf>,
    /// Additional assets to include in the ebook, such as typefaces.
    pub additional_resources: Vec<PathBuf>,
}

impl Config {
    /// Get the `output.epub` table from the provided `book.toml` config,
    /// falling back to the default if
    pub fn from_render_context(ctx: &RenderContext) -> Result<Config, Error> {
        match ctx.config.get("output.epub") {
            Some(table) => {
                let mut cfg: Config = table.clone().try_into()?;

                // make sure we update the `index_template` to make it relative
                // to the book root
                if let Some(template_file) = cfg.index_template.take() {
                    cfg.index_template = Some(ctx.root.join(template_file));
                }

                Ok(cfg)
            }
            None => Ok(Config::default()),
        }
    }

    pub fn template(&self) -> Result<String, Error> {
        match self.index_template {
            Some(ref filename) => {
                let buffer = std::fs::read_to_string(filename)
                    .map_err(|_| Error::OpenTemplate(filename.clone()))?;

                Ok(buffer)
            }
            None => Ok(DEFAULT_TEMPLATE.to_string()),
        }
    }
}

impl Default for Config {
    fn default() -> Config {
        Config {
            use_default_css: true,
            additional_css: Vec::new(),
            index_template: None,
            cover_image: None,
            additional_resources: Vec::new(),
        }
    }
}
