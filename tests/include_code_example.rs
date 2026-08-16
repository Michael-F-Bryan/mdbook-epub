use serial_test::serial;
use std::path::Path;
use tracing::{debug, trace};
mod common;
use crate::common::epub::{generate_epub, output_epub_is_valid};

#[test]
#[serial]
fn test_include_code_example() {
    debug!("test_include_code_example...");
    let doc = generate_epub("include_code_example");
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
    // nested include fails check = assert!(!content.contains("{{#include"));
    // `{{#rustdoc_include}}`, `{{#playground}}
    assert!(!content.contains("{{#rustdoc_include"));
    assert!(!content.contains("{{#playground"));
    // hidden rustdoc lines (`#`) are removed from language code blocks...
    assert!(!content.contains("# fn main() {}"));
    // ...while visible code and `#[...]` attribute lines are kept
    assert!(content.contains("impl&lt;'a&gt; ImportantExcerpt&lt;'a&gt; {"));
    assert!(content.contains("#[cfg(test)]"));
}

#[ignore = "CI/CD only"]
#[test]
#[serial]
fn test_include_code_example_is_valid() {
    output_epub_is_valid("include_code_example");
    // common::epub::output_epub_is_valid_preserve_temp_folder("include_code_example");
}
