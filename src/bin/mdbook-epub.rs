extern crate env_logger;
extern crate mdbook;
extern crate mdbook_epub;
extern crate pulldown_cmark;
extern crate serde_json;
extern crate structopt;

use mdbook::renderer::RenderContext;
use mdbook::MDBook;
use std::io;
use std::path::PathBuf;
use std::process;
use structopt::StructOpt;

use mdbook_epub::Error;

fn main() {
    env_logger::init();
    let args = Args::from_args();
    if let Err(e) = run(&args) {
        log::error!("{}", e);
        process::exit(1);
    }
}

fn run(args: &Args) -> Result<(), Error> {
    // get a `RenderContext`, either from stdin (because we're used as a plugin)
    // or by instrumenting MDBook directly (in standalone mode).
    let ctx: RenderContext = if args.standalone {
        let md = MDBook::load(&args.root)?;
        let destination = md.build_dir_for("epub");

        RenderContext::new(md.root, md.book, md.config, destination)
    } else {
        serde_json::from_reader(io::stdin()).map_err(|_| Error::RenderContext)?
    };

    mdbook_epub::generate(&ctx)?;

    Ok(())
}

#[derive(Debug, Clone, StructOpt)]
struct Args {
    #[structopt(
        short = "s",
        long = "standalone",
        help = "Run standalone (i.e. not as a mdbook plugin)"
    )]
    standalone: bool,
    #[structopt(help = "The book to render.", parse(from_os_str), default_value = ".")]
    root: PathBuf,
}
