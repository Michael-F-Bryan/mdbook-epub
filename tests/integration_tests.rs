extern crate epub;
extern crate failure;
extern crate mdbook;
extern crate mdbook_epub;
extern crate tempdir;

use std::path::Path;
use failure::{Error, SyncFailure};
use tempdir::TempDir;
use epub::doc::EpubDoc;
use mdbook::renderer::RenderContext;
use mdbook::MDBook;

/// Convenience function for compiling the dummy book into an `EpubDoc`.
fn generate_epub() -> Result<EpubDoc, Error> {
    let (ctx, _md, temp) = create_dummy_book().unwrap();
    mdbook_epub::generate(&ctx)?;
    let output_file = mdbook_epub::output_filename(temp.path(), &ctx.config);

    let output_file = output_file.display().to_string();
    EpubDoc::new(&output_file).map_err(Error::from)
}

#[test]
fn output_epub_exists() {
    let (ctx, _md, temp) = create_dummy_book().unwrap();

    let output_file = mdbook_epub::output_filename(temp.path(), &ctx.config);

    assert!(!output_file.exists());
    mdbook_epub::generate(&ctx).unwrap();
    assert!(output_file.exists());
}

#[test]
fn output_epub_is_valid() {
    let (ctx, _md, temp) = create_dummy_book().unwrap();
    mdbook_epub::generate(&ctx).unwrap();

    let output_file = mdbook_epub::output_filename(temp.path(), &ctx.config);
    let output_file = output_file.display().to_string();

    let got = EpubDoc::new(&output_file);

    assert!(got.is_ok());
}

#[test]
fn look_for_chapter_1_heading() {
    let mut doc = generate_epub().unwrap();

    let path = Path::new("OEBPS").join("chapter_1.html");
    let path = path.display().to_string();
    let content = doc
        .get_resource_str_by_path(path)
        .unwrap();

    assert!(content.contains("<h1>Chapter 1</h1>"));
}

#[test]
fn rendered_document_contains_all_chapter_files_and_assets() {
    let chapters = vec!["chapter_1.html", "rust-logo.png"];
    let mut doc = generate_epub().unwrap();

    for chapter in chapters {
        let path = Path::new("OEBPS").join(chapter);
        let path = path.display().to_string();
        let got = doc.get_resource_by_path(&path);

        assert!(got.is_ok(), "{}", path);
    }
}

/// Use `MDBook::load()` to load the dummy book into memory, then set up the
/// `RenderContext` for use the EPUB generator.
fn create_dummy_book() -> Result<(RenderContext, MDBook, TempDir), Error> {
    let temp = TempDir::new("mdbook-epub")?;

    let dummy_book = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("dummy");

    let md = MDBook::load(dummy_book).map_err(SyncFailure::new)?;

    let ctx = RenderContext::new(md.root.clone(), md.book.clone(), md.config.clone(), temp.path().to_path_buf());

    Ok((ctx, md, temp))
}
