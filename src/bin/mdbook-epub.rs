extern crate failure;
extern crate mdbook;
extern crate mdbook_epub;
extern crate serde_json;

use std::io;
use mdbook::renderer::RenderContext;
use failure::Fail;


fn main() {
    let ctx: RenderContext = serde_json::from_reader(io::stdin()).unwrap();
    if let Err(e) = mdbook_epub::generate(&ctx) {
        eprintln!("Error: {}", e);

        for cause in e.causes().skip(1) {
            eprintln!("\tCaused By: {}", cause);
        }

        if let Some(bt) = e.backtrace() {
            eprintln!();
            eprintln!("{}", bt);
        }
    }
}
