use ::epub;
use ::failure;
use std::env;
use ::mdbook;
use ::mdbook_epub;
use ::tempdir;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serial_test;

use epub::doc::EpubDoc;
use std::path::{Path, PathBuf};
use std::process::Command;
use failure::{Error};
use tempdir::TempDir;
use std::sync::Once;
use mdbook::renderer::RenderContext;
use mdbook::MDBook;

static INIT: Once = Once::new();

fn init_logging() {
    INIT.call_once(|| {
        env_logger::init();
    });
}

/// Convenience function for compiling the dummy book into an `EpubDoc`.
fn generate_epub() -> Result< (EpubDoc, PathBuf), Error> {
    let (ctx, _md, temp) = create_dummy_book().unwrap();
    debug!("temp dir = {:?}", &temp);
    mdbook_epub::generate(&ctx)?;
    let output_file = mdbook_epub::output_filename(temp.path(), &ctx.config);
    debug!("output_file = {:?}", &output_file.display());

    // let output_file_name = output_file.display().to_string();
    match EpubDoc::new(&output_file) {
        Ok(epub) => { 
            let result: (EpubDoc, PathBuf) = (epub, output_file);
            return Ok( result )},
        Err(err) => return Err(err),
    }
}

#[test]
#[serial]
fn output_epub_exists() {
    init_logging();
    let (ctx, _md, temp) = create_dummy_book().unwrap();

    let output_file = mdbook_epub::output_filename(temp.path(), &ctx.config);

    assert!(!output_file.exists());
    mdbook_epub::generate(&ctx).unwrap();
    assert!(output_file.exists());
}

#[test]
#[serial]
fn output_epub_is_valid() {
    init_logging();
    let (ctx, _md, temp) = create_dummy_book().unwrap();
    mdbook_epub::generate(&ctx).unwrap();

    let output_file = mdbook_epub::output_filename(temp.path(), &ctx.config);

    let got = EpubDoc::new(&output_file);

    assert!(got.is_ok());

    // also try to run epubcheck, if it's available
    epub_check(&output_file).unwrap();
}

fn epub_check(path: &Path) -> Result<(), Error> {
    init_logging();
    let cmd = Command::new("epubcheck").arg(path).output();

    match cmd {
        Ok(output) => {
            if output.status.success() {
                Ok(())
            } else {
                let msg = failure::err_msg(format!("epubcheck failed\n{:?}", output));
                Err(msg)
            }
        }
        Err(_) => {
            // failed to launch epubcheck, it's probably not installed
            Ok(())
        }
    }
}

#[test]
#[serial]
fn look_for_chapter_1_heading() {
    init_logging();
    debug!("look_for_chapter_1_heading...");
    let mut doc = generate_epub().unwrap();
    debug!("doc current path = {:?}", doc.1);

    let path;
    if cfg!(target_os = "linux") {
        path = Path::new("OEBPS").join("chapter_1.html"); // linux
    } else {
        path = Path::new("OEBPS/chapter_1.html").to_path_buf(); // windows with 'forward slash' /
    }
    debug!("short path = {:?}", path.display().to_string());
    debug!("full path = {:?}", &doc.1);
    let file = doc.0.get_resource_str_by_path(path);
    debug!("file = {:?}", &file);
    let content = file.unwrap();
    debug!("content = {:?}", content.len());
    assert!(content.contains("<h1>Chapter 1</h1>"));
}

#[test]
#[serial]
fn rendered_document_contains_all_chapter_files_and_assets() {
    init_logging();
    debug!("rendered_document_contains_all_chapter_files_and_assets...");
    let chapters = vec!["chapter_1.html", "rust-logo.png"];
    let mut doc = generate_epub().unwrap();
    debug!("doc current path = {:?} / {:?}", doc.0.get_current_path(), doc.1);

    for chapter in chapters {
        let path;
        if cfg!(target_os = "windows") {
            path = Path::new("OEBPS/").join(chapter); // windows with 'forward slash' /
        } else {
            path = Path::new("OEBPS").join(chapter); // linux
        }
        // let path = path.display().to_string();
        debug!("path = {}", &path.display().to_string());
        let got = doc.0.get_resource_by_path(&path);
        debug!("got = {:?}", got.is_ok());
        assert!(got.is_ok(), "{}", &path.display().to_string());
    }
}

/// Use `MDBook::load()` to load the dummy book into memory, then set up the
/// `RenderContext` for use the EPUB generator.
fn create_dummy_book() -> Result<(RenderContext, MDBook, TempDir), Error> {
    let temp = TempDir::new("mdbook-epub")?;

    let dummy_book = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("dummy");
    debug!("dummy_book = {:?}", &dummy_book.display().to_string());

    let md = MDBook::load(dummy_book);

    let book = md.expect("dummy MDBook is not loaded");
    let ctx = RenderContext::new(
        book.root.clone(),
        book.book.clone(),
        book.config.clone(),
        temp.path().to_path_buf(),
    );

    Ok((ctx, book, temp))
}
