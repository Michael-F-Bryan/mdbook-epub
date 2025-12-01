use serial_test::serial;
use std::path::Path;
use tracing::debug;
mod common;
use crate::common::epub::{create_dummy_book, output_epub_is_valid};
use common::epub::generate_epub;
use common::init_logging::init_logging;

#[test]
#[serial]
fn test_output_long_book_exists() {
    init_logging();
    debug!("fn output_epub_exists...");
    let (ctx, _md, temp) = create_dummy_book("long_book_example").unwrap();

    let output_file = mdbook_epub::output_filename(temp.path(), &ctx.config);
    assert!(
        output_file.is_ok(),
        "{}",
        format!(
            "output_file is incorrect !: {}",
            output_file.unwrap().display().to_string()
        )
    );
    let output_file = output_file.unwrap();

    assert!(!output_file.exists());
    mdbook_epub::generate(&ctx).unwrap();
    assert!(output_file.exists());
}

#[ignore = "CI/CD only"]
#[test]
#[serial]
fn test_output_long_book_is_valid() {
    output_epub_is_valid("long_book_example");
    // common::epub::output_epub_is_valid_preserve_temp_folder("long_book_example");
}

#[test]
#[serial]
fn test_long_book_lookup_chapter_1_heading() {
    init_logging();
    debug!("look_for_chapter_1_heading...");
    let mut doc = generate_epub("long_book_example").unwrap();
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
fn test_long_book_lookup_chapter_2_image_link_in_readme() {
    init_logging();
    let mut doc = generate_epub("long_book_example").unwrap();
    // let mut doc = common::epub::generate_epub_preserve_temp_folder("long_book_example").unwrap();

    let path = if cfg!(target_os = "linux") {
        Path::new("OEBPS").join("02_advanced").join("README.html") // linux
    } else {
        Path::new("OEBPS/02_advanced/README.html").to_path_buf() // windows with 'forward slash' /
    };
    let content = doc.0.get_resource_str_by_path(path).unwrap();

    assert!(content.contains("<img src=\"Epub_logo.svg\""));
    assert!(content.contains("<img src=\"../assets/rust-logo.png\""));
    assert!(content.contains("<img src=\"../reddit.svg\""));
}

#[test]
#[serial]
fn test_long_book_contains_all_chapter_files_and_assets() {
    init_logging();
    debug!("rendered_document_contains_all_chapter_files_and_assets...");
    let chapters = vec![
        "chapter_1.html",
        "rust-logo.png",
        "02_advanced/README.html",
        "02_advanced/Epub_logo.svg",
    ];
    let mut doc = generate_epub("long_book_example").unwrap();
    debug!("Number of internal epub resources = {:?}", doc.0.resources);
    // number of internal epub resources for long_book_example test book
    assert_eq!(13, doc.0.resources.len());
    assert_eq!(3, doc.0.spine.len());
    assert_eq!(doc.0.mdata("title").unwrap().value, "LongBookExample");
    assert_eq!(doc.0.mdata("language").unwrap().value, "en");
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
