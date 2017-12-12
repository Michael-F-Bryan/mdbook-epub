extern crate failure;
extern crate mdbook;
extern crate mdbook_epub;
extern crate pulldown_cmark;
extern crate serde_json;

use std::io;
use std::env;
use mdbook::renderer::RenderContext;


fn main() {
    let ctx: RenderContext = serde_json::from_reader(io::stdin()).unwrap();
    if let Err(e) = mdbook_epub::generate(&ctx) {
        eprintln!("Error: {}", e);

        for cause in e.causes().skip(1) {
            eprintln!("\tCaused By: {}", cause);
        }

        if let Ok(_) = env::var("RUST_BACKTRACE") {
            eprintln!();
            eprintln!("{}", e.backtrace());
        }
    }
}
