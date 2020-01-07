use blake2::Blake2s;
use blake2::Digest;
use failure::{self, Error, ResultExt};
use mdbook::book::BookItem;
use mdbook::book::Chapter;
use mdbook::renderer::RenderContext;
use mime_guess::{self, Mime};
use pulldown_cmark::{Event, Parser, Tag};
use reqwest::blocking::get;
use std::path::{Path, PathBuf};
use std::fs::create_dir_all;
use std::fs::OpenOptions;

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

            assets.extend(assets_from_chapter(ch, &src_dir, &ctx.destination)?);
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

fn asset_from_url(link: &str, out_dir: &Path) -> Result<Asset, Error> {
    let mut hasher = Blake2s::new();
    hasher.input(link);

    let link_hash = format!("{:x}", hasher.result());
    let relative_parent = out_dir.join("epub").join("cache");
    let relative = relative_parent.join(&link_hash);

    create_dir_all(&relative_parent).with_context(|_| format!(
        "Unable to create the cache directory: {}",
        relative_parent.display()
    ))?;

    if !relative.is_file() {
        let mut file = OpenOptions::new().write(true).open(&relative).with_context(|_| format!(
            "Unable to open an image's cache file for writing: {}",
            relative.display()
        ))?;

        let mut downloaded = get(link).with_context(|_| format!(
            "Unable to download the linked image {} to {}",
            link,
            relative.display()
        ))?;

        downloaded.copy_to(&mut file).with_context(|_| format!(
            "Unable to write a downloaded image to {}",
            relative.display()
        ))?;
    }

    let full_filename = relative.canonicalize().with_context(|_| format!(
        "Unable to fetch the canonical path for {}",
        relative.display()
    ))?;

    Ok(Asset::new(relative, &full_filename))
}

fn asset_from_path(link: &str, ch: &Chapter, src_dir: &Path) -> Result<Asset, Error> {
    let ch_path = src_dir.join(&ch.path);
    let ch_dir = ch_path.parent().ok_or_else(|| failure::err_msg(
        "All book chapters have a parent directory"
    ))?;

    let full_filename = ch_dir.join(link);
    let full_filename = full_filename.canonicalize().with_context(|_| format!(
        "Unable to fetch the canonical path for {}",
        full_filename.display()
    ))?;

    if !full_filename.is_file() {
        return Err(failure::err_msg(format!(
            "Asset was not a file, {}",
            full_filename.display()
        )));
    }

    let relative = full_filename.strip_prefix(&src_dir).unwrap();
    Ok(Asset::new(relative, &full_filename))
}

fn asset_from_link(link: &str, ch: &Chapter, src_dir: &Path, out_dir: &Path) -> Result<Asset, Error> {
    if link.starts_with("http:") || link.starts_with("https:") {
        asset_from_url(link, out_dir)
    } else {
        asset_from_path(link, ch, src_dir)
    }
}

fn assets_from_chapter(ch: &Chapter, src_dir: &Path, out_dir: &Path) -> Result<Vec<Asset>, Error> {
    let mut found = Vec::new();

    for event in Parser::new(&ch.content) {
        if let Event::Start(Tag::Image(_, dest, _)) = event {
            found.push(dest.to_string());
        }
    }

    let mut assets = Vec::new();

    for link in found {
        let asset = asset_from_link(&link, ch, src_dir, out_dir)?;
        assets.push(asset);
    }

    Ok(assets)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_images() {
        let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/dummy/src");
        let out_dir = src_dir.join("book");

        let src ="![Image 1](./rust-logo.png)\n[a link](to/nowhere) ![Image 2][2]\n\n[2]: reddit.svg\n";
        let ch = Chapter::new("Test Chapter", src.to_owned(), "chapter_1.md", vec![]);

        let image_1_relative = "rust-logo.png";
        let image_1_absolute = src_dir.join(image_1_relative);
        let image_1 = Asset::new(image_1_relative, image_1_absolute);

        let image_2_relative = "reddit.svg";
        let image_2_absolute = src_dir.join(image_2_relative);
        let image_2 = Asset::new(image_2_relative, image_2_absolute);

        let assets_should_be = vec![image_1, image_2];
        let assets_are = assets_from_chapter(&ch, &src_dir, &out_dir).unwrap();

        assert_eq!(assets_are, assets_should_be);
    }
}
