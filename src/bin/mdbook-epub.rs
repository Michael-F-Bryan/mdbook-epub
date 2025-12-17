use std::io;
use std::path::PathBuf;
use std::process;

use clap::Parser;
use mdbook_driver::MDBook;
use mdbook_renderer::RenderContext;
use ::serde_json;

use ::mdbook_epub;
use mdbook_epub::errors::Error;
use mdbook_epub::init_tracing;
use tracing::{debug, error, info};

fn main() {
    init_tracing();
    info!("Booting EPUB generator...");
    let args = Args::parse();
    debug!("prepared generator args = {:?}", args);

    if let Err(e) = run(&args) {
        error!("{}", e);

        process::exit(1);
    }
}

fn run(args: &Args) -> Result<(), Error> {
    debug!("run EPUB book build...");
    // get a `RenderContext`, either from stdin (because it's used as a plugin)
    // or by instrumenting MDBook directly
    let ctx: RenderContext = if args.standalone {
        println!("Running mdbook-epub as standalone app...");
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
        println!(
            "Running mdbook-epub as plugin waiting on the STDIN input. If you wanted to process the files in the current folder, use the -s flag from documentation, See: mdbook-epub --help"
        );
        serde_json::from_reader(io::stdin()).map_err(|_| Error::RenderContext)?
    };
    debug!("calling the main code for epub creation");
    mdbook_epub::generate(&ctx)?;
    println!(
        "Book is READY in directory: '{}'",
        ctx.destination.display()
    );

    Ok(())
}

#[derive(Debug, Clone, Parser)]
#[clap(
    name = "MDBook epub utility",
    about = "MDBook epub utility makes EPUB file from MD source files described by book.toml"
)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(
        short = 's',
        long = "standalone",
        help = "Run standalone (i.e. not as a mdbook plugin)"
    )]
    standalone: bool,

    #[arg(
        help = "Root folder the book to render from",
        value_parser = clap::value_parser!(PathBuf),
        default_value = ".",
        name = "root"
    )]
    root: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_standalone_only() {
        let args = Args::try_parse_from(["test", "--standalone"]).unwrap();
        debug_assert!(args.standalone);
        debug_assert_eq!(args.root, PathBuf::from("."));
    }

    #[test]
    fn test_standalone_with_root_path() {
        let args = Args::try_parse_from(["test", "--standalone", "/some/path"]).unwrap();
        debug_assert!(args.standalone);
        debug_assert_eq!(args.root, PathBuf::from("/some/path"));
    }

    #[test]
    fn test_default_root_default_short() {
        let args = Args::try_parse_from(["test"]).unwrap();
        debug_assert!(!args.standalone);
        debug_assert_eq!(args.root, PathBuf::from("."));
    }

    #[test]
    fn test_short_flag() {
        let args = Args::try_parse_from(["test", "-s"]).unwrap();
        debug_assert!(args.standalone);
        debug_assert_eq!(args.root, PathBuf::from("."));
    }

    #[test]
    fn test_with_root_only() {
        let args = Args::try_parse_from(["test", "/another/path"]).unwrap();
        debug_assert!(!args.standalone);
        debug_assert_eq!(args.root, PathBuf::from("/another/path"));
    }
}
