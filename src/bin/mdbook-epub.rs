use ::env_logger;
#[macro_use]
extern crate log;
use ::mdbook;
use ::mdbook_epub;
use ::serde_json;
use ::structopt;

use mdbook::renderer::RenderContext;
use mdbook::MDBook;
use std::io;
use std::path::PathBuf;
use std::process;
use structopt::StructOpt;

use mdbook_epub::Error;

fn main() {
    env_logger::init();
    info!("Booting EPUB generator...");
    let args = Args::from_args();
    debug!("prepared generator args = {:?}", args);

    if let Err(e) = run(&args) {
        log::error!("{}", e);

        process::exit(1);
    }
}

fn run(args: &Args) -> Result<(), Error> {
    debug!("run EPUB book build...");
    // get a `RenderContext`, either from stdin (because we're used as a plugin)
    // or by instrumenting MDBook directly (in standalone mode).
    let ctx: RenderContext = if args.standalone {
        let error = format!(
            "book.toml root file is not found by a path {:?}",
            &args.root.display()
        );
        let md = MDBook::load(&args.root).expect(&error);
        let destination = md.build_dir_for("epub");
        debug!(
            "EPUB book destination folder is : {:?}",
            destination.display()
        );
        debug!("EPUB book config is : {:?}", md.config);
        RenderContext::new(md.root, md.book, md.config, destination)
    } else {
        serde_json::from_reader(io::stdin()).map_err(|_| Error::RenderContext)?
    };

    mdbook_epub::generate(&ctx)?;
    info!(
        "Book is READY in directory: '{}'",
        ctx.destination.display()
    );

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
