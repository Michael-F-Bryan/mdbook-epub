extern crate failure;
extern crate mdbook;
extern crate mdbook_epub;
extern crate pulldown_cmark;
extern crate serde_json;
extern crate structopt;
#[macro_use]
extern crate structopt_derive;

use std::io;
use std::env;
use std::process;
use std::path::PathBuf;
use failure::{Error, ResultExt, SyncFailure};
use structopt::StructOpt;
use mdbook::renderer::RenderContext;
use mdbook::MDBook;

fn main() {
    let args = Args::from_args();

    if let Err(e) = run(&args) {
        eprintln!("Error: {}", e);

        for cause in e.causes().skip(1) {
            eprintln!("\tCaused By: {}", cause);
        }

        if let Ok(_) = env::var("RUST_BACKTRACE") {
            eprintln!();
            eprintln!("{}", e.backtrace());
        }

        process::exit(1);
    }
}

fn run(args: &Args) -> Result<(), Error> {
    // get a `RenderContext`, either from stdin (because we're used as a plugin)
    // or by instrumenting MDBook directly (in standalone mode).
    let ctx: RenderContext = if args.standalone {
        let md = MDBook::load(&args.root).map_err(SyncFailure::new)?;
        let destination = md.build_dir_for("epub");

        RenderContext {
            version: mdbook_epub::MDBOOK_VERSION.to_string(),
            root: md.root,
            book: md.book,
            config: md.config,
            destination: destination,
        }
    } else {
        serde_json::from_reader(io::stdin()).context("Unable to parse RenderContext")?
    };

    mdbook_epub::generate(&ctx)?;

    Ok(())
}

#[derive(Debug, Clone, StructOpt)]
struct Args {
    #[structopt(short = "s", long = "standalone",
                help = "Run standalone (i.e. not as a mdbook plugin)")]
    standalone: bool,
    #[structopt(help = "The book to render.", parse(from_os_str), default_value = ".")]
    root: PathBuf,
}
