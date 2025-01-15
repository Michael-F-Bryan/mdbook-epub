use super::Error;
use mdbook::renderer::RenderContext;
use std::path::PathBuf;

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
    /// Don't render section labels.
    pub no_section_label: bool,
    /// Use "smart quotes" instead of the usual `"` character.
    pub curly_quotes: bool,
    /// Add backreference links to footnote definitions and allow pop-up footnote behaviour.
    /// Requires `epub-version = 3`, in which case it is enabled by default.
    pub footnote_backrefs: bool,
    /// EPUB version to use if specified, otherwise defaults to the epub-builder default.
    pub epub_version: Option<u8>,
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
            no_section_label: false,
            curly_quotes: false,
            footnote_backrefs: false,
            epub_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn test_from_render_context_minimal_settings() {
        let tmp_dir = TempDir::new().unwrap();
        let json = ctx_with_template(
            "unknown_src",
            tmp_dir.path().join("test-mdbook-epub").as_path(),
        )
        .to_string();
        let ctx = RenderContext::from_json(json.as_bytes()).unwrap();
        let config = Config::from_render_context(&ctx);
        assert!(config.is_ok());
    }

    fn ctx_with_template(source: &str, destination: &Path) -> serde_json::Value {
        json!({
            "version": mdbook::MDBOOK_VERSION,
            "root": "tests/long_book_example",
            "book": {"sections": [], "__non_exhaustive": null},
            "config": {
                "book": {"authors": [], "language": "en", "multilingual": false,
                    "src": source, "title": "DummyBook"},
                "output": {"epub": {"optional": true}}},
            "destination": destination
        })
    }
}
