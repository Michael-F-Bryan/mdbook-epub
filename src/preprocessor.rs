use crate::errors::Error;
use mdbook_driver::builtin_preprocessors::LinkPreprocessor;
use mdbook_preprocessor::Preprocessor;
use mdbook_preprocessor::PreprocessorContext;
use mdbook_renderer::RenderContext;
use tracing::debug;

/// Run mdbook's built-in [`LinkPreprocessor`] on the book being processed and return a new
/// `RenderContext` with the preprocessed book.
///
/// Just like in mdbook itself, the preprocessor only runs when it is enabled
/// in the book configuration (`[preprocessor.links]` in `book.toml`), so
/// `Ok(None)` is returned when it is not enabled.
///
/// It is a no-op for already processed books, because the preprocessor only
/// acts on `{{#...}}` shortcodes, which are gone after the first pass.
pub(super) fn run_link_preprocessor(ctx: &RenderContext) -> Result<Option<RenderContext>, Error> {
    let preprocessors = ctx.config.preprocessors::<serde_json::Value>()?;
    // checking [preprocessor.links] setting inside book.toml
    if !preprocessors.contains_key("links") {
        debug!(
            "The LinkPreprocessor is not enabled in book.toml \
             (`[preprocessor.links]` is missing), skipping it"
        );
        return Ok(None);
    }

    debug!("Running the built-in LinkPreprocessor on the book...");
    let preprocess_ctx =
        PreprocessorContext::new(ctx.root.clone(), ctx.config.clone(), "epub".to_string());

    let book = LinkPreprocessor::new().run(&preprocess_ctx, ctx.book.clone())?;

    let mut preprocessed_ctx = ctx.clone();
    preprocessed_ctx.book = book;
    preprocessed_ctx
        .chapter_titles
        .extend(preprocess_ctx.chapter_titles.borrow().clone());
    // context with preprocessed book
    Ok(Some(preprocessed_ctx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdbook_core::book::BookItem;
    use serde_json::json;

    fn render_ctx_with_preprocessor(preprocessor_links: bool, content: &str) -> RenderContext {
        let mut config = json!({
            "book": {"authors": [], "language": "en", "src": "src", "title": "DummyBook"},
            "output": {"epub": {}}
        });
        if preprocessor_links {
            config["preprocessor"] = json!({"links": {}});
        }
        let ctx = json!({
            "version": mdbook_core::MDBOOK_VERSION,
            "root": "tests/long_book_example",
            "book": {"items": [{
                "Chapter": {
                    "name": "Chapter 1",
                    "content": content,
                    "number": [1],
                    "sub_items": [],
                    "path": "chapter_1.md",
                    "parent_names": []
                }}], "__non_exhaustive": null},
            "config": config,
            "destination": "target/epub-test"
        });
        RenderContext::from_json(ctx.to_string().as_bytes()).unwrap()
    }

    #[test]
    fn link_preprocessor_is_skipped_without_config_parameter() {
        // no [preprocessor.links] inside book.toml
        let ctx = render_ctx_with_preprocessor(false, "{{#include some_file.rs}}\n");
        let got = run_link_preprocessor(&ctx).unwrap();
        assert!(got.is_none());
    }

    #[test]
    fn link_preprocessor_runs_when_configured() {
        let ctx = render_ctx_with_preprocessor(
            true,
            "{{#rustdoc_include ../listings/ch02-guessing-game-tutorial/no-listing-04-looping/src/main.rs:here}}\n",
        );
        let got = run_link_preprocessor(&ctx).unwrap().unwrap();

        let mut expanded = false;
        for item in got.book.iter() {
            if let BookItem::Chapter(ch) = item {
                assert!(
                    !ch.content.contains("{{#rustdoc_include"),
                    "the {{#rustdoc_include}} shortcode was not expanded"
                );
                expanded = ch.content.contains("guess.cmp");
            }
        }
        assert!(
            expanded,
            "expected the anchored code from main.rs to be present"
        );
    }
}
