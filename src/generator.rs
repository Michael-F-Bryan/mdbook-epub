use std::io::{Cursor, Read, Write};
use std::fs::File;

use mdbook::renderer::RenderContext;
use mdbook::book::{BookItem, Chapter};
use epub_builder::{EpubBuilder, EpubContent, TocElement, ZipLibrary};
use failure::{Error, ResultExt};
use pulldown_cmark::{html, Parser};

use config::Config;
use DEFAULT_CSS;
use utils::ResultExt as SyncResultExt;

const CHAPTER_TEMPLATE_HEADER: &str = r#"<?xml version='1.0' encoding='utf-8'?>
<html xmlns="http://www.w3.org/1999/xhtml">
  <head>
    <meta http-equiv="Content-Type" content="text/html; charset=utf-8"/>
    <link href="stylesheet.css" rel="stylesheet" type="text/css"/>
    <title>TODO something here TODO</title>
</head>
<body>
"#;

const CHAPTER_TEMPLATE_FOOTER: &str = r#"</body>
</html>
"#;

/// The actual EPUB book renderer.
#[derive(Debug)]
pub struct Generator<'a> {
    ctx: &'a RenderContext,
    builder: EpubBuilder<ZipLibrary>,
    config: Config,
}

impl<'a> Generator<'a> {
    pub fn new(ctx: &'a RenderContext) -> Result<Generator<'a>, Error> {
        let builder = EpubBuilder::new(ZipLibrary::new().sync()?).sync()?;

        let config = Config::from_render_context(ctx)?;

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
        buffer.push_str(CHAPTER_TEMPLATE_HEADER);
        html::push_html(&mut buffer, Parser::new(&ch.content));
        buffer.push_str(CHAPTER_TEMPLATE_FOOTER);

        let data = Cursor::new(Vec::from(buffer));

        let path = ch.path.with_extension("html").display().to_string();
        let mut content = EpubContent::new(path, data).title(format!("{}", ch));

        let level = ch.number.as_ref().map(|n| n.len() as i32 - 1).unwrap_or(0);
        content = content.level(level);

        // unfortunately we need to do two passes through `ch.sub_items` here.
        // The first pass will add each sub-item to the current chapter's toc
        // and the second pass actually adds the sub-items to the book.
        for sub_item in &ch.sub_items {
            if let BookItem::Chapter(ref sub_ch) = *sub_item {
                let child_path = sub_ch.path.with_extension("html").display().to_string();
                content = content.child(TocElement::new(child_path, format!("{}", sub_ch)));
            }
        }

        self.builder.add_content(content).sync()?;

        // second pass to actually add the sub-chapters
        for sub_item in &ch.sub_items {
            if let BookItem::Chapter(ref sub_ch) = *sub_item {
                self.add_chapter(sub_ch)?;
            }
        }

        Ok(())
    }

    /// Add any other additional assets to the book (CSS, images, etc).
    fn additional_assets(&mut self) -> Result<(), Error> {
        let stylesheet = self.generate_stylesheet()
            .context("Unable to generate stylesheet")?;
        self.builder.stylesheet(stylesheet.as_slice()).sync()?;

        Ok(())
    }

    /// Concatenate all provided stylesheets into one long stylesheet.
    fn generate_stylesheet(&self) -> Result<Vec<u8>, Error> {
        let mut stylesheet = Vec::new();

        if self.config.use_default_css {
            stylesheet.extend(DEFAULT_CSS.as_bytes());
        }

        for additional_css in &self.config.additional_css {
            let mut f = File::open(&additional_css)
                .with_context(|_| format!("Unable to open {}", additional_css.display()))?;
            f.read_to_end(&mut stylesheet)
                .context("Error reading stylesheet")?;
        }

        Ok(stylesheet)
    }
}
