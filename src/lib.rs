extern crate epub_builder;
extern crate failure;
#[macro_use]
extern crate failure_derive;
extern crate mdbook;
extern crate pulldown_cmark;
extern crate semver;
extern crate serde_json;


use std::fs::File;
use std::io::Cursor;
use mdbook::renderer::RenderContext;
use mdbook::book::{BookItem, Chapter};
use mdbook::config::Config;
use failure::{Error, SyncFailure};
use pulldown_cmark::{html, Parser};
use epub_builder::{EpubBuilder, EpubContent, Zip, ZipLibrary};
use semver::{Version, VersionReq};


/// The exact version of `mdbook` this crate is compiled against.
pub const MDBOOK_VERSION: &'static str = env!("MDBOOK_VERSION");


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

#[derive(Debug, Clone, PartialEq, Fail)]
#[fail(display = "Incompatible mdbook version, expected {} but got {}", expected, got)]
struct IncompatibleMdbookVersion {
    expected: String,
    got: String,
}


/// Check that the version of `mdbook` we're called by is compatible with this
/// backend.
fn version_check(ctx: &RenderContext) -> Result<(), Error> {
    let provided_version = Version::parse(&ctx.version)?;
    let required_version = VersionReq::parse(MDBOOK_VERSION)?;

    if !required_version.matches(&provided_version) {
        let e = IncompatibleMdbookVersion {
            expected: MDBOOK_VERSION.to_string(),
            got: ctx.version.clone(),
        };

        Err(Error::from(e))
    } else {
        Ok(())
    }
}

pub fn generate(ctx: &RenderContext) -> Result<(), Error> {
    version_check(ctx)?;

    let mut builder = EpubBuilder::new(ZipLibrary::new().sync()?).sync()?;

    populate_metadata(&mut builder, &ctx.config)?;
    builder.inline_toc();

    let chapters = ctx.book.iter().filter_map(|item| match *item {
        BookItem::Chapter(ref ch) => Some(ch),
        BookItem::Separator => None,
    });

    for chapter in chapters {
        let content = generate_chapter(chapter);
        builder.add_content(content).sync()?;
    }

    let outfile = match ctx.config.book.title {
        Some(ref title) => ctx.build_dir().join(title).with_extension("epub"),
        None => ctx.build_dir().join("book.epub"),
    };
    let f = File::create(&outfile)?;

    builder.generate(f).sync()?;

    Ok(())
}

fn populate_metadata<Z: Zip>(builder: &mut EpubBuilder<Z>, cfg: &Config) -> Result<(), Error> {
    builder.metadata("generator", "mdbook-epub").sync()?;

    if let Some(title) = cfg.book.title.clone() {
        builder.metadata("title", title).sync()?;
    }
    if let Some(desc) = cfg.book.description.clone() {
        builder.metadata("description", desc).sync()?;
    }

    if !cfg.book.authors.is_empty() {
        builder
            .metadata("author", cfg.book.authors.join(", "))
            .sync()?;
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Fail)]
#[fail(display = "Rendering Failed")]
pub struct RenderError {}


fn generate_chapter(ch: &Chapter) -> EpubContent<Cursor<Vec<u8>>> {
    let mut buffer = String::new();
    html::push_html(&mut buffer, Parser::new(&ch.content));

    let data = Cursor::new(Vec::from(buffer));

    EpubContent::new(format!("{}.html", ch.name), data).title(ch.name.clone())
}
