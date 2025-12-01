use crate::common::init_logging::init_logging;
use epub::doc::EpubDoc;
use mdbook_driver::MDBook;
use mdbook_epub::errors::Error;
use mdbook_renderer::RenderContext;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;
use tracing::{debug, error};

/// Convenience function for compiling the dummy book into an `EpubDoc`.
#[allow(dead_code)]
pub fn generate_epub(epub_book_name: &str) -> Result<(EpubDoc<BufReader<File>>, PathBuf), Error> {
    debug!("generate_epub: {:?}...", epub_book_name);
    let (ctx, _md, temp) = create_dummy_book(epub_book_name).unwrap();
    debug!("temp dir = {:?}", &temp);
    debug!("Before start generate...");
    mdbook_epub::generate(&ctx)?;
    let output_file = mdbook_epub::output_filename(temp.path(), &ctx.config)?;
    debug!("output_file = {:?}", &output_file.display());

    match EpubDoc::new(&output_file) {
        Ok(epub) => Ok((epub, output_file)),
        Err(err) => {
            error!("dummy book creation error = {:?}", err);
            Err(Error::EpubDocCreate(output_file.display().to_string()))
        }
    }
}

#[allow(dead_code)]
pub fn generate_epub_preserve_temp_folder(
    epub_book_name: &str,
) -> Result<(EpubDoc<BufReader<File>>, PathBuf), Error> {
    debug!("generate_epub: {:?}...", epub_book_name);
    let (ctx, _md, temp) = create_dummy_book_preserve_temp_folder(epub_book_name).unwrap();
    debug!("temp dir = {:?}", &temp);
    debug!("Before start generate...");
    mdbook_epub::generate(&ctx)?;
    let output_file = mdbook_epub::output_filename(&temp, &ctx.config)?;
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

pub fn create_dummy_book_preserve_temp_folder(
    name: &str,
) -> Result<(RenderContext, MDBook, PathBuf), Error> {
    debug!("create_{:?}...", name);
    let temp = TempDir::with_prefix_in("mdbook-epub", ".")?;
    let temp_path: PathBuf = temp.keep();
    debug!("Temporary directory preserved at: {:?}", temp_path);

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
        temp_path.to_path_buf(),
    );

    Ok((ctx, book, temp_path))
}

pub fn epub_check(path: &Path) -> Result<(), Error> {
    init_logging();
    debug!("check epub book by path = '{}'...", &path.display());

    // windows workaround
    #[cfg(any(windows, target_os = "linux"))]
    let cmd = {
        // On Windows run epubcheck via : java -jar
        debug!("Windows/Linux environment detected");
        let epubcheck_path =
            std::env::var("EPUBCHECK_JAR").expect("EPUBCHECK_JAR environment variable not set");

        debug!("Current directory: {:?}", std::env::current_dir().unwrap());
        debug!("Epubcheck JAR path: {}", &epubcheck_path);
        debug!("File exists: {}", Path::new(&epubcheck_path).exists());

        Command::new("java")
            .args(&["-jar", &epubcheck_path, path.to_str().unwrap()])
            .output()
    };

    #[cfg(any(target_os = "macos"))]
    let cmd = Command::new("epubcheck").arg(path).output();

    match cmd {
        Ok(output) => {
            // logging for debug
            debug!("Command executed. Status: {}", output.status);
            debug!("stdout: {}", String::from_utf8_lossy(&output.stdout));
            debug!("stderr: {}", String::from_utf8_lossy(&output.stderr));

            if output.status.success() {
                Ok(())
            } else {
                let error_from_epubcheck = String::from_utf8_lossy(output.stderr.as_slice());
                error!("Error running epubcheck: {:?}", &error_from_epubcheck);
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
    assert!(output_file.is_ok());
    let output_file = output_file.unwrap();
    let got = EpubDoc::new(&output_file);

    assert!(got.is_ok());

    // also try to run epubcheck, if it's available
    epub_check(&output_file).unwrap();
}

#[allow(dead_code)]
pub fn output_epub_is_valid_preserve_temp_folder(epub_book_name: &str) {
    init_logging();
    debug!("output_epub_is_valid...");
    let (ctx, _md, temp) = create_dummy_book_preserve_temp_folder(epub_book_name).unwrap();
    mdbook_epub::generate(&ctx).unwrap();

    let output_file = mdbook_epub::output_filename(&temp, &ctx.config);
    assert!(output_file.is_ok());
    let output_file = output_file.unwrap();
    let got = EpubDoc::new(&output_file);

    assert!(got.is_ok());

    // also try to run epubcheck, if it's available
    epub_check(&output_file).unwrap();
}
