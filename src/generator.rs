use std::io::{Cursor, Write};
use std::path::PathBuf;

use mdbook::renderer::RenderContext;
use mdbook::book::{BookItem, Chapter};
use epub_builder::{EpubBuilder, EpubContent, ReferenceType, Toc, TocElement, ZipLibrary};
use failure::{Error, SyncFailure};
use pulldown_cmark::{html, Parser};


#[derive(Debug)]
pub struct Generator<'a> {
    ctx: &'a RenderContext,
    builder: EpubBuilder<ZipLibrary>,
    config: EpubConfig,
}

impl<'a> Generator<'a> {
    pub fn new(ctx: &'a RenderContext) -> Result<Generator<'a>, Error> {
        let builder = EpubBuilder::new(ZipLibrary::new().sync()?).sync()?;

        let config = EpubConfig::from_render_context(ctx)?;

        Ok(Generator {
            builder,
            ctx,
            config,
        })
    }

    fn populate_metadata(&mut self) -> Result<(), Error> {
        self.builder.metadata("generator", "mdbook-epub").sync()?;

        if let Some(title) = self.ctx.config.book.title.clone() {
            self.builder.metadata("title", title).sync()?;
        }
        if let Some(desc) = self.ctx.config.book.description.clone() {
            self.builder.metadata("description", desc).sync()?;
        }

        if !self.ctx.config.book.authors.is_empty() {
            self.builder
                .metadata("author", self.ctx.config.book.authors.join(", "))
                .sync()?;
        }

        Ok(())
    }

    pub fn generate<W: Write>(mut self, writer: W) -> Result<(), Error> {
        self.populate_metadata()?;
        self.generate_chapters()?;

        self.additional_assets()?;
        self.builder.generate(writer).sync()?;

        Ok(())
    }

    fn generate_chapters(&mut self) -> Result<(), Error> {
        for item in &self.ctx.book.sections {
            if let BookItem::Chapter(ref ch) = *item {
                self.add_chapter(ch)?;
            }
        }

        Ok(())
    }

    fn add_chapter(&mut self, ch: &Chapter) -> Result<(), Error> {
        let mut buffer = String::new();
        html::push_html(&mut buffer, Parser::new(&ch.content));

        let data = Cursor::new(Vec::from(buffer));

        let path = ch.path.with_extension("html").display().to_string();
        let mut content = EpubContent::new(path, data).title(format!("{}", ch));

        let level = ch.number.as_ref().map(|n| n.len() as i32).unwrap_or(0);
        content = content.level(level);

        for sub_item in &ch.sub_items {
            if let BookItem::Chapter(ref sub_ch) = *sub_item {
                self.add_chapter(sub_ch)?;

                let child_path = sub_ch.path.with_extension("html").display().to_string();
                content = content.child(TocElement::new(child_path, format!("{}", sub_ch)));
            }
        }

        self.builder.add_content(content).sync()?;
        Ok(())
    }


    /// Add any other additional assets to the book (CSS, images, etc).
    fn additional_assets(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

trait ResultExt<T, E> {
    fn sync(self) -> Result<T, SyncFailure<E>>
    where
        Self: Sized,
        E: ::std::error::Error + Send + 'static;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    fn sync(self) -> Result<T, SyncFailure<E>>
    where
        Self: Sized,
        E: ::std::error::Error + Send + 'static,
    {
        self.map_err(SyncFailure::new)
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct EpubConfig {
    additional_css: Vec<PathBuf>,
}

impl EpubConfig {
    /// Get the `output.epub` table from the provided `book.toml` config,
    /// falling back to the default if
    pub fn from_render_context(ctx: &RenderContext) -> Result<EpubConfig, Error> {
        match ctx.config.get("output.epub") {
            Some(table) => table.clone().try_into().map_err(|e| Error::from(e)),
            None => Ok(EpubConfig::default()),
        }
    }
}
