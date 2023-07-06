use ::epub;
use ::mdbook;
use ::mdbook_epub;
use ::tempdir;
use std::env;
use std::fs::File;
use std::io::BufReader;

#[macro_use]
extern crate log;
#[macro_use]
extern crate serial_test;

use epub::doc::EpubDoc;
use mdbook::renderer::RenderContext;
use mdbook::MDBook;
use mdbook_epub::Error;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Once;
use tempdir::TempDir;

static INIT: Once = Once::new();

fn init_logging() {
    INIT.call_once(|| {
        env_logger::init();
    });
}

/// Convenience function for compiling the dummy book into an `EpubDoc`.
fn generate_epub() -> Result<(EpubDoc<BufReader<File>>, PathBuf), Error> {
    let (ctx, _md, temp) = create_dummy_book().unwrap();
    debug!("temp dir = {:?}", &temp);
    mdbook_epub::generate(&ctx)?;
    let output_file = mdbook_epub::output_filename(temp.path(), &ctx.config);
    debug!("output_file = {:?}", &output_file.display());

    // let output_file_name = output_file.display().to_string();
    match EpubDoc::new(&output_file) {
        Ok(epub) => {
            Ok((epub, output_file))
        }
        Err(err) => {
            error!("dummy book creation error = {}", err);
            Err(Error::EpubDocCreate(output_file.display().to_string()))
        }
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
                Err(Error::EpubCheck)
            }
        }
        Err(_) => {
            // failed to launch epubcheck, it's probably not installed
            debug!("Failed to launch epubcheck, it's probably not installed here...");
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

    let path = if cfg!(target_os = "linux") {
        Path::new("OEBPS").join("chapter_1.html") // linux
    } else {
        Path::new("OEBPS/chapter_1.html").to_path_buf() // windows with 'forward slash' /
    };
    debug!("short path = {:?}", path.display().to_string());
    debug!("full path = {:?}", &doc.1);
    let file = doc.0.get_resource_str_by_path(path);
    debug!("file = {:?}", &file);
    let content = file.unwrap();
    debug!("content = {:?}", content.len());
    assert!(content.contains("<h1>Chapter 1</h1>"));
    // assert!(!content.contains("{{#rustdoc_include")); // prepare fix link error
    // assert!(content.contains("fn main() {")); // prepare fix link error
}

#[test]
#[serial]
fn rendered_document_contains_all_chapter_files_and_assets() {
    init_logging();
    debug!("rendered_document_contains_all_chapter_files_and_assets...");
    let chapters = vec!["chapter_1.html", "rust-logo.png"];
    let mut doc = generate_epub().unwrap();
    assert_eq!(9, doc.0.resources.len());
    assert_eq!(2, doc.0.spine.len());
    // let title = doc.0.mdata("title");
    assert_eq!(doc.0.mdata("title").unwrap(), "DummyBook");
    assert_eq!(doc.0.mdata("language").unwrap(), "en");
    debug!(
        "doc current path = {:?} / {:?}",
        doc.0.get_current_path(),
        doc.1
    );

    for chapter in chapters {
        let path = if cfg!(target_os = "windows") {
            Path::new("OEBPS/").join(chapter) // windows with 'forward slash' /
        } else {
            Path::new("OEBPS").join(chapter) // linux
        };
        let path = path.display().to_string();
        debug!("path = {}", &path);
        let got = doc.0.get_resource_by_path(&path);
        // data length
        assert!(got.unwrap().len() > 0);
    }
}

#[test]
#[serial]
fn straight_quotes_transformed_into_curly_quotes() {
    init_logging();
    debug!("straight_quotes_transformed_into_curly_quotes...");
    let mut doc = generate_epub().unwrap();
    debug!("doc current path = {:?}", doc.1);

    let path = if cfg!(target_os = "linux") {
        Path::new("OEBPS").join("chapter_1.html") // linux
    } else {
        Path::new("OEBPS/chapter_1.html").to_path_buf() // windows with 'forward slash' /
    };
    let file = doc.0.get_resource_str_by_path(path);
    let content = file.unwrap();
    debug!("content = {:?}", content);
    assert!(content.contains("<p>“One morning, when Gregor Samsa woke from troubled dreams, he found himself ‘transformed’ in his bed into a horrible vermin.”</p>"));
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
