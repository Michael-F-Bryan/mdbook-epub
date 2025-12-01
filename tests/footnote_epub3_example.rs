use serial_test::serial;
use std::path::Path;
use tracing::debug;

mod common;
use crate::common::epub::{create_dummy_book, output_epub_is_valid};
use common::epub::generate_epub;
use common::init_logging::init_logging;

#[test]
#[serial]
fn test_footnote_has_linked_label() {
    init_logging();
    debug!("footnote_has_linked_label...");
    let mut doc = generate_epub("footnote_epub3_example").unwrap();
    debug!("doc current path = {:?}", doc.1);

    let path = if cfg!(target_os = "linux") {
        Path::new("OEBPS").join("chapter_1.html") // linux
    } else {
        Path::new("OEBPS/chapter_1.html").to_path_buf() // windows with 'forward slash' /
    };
    let file = doc.0.get_resource_str_by_path(path);
    let content = file.unwrap();
    debug!("content = {:?}", content);
    assert!(content.contains("<sup class=\"footnote-reference\" id=\"fr-example-1\"><a href=\"#fn-example\">[1]</a></sup> with back-references"));
}

#[test]
#[serial]
fn test_footnote_definition_has_backreference_link() {
    init_logging();
    debug!("footnote_definition_has_backreference_link...");
    let mut doc = generate_epub("footnote_epub3_example").unwrap();
    debug!("doc current path = {:?}", doc.1);

    let path = if cfg!(target_os = "linux") {
        Path::new("OEBPS").join("chapter_1.html") // linux
    } else {
        Path::new("OEBPS/chapter_1.html").to_path_buf() // windows with 'forward slash' /
    };
    let file = doc.0.get_resource_str_by_path(path);
    let content = file.unwrap();
    println!("content = \n{:?}", content);
    assert!(content.contains("<a href=\"#fr-example-1\">↩</a></p>"));
}

#[test]
#[serial]
fn test_footnote_definition_label() {
    init_logging();
    debug!("footnote_definition_label...");
    let mut doc = generate_epub("footnote_epub3_example").unwrap();
    debug!("doc current path = {:?}", doc.1);

    let path = if cfg!(target_os = "linux") {
        Path::new("OEBPS").join("chapter_1.html") // linux
    } else {
        Path::new("OEBPS/chapter_1.html").to_path_buf() // windows with 'forward slash' /
    };
    let file = doc.0.get_resource_str_by_path(path);
    let content = file.unwrap();
    println!("content = \n{:?}", content);
    assert!(content.contains("<div class=\"footnotes\" epub:type=\"footnotes\">\n<div class=\"footnote-definition\" id=\"fn-example\" epub:type=\"footnote\"><p><span class=\"footnote-definition-label\">[1]</span>"));
}

#[test]
#[serial]
fn test_output_footnote_book_exists() {
    init_logging();
    debug!("test_output_footnote_book_exists...");
    let (ctx, _md, temp) = create_dummy_book("footnote_epub3_example").unwrap();

    let output_file = mdbook_epub::output_filename(temp.path(), &ctx.config).unwrap();

    assert!(!output_file.exists());
    mdbook_epub::generate(&ctx).unwrap();
    assert!(output_file.exists());
}

#[ignore = "CI/CD only"]
#[test]
#[serial]
fn test_output_footnote_book_is_valid() {
    init_logging();
    debug!("test_output_footnote_book_is_valid...");
    output_epub_is_valid("footnote_epub3_example");
}
