use serial_test::serial;
mod common;
use crate::common::epub::output_epub_is_valid;

#[ignore = "CI/CD only"]
#[test]
#[serial]
fn test_output_page_break_is_valid() {
    output_epub_is_valid("page_break_example");
}
