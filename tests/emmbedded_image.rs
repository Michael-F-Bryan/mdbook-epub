use serial_test::serial;
use std::path::Path;
use tracing::{debug, trace};
mod common;
use crate::common::epub::{generate_epub, output_epub_is_valid};

#[test]
#[serial]
fn test_embedded_image_tag() {
    debug!("test_embedded_image_tag...");
    let doc = generate_epub("embedded_image");
    println!("{:?}", doc);
    assert!(doc.is_ok());
    let mut doc = doc.unwrap();
    debug!("doc current path = {:?}", doc.1);

    let path = if cfg!(target_os = "linux") {
        Path::new("OEBPS").join("chapter_1.html") // linux
    } else {
        Path::new("OEBPS/chapter_1.html").to_path_buf() // windows with 'forward slash' /
    };
    let file = doc.0.get_resource_str_by_path(path);
    let content = file.unwrap();
    trace!("content =\n{:?}", content);
    assert!(content.contains("<img src=\"data:image/jpeg;base64,/9j/4AAQSkZJRgABAQEASABIAAD"));
}

#[ignore = "CI/CD only"]
#[test]
#[serial]
fn test_output_embedded_image_is_valid() {
    output_epub_is_valid("remote_image_fetch");
    // common::epub::output_epub_is_valid_preserve_temp_folder("embedded_image");
}
