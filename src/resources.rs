use failure::{self, Error, ResultExt};
use mdbook::book::BookItem;
use mdbook::renderer::RenderContext;
use mime_guess::{self, Mime};
use pulldown_cmark::{Event, Parser, Tag};
use std::path::{Path, PathBuf};

pub(crate) fn find(ctx: &RenderContext) -> Result<Vec<Asset>, Error> {
    let mut assets = Vec::new();
    let src_dir = ctx
        .root
        .join(&ctx.config.book.src)
        .canonicalize()
        .context("Unable to canonicalize the src directory")?;

    for section in ctx.book.iter() {
        if let BookItem::Chapter(ref ch) = *section {
            log::trace!("Searching {} for links and assets", ch);

            let asset_path = ch.path.as_ref()
                .ok_or_else(|| failure::err_msg(format!("No asset file is found by a path = {:?}", ch.path)))?;

            let full_path = src_dir.join(&asset_path);
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
pub(crate) struct Asset {
    /// The asset's absolute location on disk.
    pub(crate) location_on_disk: PathBuf,
    /// The asset's filename relative to the `src/` directory.
    pub(crate) filename: PathBuf,
    pub(crate) mimetype: Mime,
}

impl Asset {
    fn new<P, Q>(filename: P, absolute_location: Q) -> Asset
    where
        P: Into<PathBuf>,
        Q: Into<PathBuf>,
    {
        let location_on_disk = absolute_location.into();
        let mt = mime_guess::from_path(&location_on_disk).first_or_octet_stream();

        Asset {
            location_on_disk,
            filename: filename.into(),
            mimetype: mt,
        }
    }
}

fn assets_in_markdown(src: &str, parent_dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut found = Vec::new();

    for event in Parser::new(src) {
        if let Event::Start(Tag::Image(_, dest, _)) = event {
            found.push(dest.to_string());
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
        let link = PathBuf::from(normalize_path_sep(link));
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

fn normalize_path_sep(s: String) -> String {
    if std::path::MAIN_SEPARATOR == '\\' {
        s.replace('/', "\\")
    } else {
        s
    }
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
            parent_dir.join("rust-logo.png").canonicalize().unwrap(),
            parent_dir.join("reddit.svg").canonicalize().unwrap(),
        ];

        let got = assets_in_markdown(src, &parent_dir).unwrap();

        assert_eq!(got, should_be);
    }
}
