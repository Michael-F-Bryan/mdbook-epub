use super::Error;
use html_parser::{Dom, Node};
use mdbook::book::BookItem;
use mdbook::renderer::RenderContext;
use mime_guess::{self, Mime};
use pulldown_cmark::{Event, Parser, Options, Tag};
use std::path::{Path, PathBuf};

pub(crate) fn find(ctx: &RenderContext) -> Result<Vec<Asset>, Error> {
    let mut assets = Vec::new();
    debug!("Finding resources by:\n{:?}", ctx.config);
    let src_dir = ctx
        .root
        .join(&ctx.config.book.src)
        .canonicalize()?;

    debug!("Start iteration over a [{:?}] sections in src_dir = {:?}", ctx.book.sections.len(), src_dir);
    for section in ctx.book.iter() {
        if let BookItem::Chapter(ref ch) = *section {
            debug!("Searching links and assets for: {}", ch);

            let asset_path = ch.path.as_ref()
                .ok_or_else(|| Error::AssetFileNotFound(format!("Asset was not found for Chapter {}", ch.name) ))?;
            let full_path = src_dir.join(asset_path);
            debug!("Asset full path = {:?}", full_path);
            let parent = full_path
                .parent()
                .expect("All book chapters have a parent directory");
            let found = assets_in_markdown(&ch.content, parent)?;

            for full_filename in found {
                let relative = full_filename.strip_prefix(&src_dir).unwrap();
                debug!("An relative path to asset: {:?}", full_path);
                assets.push(Asset::new(relative, &full_filename));
            }
        } else {
            debug!("That's odd! Section is not found !");
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

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let pulldown_parser = Parser::new_ext(src, options);

    for event in pulldown_parser {
        match event {
            Event::Start(Tag::Image(_, dest, _)) => {
                found.push(dest.to_string());
            }
            Event::Html(html) => {
                let content = html.into_string();

                if let Ok(dom) = Dom::parse(&content) {
                    for item in dom.children {
                        match item {
                            Node::Element(ref element) if element.name == "img" => {
                                if let Some(dest) = &element.attributes["src"] {
                                    if !dest.starts_with("http") {
                                        found.push(dest.clone());
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }

    found.sort();
    found.dedup();

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
        let filename = filename.canonicalize()?;

        if !filename.is_file() {
            return Err(Error::AssetFile(filename));
        }

        assets.push(filename);
    }
    trace!("Assets found in content : [{}]", assets.len());
    Ok(assets)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_images() {
        let parent_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/dummy/src");
        let src =
            "![Image 1](./rust-logo.png)\n[a link](to/nowhere) ![Image 2][2]\n\n[2]: reddit.svg\n\
            \n\n<img alt=\"Rust Logo in html\" src=\"rust-logo.svg\" class=\"center\" style=\"width: 20%;\" />\n\n\
            ![Image 4](./rust-logo.png)\n[a link](to/nowhere)";
        let should_be = vec![
            parent_dir.join("rust-logo.png").canonicalize().unwrap(),
            parent_dir.join("reddit.svg").canonicalize().unwrap(),
            parent_dir.join("rust-logo.svg").canonicalize().unwrap(),
        ];

        let got = assets_in_markdown(src, &parent_dir).unwrap();

        assert_eq!(got, should_be);
    }
}
