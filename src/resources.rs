use std::path::{Path, PathBuf};
use mime_guess::{self, Mime};
use mdbook::renderer::RenderContext;
use mdbook::book::BookItem;
use pulldown_cmark::{Event, Parser, Tag};
use failure::{self, Error, ResultExt};

pub fn find(ctx: &RenderContext) -> Result<Vec<Asset>, Error> {
    let mut assets = Vec::new();
    let src_dir = ctx.root
        .join(&ctx.config.book.src)
        .canonicalize()
        .context("Unable to canonicalize the src directory")?;

    for section in ctx.book.iter() {
        if let BookItem::Chapter(ref ch) = *section {
            trace!("Searching {} for links and assets", ch);

            let full_path = src_dir.join(&ch.path);
            let parent = full_path
                .parent()
                .expect("All book chapters have a parent directory");
            let found = assets_in_markdown(&ch.content, parent)?;

            for full_filename in found {
                let relative = full_filename.strip_prefix(&src_dir).unwrap();
                assets.push(Asset::new(relative, &full_filename));
            }
        }
    }

    Ok(assets)
}

#[derive(Clone, PartialEq, Debug)]
pub struct Asset {
    /// The asset's absolute location on disk.
    pub location_on_disk: PathBuf,
    /// The asset's filename relative to the `src/` directory.
    pub filename: PathBuf,
    pub mimetype: Mime,
}

impl Asset {
    fn new<P, Q>(filename: P, absolute_location: Q) -> Asset
    where
        P: Into<PathBuf>,
        Q: Into<PathBuf>,
    {
        let location_on_disk = absolute_location.into();
        let mt = mime_guess::guess_mime_type(&location_on_disk);

        Asset {
            location_on_disk: location_on_disk,
            filename: filename.into(),
            mimetype: mt,
        }
    }
}

fn assets_in_markdown(src: &str, parent_dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut found = Vec::new();

    for event in Parser::new(src) {
        match event {
            Event::Start(Tag::Image(dest, _)) => {
                found.push(dest.into_owned());
            }
            _ => {}
        }
    }

    // TODO: Allow linked images to be either a URL or path on disk

    // I'm assuming you'd just determine if each link is a URL or filename so
    // the `find()` function can put together a deduplicated list of URLs and
    // try to download all of them (in parallel?) to a temporary location. It'd
    // be nice if we could have some sort of caching mechanism by using the
    // destination directory (hash the URL and store it as
    // `book/epub/cache/$hash.$ext`?).
    let mut assets = Vec::new();

    for link in found {
        let link = PathBuf::from(link);
        let filename = parent_dir.join(link);
        let filename = filename.canonicalize().with_context(|_| {
            format!(
                "Unable to fetch the canonical path for {}",
                filename.display()
            )
        })?;

        if !filename.is_file() {
            return Err(failure::err_msg(format!(
                "Asset was not a file, {}",
                filename.display()
            )));
        }

        assets.push(filename);
    }

    Ok(assets)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_images() {
        let parent_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/dummy/src");
        let src =
            "![Image 1](./rust-logo.png)\n[a link](to/nowhere) ![Image 2][2]\n\n[2]: reddit.svg\n";
        let should_be = vec![
            parent_dir.join("rust-logo.png"),
            parent_dir.join("reddit.svg"),
        ];

        let got = assets_in_markdown(src, &parent_dir).unwrap();

        assert_eq!(got, should_be);
    }
}
