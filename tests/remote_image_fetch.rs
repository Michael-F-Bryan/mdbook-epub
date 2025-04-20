use log::debug;
use serial_test::serial;
use std::path::Path;
mod common;
use crate::common::epub::{generate_epub, output_epub_is_valid};
use crate::common::init_logging::init_logging;

#[test]
#[serial]
fn test_remote_image_urls() {
    init_logging();
    debug!("test_remote_image_urls...");
    let mut doc = generate_epub("remote_image_fetch").unwrap();
    debug!("doc current path = {:?}", doc.1);

    let path = if cfg!(target_os = "linux") {
        Path::new("OEBPS").join("chapter_1.html") // linux
    } else {
        Path::new("OEBPS/chapter_1.html").to_path_buf() // windows with 'forward slash' /
    };
    let file = doc.0.get_resource_str_by_path(path);
    let content = file.unwrap();
    debug!("content =\n{:?}", content);
    assert!(content.contains("<img src=\"b270cb6837d41f98.png\" alt=\"Image\" />"));
    assert!(content.contains("<img src=\"4dbdb25800b6fa1b.jpg\" alt=\"Image\" />"));
}

#[ignore = "CI/CD only"]
#[test]
#[serial]
fn test_output_remote_image_is_valid() {
    output_epub_is_valid("remote_image_fetch");
    // common::epub::output_epub_is_valid_preserve_temp_folder("remote_image_fetch");
}
