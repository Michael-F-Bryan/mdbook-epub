use crate::common::init_logging::init_logging;
use epub::doc::EpubDoc;
use log::{debug, error};
use mdbook::renderer::RenderContext;
use mdbook::MDBook;
use mdbook_epub::errors::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// Convenience function for compiling the dummy book into an `EpubDoc`.
pub fn generate_epub(epub_book_name: &str) -> Result<(EpubDoc<BufReader<File>>, PathBuf), Error> {
    debug!("generate_epub: {:?}...", epub_book_name);
    let (ctx, _md, temp) = create_dummy_book(epub_book_name).unwrap();
    debug!("temp dir = {:?}", &temp);
    mdbook_epub::generate(&ctx)?;
    let output_file = mdbook_epub::output_filename(temp.path(), &ctx.config);
    debug!("output_file = {:?}", &output_file.display());

    match EpubDoc::new(&output_file) {
        Ok(epub) => Ok((epub, output_file)),
        Err(err) => {
            error!("dummy book creation error = {:?}", err);
            Err(Error::EpubDocCreate(output_file.display().to_string()))
        }
    }
}

/// Use `MDBook::load()` to load the dummy book into memory, then set up the
/// `RenderContext` for use the EPUB generator.
pub fn create_dummy_book(name: &str) -> Result<(RenderContext, MDBook, TempDir), Error> {
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

pub fn epub_check(path: &Path) -> Result<(), Error> {
    init_logging();
    debug!("epub_check in path = {}...", &path.display());

    // windows workaround
    let epubcheck_cmd = if cfg!(windows) {
        // On Windows run epubcheck via : java -jar
        let epubcheck_path =
            std::env::var("EPUBCHECK_PATH").unwrap_or_else(|_| "epubcheck".to_string());

        Command::new("java")
            .args(&["-jar", &epubcheck_path, path.to_str().unwrap()])
            .output()
    } else {
        // Unix systems run epubcheck directly
        Command::new("epubcheck").arg(path).output()
    };

    match epubcheck_cmd {
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

pub fn output_epub_is_valid(epub_book_name: &str) {
    init_logging();
    debug!("output_epub_is_valid...");
    let (ctx, _md, temp) = create_dummy_book(epub_book_name).unwrap();
    mdbook_epub::generate(&ctx).unwrap();

    let output_file = mdbook_epub::output_filename(temp.path(), &ctx.config);

    let got = EpubDoc::new(&output_file);

    assert!(got.is_ok());

    // also try to run epubcheck, if it's available
    epub_check(&output_file).unwrap();
}
