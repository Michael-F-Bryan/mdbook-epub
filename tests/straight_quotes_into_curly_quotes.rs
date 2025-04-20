use log::debug;
use serial_test::serial;
use std::path::Path;

mod common;
use crate::common::epub::output_epub_is_valid;
use common::epub::generate_epub;
use common::init_logging::init_logging;

#[test]
#[serial]
fn test_straight_quotes_transformed_into_curly_quotes() {
    init_logging();
    debug!("straight_quotes_transformed_into_curly_quotes...");
    let mut doc = generate_epub("straight_quotes_into_curly_quotes").unwrap();
    debug!("doc current path = {:?}", doc.1);

    let path = if cfg!(target_os = "linux") {
        Path::new("OEBPS").join("chapter_1.html") // linux
    } else {
        Path::new("OEBPS/chapter_1.html").to_path_buf() // windows with 'forward slash' /
    };
    let file = doc.0.get_resource_str_by_path(path);
    let content = file.unwrap();
    debug!("content = {:?}", content);
    assert!(content.contains("<p>“One morning, when Gregor Samsa woke from troubled dreams, he found himself ‘transformed’ in his bed into a horrible\nvermin.”</p>"));
}

#[ignore = "CI/CD only"]
#[test]
#[serial]
fn test_output_straight_quotes_is_valid() {
    output_epub_is_valid("straight_quotes_into_curly_quotes");
}
