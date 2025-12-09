use std::collections::HashMap;
use std::path::MAIN_SEPARATOR_STR;

use const_format::concatcp;
use html_parser::{Dom, Element, Node};
use mdbook_core::book::BookItem;
use mdbook_renderer::RenderContext;
use pulldown_cmark::{Event, Tag};
use tracing::{debug, trace, warn};
use url::Url;

use crate::resources::asset::{Asset, AssetKind};
use crate::{Error, utils};

// Internal constants for reveling 'upper folder' paths in resource links inside MD
pub(crate) const UPPER_PARENT: &str = concatcp!("..", MAIN_SEPARATOR_STR);
pub(crate) const UPPER_PARENT_LINUX: &str = concatcp!("..", "/");
pub(crate) const UPPER_PARENT_STARTS_SLASH: &str =
    concatcp!(MAIN_SEPARATOR_STR, "..", MAIN_SEPARATOR_STR);
pub(crate) const UPPER_PARENT_STARTS_SLASH_LINUX: &str = concatcp!("/", "..", "/");

#[cfg(not(target_os = "windows"))]
pub(crate) const UPPER_FOLDER_PATHS: &[&str] =
    &[MAIN_SEPARATOR_STR, UPPER_PARENT, UPPER_PARENT_LINUX];

#[cfg(target_os = "windows")]
pub(crate) const UPPER_FOLDER_PATHS: &[&str] =
    &["/", MAIN_SEPARATOR_STR, UPPER_PARENT, UPPER_PARENT_LINUX];

/// Find all resources in book and put them into HashMap.
/// The key is a link, value is a composed Asset
pub(crate) fn find(ctx: &RenderContext) -> Result<HashMap<String, Asset>, Error> {
    let mut assets: HashMap<String, Asset> = HashMap::new();
    debug!("Finding resources by:\n{:?}", ctx.config);
    let src_dir = ctx.root.join(&ctx.config.book.src).canonicalize()?;

    debug!(
        "Start iteration over a [{:?}] sections in src_dir = {:?}",
        ctx.book.items.len(),
        src_dir
    );
    for section in ctx.book.iter() {
        match *section {
            BookItem::Chapter(ref ch) => {
                let mut assets_count = 0;
                debug!("Searching links and assets for: '{}'", ch);
                if ch.path.is_none() {
                    debug!("'{}' is a draft chapter and should be no content.", ch.name);
                    continue;
                }
                for link in find_assets_in_markdown(&ch.content)? {
                    debug!("'{}' finding Asset...", &link);
                    let asset = if let Ok(url) = Url::parse(&link) {
                        Asset::from_url(&link, url, &ctx.destination)
                    } else {
                        let result = Asset::from_local(&link, &src_dir, ch.path.as_ref().unwrap());
                        if let Err(Error::AssetOutsideSrcDir(_)) = result {
                            warn!("Asset '{link}' is outside source dir '{src_dir:?}' and ignored");
                            continue;
                        };
                        result
                    }?;

                    // that is CORRECT generation way
                    debug!(
                        "Check relative path assets chapter: '{}' for\n{}",
                        ch.name, asset
                    );
                    match asset.source {
                        // local asset kind
                        AssetKind::Local(_) => {
                            let relative = asset.location_on_disk.strip_prefix(&src_dir);
                            match relative {
                                Ok(_relative_link_path) => {
                                    let link_key = asset.original_link.clone();
                                    if let std::collections::hash_map::Entry::Vacant(e) =
                                        assets.entry(link_key.to_owned())
                                    {
                                        debug!(
                                            "Adding asset by link '{:?}' : {}",
                                            link_key, &asset
                                        );
                                        e.insert(asset);
                                        assets_count += 1;
                                    } else {
                                        debug!("Skipped asset for '{}'", link_key);
                                    }
                                }
                                _ => {
                                    // skip incorrect resource/image link outside of book /SRC/ folder
                                    warn!(
                                        "Sorry, we can't add 'Local asset' that is outside of book's /src/ folder, {:?}",
                                        &asset
                                    );
                                }
                            }
                        }
                        AssetKind::Remote(_) => {
                            // remote asset kind
                            let link_key = asset.original_link.clone();
                            debug!("Adding Remote asset by link '{}' : {}", link_key, &asset);
                            assets.insert(link_key, asset);
                            assets_count += 1;
                        }
                    };
                }
                debug!(
                    "Found '{}' links and assets inside '{}'",
                    assets_count, ch.name
                );
            }
            BookItem::Separator => trace!("Skip separator."),
            BookItem::PartTitle(ref title) => trace!("Skip part title: {}.", title),
        }
    }
    debug!("Added '{}' links and assets in total", assets.len());
    Ok(assets)
}

// Look up resources in nested HTML element
fn find_assets_in_nested_html_tags(element: &Element) -> Result<Vec<String>, Error> {
    let mut found_asset = Vec::new();

    if element.name == "img"
        && let Some(dest) = &element.attributes["src"]
    {
        found_asset.push(dest.clone());
    }
    for item in &element.children {
        if let Node::Element(nested_element) = item {
            found_asset.extend(find_assets_in_nested_html_tags(nested_element)?.into_iter());
        }
    }

    Ok(found_asset)
}

// Look up resources in chapter md content
fn find_assets_in_markdown(chapter_src_content: &str) -> Result<Vec<String>, Error> {
    let mut found_asset = Vec::new();

    let pull_down_parser = utils::create_new_pull_down_parser(chapter_src_content);
    // that will process chapter content and find assets
    for event in pull_down_parser {
        match event {
            Event::Start(Tag::Image {
                link_type: _,
                dest_url,
                title: _,
                id: _,
            }) => {
                found_asset.push(dest_url.to_string());
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                let content = html.to_owned().into_string();

                if let Ok(dom) = Dom::parse(&content) {
                    for item in dom.children {
                        if let Node::Element(ref element) = item {
                            found_asset
                                .extend(find_assets_in_nested_html_tags(element)?.into_iter());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    found_asset.sort();
    found_asset.dedup();
    if !found_asset.is_empty() {
        trace!("Assets found in content : {:?}", found_asset);
    }
    Ok(found_asset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[test]
    fn test_find_images() {
        let parent_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/long_book_example/src");
        let upper_parent_dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/long_book_example");
        let src =
            "![Image 1](./rust-logo.png)\n[a link](to/nowhere) ![Image 2][2]\n\n[2]: reddit.svg\n\
            \n\n<img alt=\"Rust Logo in html\" src=\"reddit.svg\" class=\"center\" style=\"width: 20%;\" />\n\n\
            \n\n![Image 4](assets/rust-logo.png)\n[a link](to/nowhere)
            ![Image 4](../third_party/wikimedia/Epub_logo_color.svg)\n[a link](to/nowhere)";
        let should_be = vec![
            upper_parent_dir
                .join("third_party/wikimedia/Epub_logo_color.svg")
                .canonicalize()
                .unwrap(),
            parent_dir.join("rust-logo.png").canonicalize().unwrap(),
            parent_dir
                .join("assets/rust-logo.png")
                .canonicalize()
                .unwrap(),
            parent_dir.join("reddit.svg").canonicalize().unwrap(),
        ];

        let got = find_assets_in_markdown(src)
            .unwrap()
            .into_iter()
            .map(|a| parent_dir.join(a).canonicalize().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(got, should_be);
    }

    #[test]
    fn test_find_local_asset() {
        let link = "./rust-logo.png";
        // link and link2 - are the same asset
        let link2 = "assets/rust-logo.png";
        // not_found_link3 path won't be found because it's outside of src/
        let not_found_link3 = "../third_party/wikimedia/Epub_logo_color.svg";

        let tmp_dir = TempDir::new().unwrap();
        let temp = tmp_dir.path().join("mdbook-epub");
        let dest_dir = temp.as_path().to_string_lossy().to_string();
        let chapters = json!([{
            "Chapter": {
            "name": "Chapter 1",
            "content": format!("# Chapter 1\r\n\r\n![Image]({link})\r\n![Image]({link2})\r\n![Image]({not_found_link3})"),
            "number": [1],
            "sub_items": [],
            "path": "chapter_1.md",
            "parent_names": []}
        }]);
        let ctx = ctx_with_chapters(&chapters, &dest_dir).unwrap();

        let mut assets = find(&ctx).unwrap();
        assert_eq!(2, assets.len());

        fn assert_asset(a: Asset, link: &str, ctx: &RenderContext) {
            let link_as_path = utils::normalize_path(&PathBuf::from(link));
            let mut src_path = PathBuf::from(&ctx.config.book.src);
            if link.starts_with(UPPER_PARENT) || link.starts_with(UPPER_PARENT_STARTS_SLASH) {
                src_path.pop();
            }

            let filename = link_as_path.as_path().to_str().unwrap();
            let absolute_location = PathBuf::from(&ctx.root)
                .join(&src_path)
                .join(&link_as_path)
                .canonicalize()
                .expect("Asset Location is not found");

            let source = AssetKind::Local(PathBuf::from(link));
            let should_be = Asset::new(link.to_string(), filename, absolute_location, source);
            assert_eq!(a, should_be);
        }
        trace!("All = {:?}", assets);
        assert_asset(assets.remove(link).unwrap(), link, &ctx);
        assert_asset(assets.remove(link2).unwrap(), link2, &ctx);
    }

    #[test]
    fn test_find_remote_asset() {
        let link = "https://www.rust-lang.org/static/images/rust-logo-blk.svg";
        let link2 = "https://www.rust-lang.org/static/images/rust-logo-blk.png";
        let link_parsed = Url::parse(link).unwrap();
        let link_parsed2 = Url::parse(link2).unwrap();
        let tmp_dir = TempDir::new().unwrap();
        let temp = tmp_dir.path().join("mdbook-epub");
        let dest_dir = temp.as_path().to_string_lossy().to_string();
        let chapters = json!([
        {"Chapter": {
            "name": "Chapter 1",
            "content": format!("# Chapter 1\r\n\r\n![Image]({link})\r\n<a href=\"\"><img  src=\"{link2}\"></a>"),
            "number": [1],
            "sub_items": [],
            "path": "chapter_1.md",
            "parent_names": []}}]);
        let ctx = ctx_with_chapters(&chapters, &dest_dir).unwrap();

        let mut assets = find(&ctx).unwrap();
        assert_eq!(2, assets.len());

        for (key, value) in assets.clone().into_iter() {
            trace!("{} / {:?}", key, &value);
            match value.source {
                AssetKind::Remote(internal_url) => {
                    let key_to_remove = value.location_on_disk.to_str().unwrap();
                    // let got = assets.remove(key_to_remove).unwrap();
                    let got = assets.remove(key.clone().as_str()).unwrap();
                    let filename = if key_to_remove.contains(".svg") {
                        PathBuf::from("").join(utils::hash_link(&link_parsed))
                    } else {
                        PathBuf::from("").join(utils::hash_link(&link_parsed2))
                    };
                    let absolute_location = temp.as_path().join(&filename);
                    let source = AssetKind::Remote(internal_url);
                    let should_be = Asset::new(key, filename, absolute_location, source);
                    assert_eq!(got, should_be);
                }
                _ => {
                    // only remote urls are processed here for simplicity
                    panic!("Should not be here... only remote urls are used here")
                }
            }
        }
    }

    #[test]
    fn test_find_draft_chapter_without_error() {
        let tmp_dir = TempDir::new().unwrap();
        let temp = tmp_dir.path().join("mdbook-epub");
        let dest_dir = temp.as_path().to_string_lossy().to_string();
        let chapters = json!([
        {"Chapter": {
            "name": "Chapter 1",
            "content": "",
            "number": [1],
            "sub_items": [],
            "path": null,
            "parent_names": []}}]);
        let ctx = ctx_with_chapters(&chapters, &dest_dir).unwrap();
        assert!(find(&ctx).unwrap().is_empty());
    }

    #[test]
    #[should_panic(expected = "Asset was not found")]
    fn test_find_asset_fail_when_chapter_dir_not_exist() {
        panic!(
            "{}",
            Asset::from_local(
                "a.png",
                Path::new("tests\\dummy\\src"),
                Path::new("ch\\a.md")
            )
            .unwrap_err()
            .to_string()
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    #[should_panic(expected = "Asset was not found")]
    fn test_find_asset_fail_when_chapter_dir_not_exist_linux() {
        panic!(
            "{}",
            Asset::from_local(
                "a.png",
                Path::new("tests/long_book_example/src"),
                Path::new("ch/a.md")
            )
            .unwrap_err()
            .to_string()
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    #[should_panic(
        expected = "Asset was not found: 'wikimedia' by 'tests/long_book_example/third_party/a.md/wikimedia', error = No such file or directory (os error 2)"
    )]
    fn test_find_asset_fail_when_it_is_a_dir() {
        panic!(
            "{}",
            Asset::from_local(
                "wikimedia",
                Path::new("tests/long_book_example"),
                Path::new("third_party/a.md")
            )
            .unwrap_err()
            .to_string()
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    #[should_panic(
    //expected = "Asset was not found: 'wikimedia' by 'tests\\dummy\\third_party\\a.md\\wikimedia', error = Системе не удается найти указанный путь. (os error 3)"
    expected = "Asset was not found: 'wikimedia' by 'tests\\dummy\\third_party\\a.md\\wikimedia', error = The system cannot find the path specified. (os error 3)"
    )]
    fn test_find_asset_fail_when_it_is_a_dir_windows() {
        panic!(
            "{}",
            Asset::from_local(
                "wikimedia",
                Path::new("tests\\dummy"),
                Path::new("third_party\\a.md")
            )
            .unwrap_err()
            .to_string()
        );
    }

    fn ctx_with_chapters(
        chapters: &Value,
        destination: &str,
    ) -> Result<RenderContext, mdbook_core::errors::Error> {
        let json_ctx = json!({
            "version": mdbook_core::MDBOOK_VERSION,
            "root": "tests/long_book_example",
            "book": {"items": chapters, "__non_exhaustive": null},
            "config": {
                "book": {"authors": [], "language": "en", "text-direction": "ltr",
                    "src": "src", "title": "DummyBook"},
                "output": {"epub": {"curly-quotes": true}}},
            "destination": destination
        });
        RenderContext::from_json(json_ctx.to_string().as_bytes())
    }

    #[test]
    fn test_compute_asset_path_by_src_and_link_to_full_path() {
        let book_source_root_dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/long_book_example/src");
        let mut book_chapter_dir = book_source_root_dir.clone();
        book_chapter_dir.push(Path::new("chapter_1.md"));

        let link = "./asset1.jpg";
        let asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_chapter_dir);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        let full_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            full_path.as_path(),
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/long_book_example/src")
                .join("asset1.jpg")
        );
    }

    #[test]
    fn test_remove_prefixes() {
        let link_string = String::from("assets/verify.jpeg");
        let link_string = Asset::remove_prefixes(link_string, UPPER_FOLDER_PATHS);
        assert_eq!("assets/verify.jpeg", link_string);

        let link_string = String::from("/assets/verify.jpeg");
        let link_string = Asset::remove_prefixes(link_string, UPPER_FOLDER_PATHS);
        assert_eq!("assets/verify.jpeg", link_string);

        let link_string = String::from("../../assets/verify.jpeg");
        let link_string = Asset::remove_prefixes(link_string, UPPER_FOLDER_PATHS);
        assert_eq!("../assets/verify.jpeg", link_string);
        let new_link = Asset::remove_prefixes(link_string, UPPER_FOLDER_PATHS);
        assert_eq!("assets/verify.jpeg", new_link);

        let upper_folder_path = &[UPPER_PARENT_LINUX, UPPER_PARENT, MAIN_SEPARATOR_STR, "/"];
        let link_string = String::from("assets/verify.jpeg");
        let link_string = Asset::remove_prefixes(link_string, upper_folder_path);
        assert_eq!("assets/verify.jpeg", link_string);

        let link_string = String::from("/assets/verify.jpeg");
        let link_string = Asset::remove_prefixes(link_string, upper_folder_path);
        assert_eq!("assets/verify.jpeg", link_string);

        let link_string = String::from("../../assets/verify.jpeg");
        let link_string = Asset::remove_prefixes(link_string, upper_folder_path);
        assert_eq!("../assets/verify.jpeg", link_string);
        let new_link = Asset::remove_prefixes(link_string, upper_folder_path);
        assert_eq!("assets/verify.jpeg", new_link);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_remove_prefixes_windows() {
        let link_string = String::from("assets\\verify.jpeg");
        let link_string = Asset::remove_prefixes(link_string, UPPER_FOLDER_PATHS);
        assert_eq!("assets\\verify.jpeg", link_string);

        let link_string = String::from("\\assets\\verify.jpeg");
        let link_string = Asset::remove_prefixes(link_string, UPPER_FOLDER_PATHS);
        assert_eq!("assets\\verify.jpeg", link_string);

        let link_string = String::from("..\\..\\assets\\verify.jpeg");
        let link_string = Asset::remove_prefixes(link_string, UPPER_FOLDER_PATHS);
        assert_eq!("..\\assets\\verify.jpeg", link_string);
        let new_link = Asset::remove_prefixes(link_string, UPPER_FOLDER_PATHS);
        assert_eq!("assets\\verify.jpeg", new_link);

        let upper_folder_path = &[UPPER_PARENT_LINUX, UPPER_PARENT, MAIN_SEPARATOR_STR, &"/"];
        let link_string = String::from("assets\\verify.jpeg");
        let link_string = Asset::remove_prefixes(link_string, upper_folder_path);
        assert_eq!("assets\\verify.jpeg", link_string);

        let link_string = String::from("/assets\\verify.jpeg");
        let link_string = Asset::remove_prefixes(link_string, upper_folder_path);
        assert_eq!("assets\\verify.jpeg", link_string);

        let link_string = String::from("..\\..\\assets\\verify.jpeg");
        let link_string = Asset::remove_prefixes(link_string, upper_folder_path);
        assert_eq!("..\\assets\\verify.jpeg", link_string);
        let new_link = Asset::remove_prefixes(link_string, upper_folder_path);
        assert_eq!("assets\\verify.jpeg", new_link);
    }

    #[test]
    fn test_compute_asset_path_by_src_and_link() {
        let mut book_or_chapter_src = ["media", "book", "src"].iter().collect::<PathBuf>();

        let mut link = "./asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            asset_path.as_path().as_os_str(),
            ["media", "book", "src", "asset1.jpg"]
                .iter()
                .collect::<PathBuf>()
                .as_os_str()
        );

        link = "asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            asset_path.as_path(),
            ["media", "book", "src", "asset1.jpg"]
                .iter()
                .collect::<PathBuf>()
        );

        link = "../upper/assets/asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            asset_path.as_path(),
            ["media", "book", "upper", "assets", "asset1.jpg"]
                .iter()
                .collect::<PathBuf>()
        );

        link = "assets/asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            asset_path.as_path(),
            ["media", "book", "src", "assets", "asset1.jpg"]
                .iter()
                .collect::<PathBuf>()
        );

        link = "./assets/asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            asset_path.as_path(),
            ["media", "book", "src", "assets", "asset1.jpg"]
                .iter()
                .collect::<PathBuf>()
        );

        book_or_chapter_src = ["media", "book", "src", "chapter1"]
            .iter()
            .collect::<PathBuf>();

        link = "../assets/asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            asset_path.as_path(),
            ["media", "book", "src", "assets", "asset1.jpg"]
                .iter()
                .collect::<PathBuf>()
        );

        book_or_chapter_src = ["media", "book", "src", "chapter1", "inner"]
            .iter()
            .collect::<PathBuf>();
        link = "../../assets/asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            asset_path.as_path(),
            ["media", "book", "src", "assets", "asset1.jpg"]
                .iter()
                .collect::<PathBuf>()
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_compute_asset_path_by_src_and_link_windows() {
        let mut book_or_chapter_src = ["media", "book", "src"].iter().collect::<PathBuf>();

        let mut link = ".\\asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            asset_path.as_path().as_os_str(),
            (["media", "book", "src", "asset1.jpg"])
                .iter()
                .collect::<PathBuf>()
                .as_os_str()
        );

        link = "asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            asset_path.as_path(),
            ["media", "book", "src", "asset1.jpg"]
                .iter()
                .collect::<PathBuf>()
        );

        link = "..\\upper\\assets\\asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            asset_path.as_path(),
            ["media", "book", "upper", "assets", "asset1.jpg"]
                .iter()
                .collect::<PathBuf>()
        );

        link = "assets\\asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            asset_path.as_path(),
            ["media", "book", "src", "assets", "asset1.jpg"]
                .iter()
                .collect::<PathBuf>()
        );

        link = ".\\assets\\asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            asset_path.as_path(),
            ["media", "book", "src", "assets", "asset1.jpg"]
                .iter()
                .collect::<PathBuf>()
        );

        book_or_chapter_src = ["media", "book", "src", "chapter1"]
            .iter()
            .collect::<PathBuf>();

        link = "..\\assets\\asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            asset_path.as_path(),
            ["media", "book", "src", "assets", "asset1.jpg"]
                .iter()
                .collect::<PathBuf>()
        );

        book_or_chapter_src = ["media", "book", "src", "chapter1", "inner"]
            .iter()
            .collect::<PathBuf>();
        link = "..\\..\\assets\\asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            asset_path.as_path(),
            ["media", "book", "src", "assets", "asset1.jpg"]
                .iter()
                .collect::<PathBuf>()
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_incorrect_compute_asset_path_by_src_and_link() {
        let book_or_chapter_src = ["media", "book", "src"].iter().collect::<PathBuf>();

        let link = "/assets/asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(asset_path.as_path(), Path::new("/assets/asset1.jpg"));

        let link = "/../assets/asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(asset_path.as_path(), Path::new("/assets/asset1.jpg"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_incorrect_compute_asset_path_by_src_and_link_windows() {
        let book_or_chapter_src = ["media", "book", "src"].iter().collect::<PathBuf>();

        let link = "\\assets\\asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(asset_path.as_path(), Path::new("/assets/asset1.jpg"));

        let link = "\\..\\assets/asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(asset_path.as_path(), Path::new("/assets/asset1.jpg"));
    }
}
