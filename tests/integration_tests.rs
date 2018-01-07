extern crate mdbook;
extern crate mdbook_epub;
extern crate tempdir;

use std::path::Path;
use tempdir::TempDir;
use mdbook::renderer::RenderContext;
use mdbook::MDBook;

fn create_dummy_book() -> (RenderContext, MDBook, TempDir) {
    let temp = TempDir::new("mdbook-epub").unwrap();

    let dummy_book = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("dummy");

    let md = MDBook::load(dummy_book).unwrap();

    let ctx = RenderContext {
        version: mdbook_epub::MDBOOK_VERSION.to_string(),
        root: md.root.clone(),
        book: md.book.clone(),
        config: md.config.clone(),
        destination: temp.path().to_path_buf(),
    };

    (ctx, md, temp)
}

#[test]
fn output_epub_exists() {
    let (ctx, _md, temp) = create_dummy_book();

    let output_file = mdbook_epub::output_filename(temp.path(), &ctx.config);

    assert!(!output_file.exists());
    mdbook_epub::generate(&ctx).unwrap();
    assert!(output_file.exists());
}
