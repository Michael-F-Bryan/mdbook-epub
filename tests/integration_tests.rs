#[macro_use]
extern crate log;
#[macro_use]
extern crate serial_test;

use std::env;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Once;

use ::epub;
use ::mdbook;
use epub::doc::EpubDoc;
use mdbook::renderer::RenderContext;
use mdbook::MDBook;
use tempfile::TempDir;

use ::mdbook_epub;
use mdbook_epub::errors::Error;

static INIT: Once = Once::new();

fn init_logging() {
    INIT.call_once(|| {
        let _ = env_logger::builder().is_test(true).try_init();
    });
}

/// Convenience function for compiling the dummy book into an `EpubDoc`.
fn generate_epub(epub3: bool) -> Result<(EpubDoc<BufReader<File>>, PathBuf), Error> {
    debug!("generate_epub, epub3 = {:?}...", epub3);
    let name = if epub3 { "footnote_epub3_example" } else { "dummy" };
    let (ctx, _md, temp) = create_dummy_book(name).unwrap();
    debug!("temp dir = {:?}", &temp);
    mdbook_epub::generate(&ctx)?;
    let output_file = mdbook_epub::output_filename(temp.path(), &ctx.config);
    debug!("output_file = {:?}", &output_file.display());

    // let output_file_name = output_file.display().to_string();
    match EpubDoc::new(&output_file) {
        Ok(epub) => Ok((epub, output_file)),
        Err(err) => {
            error!("dummy book creation error = {:?}", err);
            Err(Error::EpubDocCreate(output_file.display().to_string()))
        }
    }
}

#[test]
#[serial]
fn output_epub_exists() {
    init_logging();
    debug!("fn output_epub_exists...");
    let (ctx, _md, temp) = create_dummy_book("dummy").unwrap();

    let output_file = mdbook_epub::output_filename(temp.path(), &ctx.config);

    assert!(!output_file.exists());
    mdbook_epub::generate(&ctx).unwrap();
    assert!(output_file.exists());
}

#[test]
#[serial]
fn output_epub3_exists() {
    init_logging();
    debug!("fn output_epub3_exists...");
    let (ctx, _md, temp) = create_dummy_book("footnote_epub3_example").unwrap();

    let output_file = mdbook_epub::output_filename(temp.path(), &ctx.config);

    assert!(!output_file.exists());
    mdbook_epub::generate(&ctx).unwrap();
    assert!(output_file.exists());
}

#[ignore = "Waiting for issue = https://github.com/lise-henry/epub-builder/issues/45"]
#[test]
#[serial]
fn output_epub_is_valid() {
    init_logging();
    debug!("output_epub_is_valid...");
    let (ctx, _md, temp) = create_dummy_book("dummy").unwrap();
    mdbook_epub::generate(&ctx).unwrap();

    let output_file = mdbook_epub::output_filename(temp.path(), &ctx.config);

    let got = EpubDoc::new(&output_file);

    assert!(got.is_ok());

    // also try to run epubcheck, if it's available
    epub_check(&output_file).unwrap();
}

fn epub_check(path: &Path) -> Result<(), Error> {
    init_logging();
    debug!("epub_check in path = {}...", &path.display());
    let cmd = Command::new("epubcheck").arg(path).output();

    match cmd {
        Ok(output) => {
            if output.status.success() {
                Ok(())
            } else {
                let error_from_epubcheck = String::from_utf8_lossy(output.stderr.as_slice());
                error!("Error: {:?}", &error_from_epubcheck);
                Err(Error::EpubCheck(error_from_epubcheck.to_string()))
            }
        }
        Err(err) => {
            // failed to launch epubcheck, it's probably not installed
            debug!("Failed to launch epubcheck, it's probably not installed here...");
            Err(Error::EpubCheck(err.to_string()))
        }
    }
}

#[test]
#[serial]
fn look_for_chapter_1_heading() {
    init_logging();
    debug!("look_for_chapter_1_heading...");
    let mut doc = generate_epub(false).unwrap();
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
}

#[test]
#[serial]
fn look_for_chapter_2_image_link_in_readme() {
    init_logging();
    let mut doc = generate_epub(false).unwrap();

    let path = if cfg!(target_os = "linux") {
        Path::new("OEBPS").join("02_advanced").join("README.html") // linux
    } else {
        Path::new("OEBPS/02_advanced/README.html").to_path_buf() // windows with 'forward slash' /
    };
    let content = doc.0.get_resource_str_by_path(path).unwrap();

    assert!(content.contains("<img src=\"Epub_logo.svg\""));
}

#[test]
#[serial]
fn rendered_document_contains_all_chapter_files_and_assets() {
    init_logging();
    debug!("rendered_document_contains_all_chapter_files_and_assets...");
    let chapters = vec![
        "chapter_1.html",
        "rust-logo.png",
        "02_advanced/README.html",
        "02_advanced/Epub_logo.svg",
    ];
    let mut doc = generate_epub(false).unwrap();
    debug!("Number of internal epub resources = {:?}", doc.0.resources);
    // number of internal epub resources for dummy test book
    assert_eq!(12, doc.0.resources.len());
    assert_eq!(3, doc.0.spine.len());
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
        assert!(!got.unwrap().is_empty());
    }
}

#[test]
#[serial]
fn straight_quotes_transformed_into_curly_quotes() {
    init_logging();
    debug!("straight_quotes_transformed_into_curly_quotes...");
    let mut doc = generate_epub(false).unwrap();
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

#[test]
#[serial]
fn footnote_has_linked_label() {
    init_logging();
    debug!("footnote_has_linked_label...");
    let mut doc = generate_epub(true).unwrap();
    debug!("doc current path = {:?}", doc.1);

    let path = if cfg!(target_os = "linux") {
        Path::new("OEBPS").join("chapter_1.html") // linux
    } else {
        Path::new("OEBPS/chapter_1.html").to_path_buf() // windows with 'forward slash' /
    };
    let file = doc.0.get_resource_str_by_path(path);
    let content = file.unwrap();
    debug!("content = {:?}", content);
    assert!(content.contains("footnotes<sup class=\"footnote-reference\" id=\"fr-example-1\"><a href=\"#fn-example\">[1]</a></sup> with back-references"));
}

#[test]
#[serial]
fn footnote_definition_has_backreference_link() {
    init_logging();
    debug!("footnote_definition_has_backreference_link...");
    let mut doc = generate_epub(true).unwrap();
    debug!("doc current path = {:?}", doc.1);

    let path = if cfg!(target_os = "linux") {
        Path::new("OEBPS").join("chapter_1.html") // linux
    } else {
        Path::new("OEBPS/chapter_1.html").to_path_buf() // windows with 'forward slash' /
    };
    let file = doc.0.get_resource_str_by_path(path);
    let content = file.unwrap();
    debug!("content = {:?}", content);
    assert!(content.contains("<a href=\"#fr-example-1\">↩</a></p>"));
}

#[test]
#[serial]
fn footnote_definition_label() {
    init_logging();
    debug!("footnote_definition_label...");
    let mut doc = generate_epub(true).unwrap();
    debug!("doc current path = {:?}", doc.1);

    let path = if cfg!(target_os = "linux") {
        Path::new("OEBPS").join("chapter_1.html") // linux
    } else {
        Path::new("OEBPS/chapter_1.html").to_path_buf() // windows with 'forward slash' /
    };
    let file = doc.0.get_resource_str_by_path(path);
    let content = file.unwrap();
    debug!("content = {:?}", content);
    assert!(content.contains("<div class=\"footnotes\" epub:type=\"footnotes\">\n<div class=\"footnote-definition\" id=\"fn-example\" epub:type=\"footnote\"><p><span class=\"footnote-definition-label\">[1]</span>"));
}

/// Use `MDBook::load()` to load the dummy book into memory, then set up the
/// `RenderContext` for use the EPUB generator.
fn create_dummy_book(name: &str) -> Result<(RenderContext, MDBook, TempDir), Error> {
    debug!("create_{:?}...", name);
    let temp = TempDir::with_prefix_in("mdbook-epub", ".")?;

    let dummy_book = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(name);
    debug!("{:?} = {:?}", name, &dummy_book.display().to_string());

    let md = MDBook::load(dummy_book);

    let book = md.expect(&format!("{:?} MDBook is not loaded", name));
    let ctx = RenderContext::new(
        book.root.clone(),
        book.book.clone(),
        book.config.clone(),
        temp.path().to_path_buf(),
    );

    Ok((ctx, book, temp))
}
