use const_format::concatcp;
use html_parser::{Dom, Node};
use mdbook::book::BookItem;
use mdbook::renderer::RenderContext;
use mime_guess::Mime;
use pulldown_cmark::{Event, Tag};
use std::collections::HashMap;
use std::path::{Path, PathBuf, MAIN_SEPARATOR_STR};
use url::Url;

use crate::{utils, Error};

// Internal constants for reveling 'upper folder' paths in resource links inside MD
const UPPER_PARENT: &str = concatcp!("..", MAIN_SEPARATOR_STR);
const UPPER_PARENT_LINUX: &str = concatcp!("..", "/");
const UPPER_PARENT_STARTS_SLASH: &str = concatcp!(MAIN_SEPARATOR_STR, "..", MAIN_SEPARATOR_STR);
const UPPER_PARENT_STARTS_SLASH_LINUX: &str = concatcp!("/", "..", "/");

#[cfg(not(target_os = "windows"))]
const UPPER_FOLDER_PATHS: &[&str] = &[MAIN_SEPARATOR_STR, UPPER_PARENT, UPPER_PARENT_LINUX];

#[cfg(target_os = "windows")]
const UPPER_FOLDER_PATHS: &[&str] = &[&"/", MAIN_SEPARATOR_STR, UPPER_PARENT, UPPER_PARENT_LINUX];

/// Find all resources in book and put them into HashMap.
/// The key is a link, value is a composed Asset
pub(crate) fn find(ctx: &RenderContext) -> Result<HashMap<String, Asset>, Error> {
    let mut assets: HashMap<String, Asset> = HashMap::new();
    debug!("Finding resources by:\n{:?}", ctx.config);
    let src_dir = ctx.root.join(&ctx.config.book.src).canonicalize()?;

    debug!(
        "Start iteration over a [{:?}] sections in src_dir = {:?}",
        ctx.book.sections.len(),
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
                    let asset = match Url::parse(&link) {
                        Ok(url) => Asset::from_url(url, &ctx.destination),
                        Err(_) => Asset::from_local(&link, &src_dir, ch.path.as_ref().unwrap()),
                    }?;

                    // that is CORRECT generation way
                    debug!(
                        "Check relative path assets for: '{}' for {:?}",
                        ch.name, asset
                    );
                    match asset.source {
                        // local asset kind
                        AssetKind::Local(_) => {
                            let relative = asset.location_on_disk.strip_prefix(&src_dir);
                            match relative {
                                Ok(relative_link_path) => {
                                    let link_key: String =
                                        String::from(relative_link_path.to_str().unwrap());
                                    if !assets.contains_key(&link_key) {
                                        debug!(
                                            "Adding asset by link '{:?}' : {:#?}",
                                            link_key, &asset
                                        );
                                        assets.insert(link_key, asset);
                                        assets_count += 1;
                                    } else {
                                        debug!("Skipped asset for '{}'", link_key);
                                    }
                                }
                                _ => {
                                    // skip incorrect resource/image link outside of book /SRC/ folder
                                    warn!("Sorry, we can't add 'Local asset' that is outside of book's /src/ folder, {:?}", &asset);
                                }
                            }
                        }
                        AssetKind::Remote(_) => {
                            // remote asset kind
                            let link_key: String =
                                String::from(asset.location_on_disk.to_str().unwrap());
                            debug!(
                                "Adding Remote asset by link '{:?}' : {:#?}",
                                link_key, &asset
                            );
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

/// The type of asset, remote or local
#[derive(Clone, PartialEq, Debug)]
pub(crate) enum AssetKind {
    Remote(Url),
    Local(PathBuf),
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) struct Asset {
    /// The asset's absolute location on disk.
    pub(crate) location_on_disk: PathBuf,
    /// The local asset's filename relative to the `src/` or `src/assets` directory.
    /// If it's a remote asset it's relative to the destination where the book generated.
    pub(crate) filename: PathBuf,
    pub(crate) mimetype: Mime,
    /// The asset's original link as a enum [local][AssetKind::Local] or [remote][AssetKind::Remote].
    pub(crate) source: AssetKind,
}

impl Asset {
    pub(crate) fn new<P, Q, K>(filename: P, absolute_location: Q, source: K) -> Self
    where
        P: Into<PathBuf>,
        Q: Into<PathBuf>,
        K: Into<AssetKind>,
    {
        let location_on_disk = absolute_location.into();
        let mt = mime_guess::from_path(&location_on_disk).first_or_octet_stream();
        let source = source.into();
        Self {
            location_on_disk,
            filename: filename.into(),
            mimetype: mt,
            source,
        }
    }

    // Create Asset by using remote Url, destination path is used for composing path
    fn from_url(url: Url, dest_dir: &Path) -> Result<Asset, Error> {
        trace!("Extract from URL: {:#?} into folder = {:?}", url, dest_dir);
        let filename = utils::hash_link(&url);
        let dest_dir = utils::normalize_path(dest_dir);
        let full_filename = dest_dir.join(filename);
        // Will fetch assets to normalized path later. fs::canonicalize() only works for existed path.
        let absolute_location = utils::normalize_path(full_filename.as_path());
        let filename = absolute_location.strip_prefix(dest_dir).unwrap();
        let asset = Asset::new(filename, &absolute_location, AssetKind::Remote(url));
        debug!("Created from URL: {:#?}", asset);
        Ok(asset)
    }

    // Create Asset by using local link, source and Chapter path are used for composing fields
    fn from_local(link: &str, src_dir: &Path, chapter_path: &Path) -> Result<Asset, Error> {
        debug!(
            "Composing asset path for {:?} + {:?} in chapter = {:?}",
            src_dir, link, chapter_path
        );
        let chapter_path = src_dir.join(chapter_path);

        // compose file name by it's link and chapter path
        let stripped_path = Self::compute_asset_path_by_src_and_link(link, &chapter_path);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        debug!("Composing full_filename by '{:?}' + '{:?}'", &stripped_path, &normalized_link.clone());
        let full_filename = stripped_path.join(normalized_link); // compose final result

        debug!("Joined full_filename = {:?}", &full_filename.display());
        let absolute_location = full_filename.canonicalize().map_err(|this_error| {
            Error::AssetFileNotFound(format!(
                "Asset was not found: '{link}' by '{}', error = {}",
                &full_filename.display(),
                this_error
            ))
        })?;
        if !absolute_location.is_file() || absolute_location.is_symlink() {
            return Err(Error::AssetFile(absolute_location));
        }
        // Use filename as embedded file path with content from absolute_location.
        let binding = utils::normalize_path(Path::new(link.clone()));
        debug!("Extracting file name from = {:?}, binding = '{binding:?}'", &full_filename.display());
        let filename;
        if cfg!(target_os = "windows") { 
            filename = binding.as_os_str().to_os_string()
            .into_string().expect("Error getting filename for Local Asset").replace("\\", "/"); 
        } else {
            filename = String::from(binding.as_path().to_str().unwrap());
        }

        let asset = Asset::new(
            filename,
            &absolute_location,
            AssetKind::Local(PathBuf::from(link)),
        );
        trace!(
            "[{:#?}] = {:?} : {:?}",
            asset.source,
            asset.filename,
            asset.location_on_disk
        );
        debug!("Created from local: {:#?}", asset);
        Ok(asset)
    }

    // Analyses input 'link' and stripes chapter's path to shorter link
    // can pop one folder above the book's src or above an internal sub folder
    // 'link' is stripped too for one upper folder on one call
    fn compute_asset_path_by_src_and_link(link: &str, chapter_dir: &PathBuf) -> PathBuf {
        let mut reassigned_asset_root: PathBuf = PathBuf::from(chapter_dir);
        let link_string = String::from(link);
        // if chapter is a MD file, remove if from path
        if chapter_dir.is_file() {
            reassigned_asset_root.pop();
        }
        trace!("check if parent present by '{}' = '{}' || '{}' || '{}'",
            link_string, MAIN_SEPARATOR_STR, UPPER_PARENT, UPPER_PARENT_STARTS_SLASH);
        // if link points to upper folder
        if !link_string.is_empty()
            && (link_string.starts_with(MAIN_SEPARATOR_STR)
                || link_string.starts_with(UPPER_PARENT_LINUX)
                || link_string.starts_with(UPPER_PARENT)
                || link_string.starts_with(UPPER_PARENT_STARTS_SLASH)
                || link_string.starts_with(UPPER_PARENT_STARTS_SLASH_LINUX))
        {
            reassigned_asset_root.pop(); // remove an one folder from asset's path
            // make a recursive call
            let new_link = Self::remove_prefixes(link_string, UPPER_FOLDER_PATHS);
            reassigned_asset_root = Self::compute_asset_path_by_src_and_link(&new_link, &reassigned_asset_root);
        }
        reassigned_asset_root // compose final result
    }

    // Strip input link by prefixes from &str array
    // return 'shorter' result or the same
    fn remove_prefixes<'a>(link_to_strip: String, prefixes: &[&str]) -> String {
        let mut stripped_link = String::from(link_to_strip.clone());
        for prefix in prefixes {
            match link_to_strip.strip_prefix(prefix) {
                Some(s) => {
                    stripped_link = s.to_string();
                    return stripped_link
                },
                None => &link_to_strip
            };
        };
        stripped_link
    }
}

// Look up resources in chapter md content
fn find_assets_in_markdown(chapter_src_content: &str) -> Result<Vec<String>, Error> {
    let mut found_asset = Vec::new();

    let pull_down_parser = utils::create_new_pull_down_parser(chapter_src_content);
    // that will process chapter content and find assets
    for event in pull_down_parser {
        match event {
            Event::Start(Tag::Image(_, dest, _)) => {
                found_asset.push(dest.to_string());
            }
            Event::Html(html) => {
                let content = html.into_string();

                if let Ok(dom) = Dom::parse(&content) {
                    for item in dom.children {
                        match item {
                            Node::Element(ref element) if element.name == "img" => {
                                if let Some(dest) = &element.attributes["src"] {
                                    found_asset.push(dest.clone());
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

    found_asset.sort();
    found_asset.dedup();
    if !found_asset.is_empty() {
        trace!("Assets found in content : {:?}", found_asset);
    }
    Ok(found_asset)
}

pub(crate) mod handler {
    use std::{
        fs::{self, File, OpenOptions},
        io::{self, Read},
        path::Path,
    };

    #[cfg(test)]
    use mockall::automock;

    use crate::Error;

    use super::{Asset, AssetKind};

    #[cfg_attr(test, automock)]
    pub(crate) trait ContentRetriever {
        fn download(&self, asset: &Asset) -> Result<(), Error> {
            if let AssetKind::Remote(url) = &asset.source {
                let dest = &asset.location_on_disk;
                if dest.is_file() {
                    debug!("Cache file {:?} to '{}' already exists.", dest, url);
                } else {
                    if let Some(cache_dir) = dest.parent() {
                        fs::create_dir_all(cache_dir)?;
                    }
                    debug!("Downloading asset : {}", url);
                    let mut file = OpenOptions::new().create(true).write(true).open(dest)?;
                    let mut resp = self.retrieve(url.as_str())?;
                    io::copy(&mut resp, &mut file)?;
                    debug!("Downloaded asset by '{}'", url);
                }
            }
            Ok(())
        }
        fn read(&self, path: &Path, buffer: &mut Vec<u8>) -> Result<(), Error> {
            File::open(path)?.read_to_end(buffer)?;
            Ok(())
        }
        fn retrieve(&self, url: &str) -> Result<Box<(dyn Read + Send + Sync + 'static)>, Error>;
    }

    pub(crate) struct ResourceHandler;
    impl ContentRetriever for ResourceHandler {
        fn retrieve(&self, url: &str) -> Result<Box<(dyn Read + Send + Sync + 'static)>, Error> {
            let res = ureq::get(url).call()?;
            match res.status() {
                200 => Ok(res.into_reader()),
                404 => Err(Error::AssetFileNotFound(format!(
                    "Missing remote resource: {url}"
                ))),
                _ => unreachable!("Unexpected response status for '{url}'"),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::ContentRetriever;
        use crate::{resources::Asset, Error};
        use tempdir::TempDir;

        type BoxRead = Box<(dyn std::io::Read + Send + Sync + 'static)>;

        #[test]
        fn download_success() {
            use std::io::Read;

            struct TestHandler;
            impl ContentRetriever for TestHandler {
                fn retrieve(&self, _url: &str) -> Result<BoxRead, Error> {
                    Ok(Box::new("Downloaded content".as_bytes()))
                }
            }
            let cr = TestHandler {};
            let a = temp_remote_asset("https://mdbook-epub.org/image.svg").unwrap();
            let r = cr.download(&a);

            assert!(r.is_ok());
            let mut buffer = String::new();
            let mut f = std::fs::File::open(&a.location_on_disk).unwrap();
            f.read_to_string(&mut buffer).unwrap();
            assert_eq!(buffer, "Downloaded content");
        }

        #[test]
        fn download_fail_when_resource_not_exist() {
            struct TestHandler;
            impl ContentRetriever for TestHandler {
                fn retrieve(&self, url: &str) -> Result<BoxRead, Error> {
                    Err(Error::AssetFileNotFound(format!(
                        "Missing remote resource: {url}"
                    )))
                }
            }
            let cr = TestHandler {};
            let a = temp_remote_asset("https://mdbook-epub.org/not-exist.svg").unwrap();
            let r = cr.download(&a);

            assert!(r.is_err());
            assert!(matches!(r.unwrap_err(), Error::AssetFileNotFound(_)));
        }

        #[test]
        #[should_panic(expected = "NOT 200 or 404")]
        fn download_fail_with_unexpected_status() {
            struct TestHandler;
            impl ContentRetriever for TestHandler {
                fn retrieve(&self, _url: &str) -> Result<BoxRead, Error> {
                    panic!("NOT 200 or 404")
                }
            }
            let cr = TestHandler {};
            let a = temp_remote_asset("https://mdbook-epub.org/bad.svg").unwrap();
            let r = cr.download(&a);

            panic!("{}", r.unwrap_err().to_string());
        }

        fn temp_remote_asset(url: &str) -> Result<Asset, Error> {
            let dest_dir = TempDir::new("mdbook-epub")?;
            Asset::from_url(url::Url::parse(url).unwrap(), dest_dir.path())
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::*;

    #[test]
    fn test_find_images() {
        let parent_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/dummy/src");
        let upper_parent_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/dummy");
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
    fn find_local_asset() {
        let link = "./rust-logo.png";
        // link and link2 - are the same asset
        let link2 = "assets/rust-logo.png";
        // not_found_link3 path won't be found because it's outside of src/
        let not_found_link3 = "../third_party/wikimedia/Epub_logo_color.svg";

        let temp = tempdir::TempDir::new("mdbook-epub").unwrap();
        let dest_dir = temp.path().to_string_lossy().to_string();
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
        assert!(assets.len() == 2);

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
            let should_be = Asset::new(filename, absolute_location, source);
            assert_eq!(a, should_be);
        }
        assert_asset(assets.remove(
            utils::normalize_path(&PathBuf::from(link)).to_str().unwrap()
        ).unwrap(), link, &ctx);
        assert_asset(assets.remove(
            utils::normalize_path(&PathBuf::from(link2)).to_str().unwrap()
        ).unwrap(), link2, &ctx);
    }

    #[test]
    fn find_remote_asset() {
        let link = "https://www.rust-lang.org/static/images/rust-logo-blk.svg";
        let link2 = "https://www.rust-lang.org/static/images/rust-logo-blk.png";
        let link_parsed = Url::parse(link).unwrap();
        let link_parsed2 = Url::parse(link2).unwrap();
        let temp = tempdir::TempDir::new("mdbook-epub").unwrap();
        let dest_dir = temp.path().to_string_lossy().to_string();
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
        assert!(assets.len() == 2);

        for (key, value) in assets.clone().into_iter() {
            trace!("{} / {:?}", key, &value);
            match value.source {
                AssetKind::Remote(internal_url) => {
                    let key_to_remove = value.location_on_disk.to_str().unwrap();
                    let got = assets.remove(key_to_remove).unwrap();
                    let filename;
                    if key_to_remove.contains(".svg") {
                        filename = PathBuf::from("").join(utils::hash_link(&link_parsed));
                    } else {
                        filename = PathBuf::from("").join(utils::hash_link(&link_parsed2));
                    }
                    let absolute_location = temp.path().join(&filename);
                    let source = AssetKind::Remote(internal_url);
                    let should_be = Asset::new(filename, absolute_location, source);
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
    fn find_draft_chapter_without_error() {
        let temp = tempdir::TempDir::new("mdbook-epub").unwrap();
        let dest_dir = temp.into_path().to_string_lossy().to_string();
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
    fn find_asset_fail_when_chapter_dir_not_exist() {
        panic!(
            "{}",
            Asset::from_local("a.png", Path::new("tests\\dummy\\src"), Path::new("ch\\a.md"))
                .unwrap_err()
                .to_string()
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    #[should_panic(expected = "Asset was not found")]
    fn find_asset_fail_when_chapter_dir_not_exist_linux() {
        panic!(
            "{}",
           Asset::from_local("a.png", Path::new("tests/dummy/src"), Path::new("ch/a.md"))
                .unwrap_err()
                .to_string()
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    #[should_panic(
       expected = "Asset was not found: 'wikimedia' by 'tests/dummy/third_party/a.md/wikimedia', error = No such file or directory (os error 2)"
    )]
    fn find_asset_fail_when_it_is_a_dir() {
        panic!(
            "{}",
            Asset::from_local(
                "wikimedia",
                Path::new("tests/dummy"),
                Path::new("third_party/a.md")
            )
            .unwrap_err()
            .to_string()
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    #[should_panic(
       expected = "Asset was not found: 'wikimedia' by 'tests\\dummy\\third_party\\a.md\\wikimedia', error = Системе не удается найти указанный путь. (os error 3)"
       //expected = "Asset was not found: 'wikimedia' by 'tests\\dummy\\third_party\\a.md\\wikimedia', error = The system cannot find the path specified. (os error 3)"
    )]
    fn find_asset_fail_when_it_is_a_dir_windows() {
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
    ) -> Result<RenderContext, mdbook::errors::Error> {
        let json_ctx = json!({
            "version": mdbook::MDBOOK_VERSION,
            "root": "tests/dummy",
            "book": {"sections": chapters, "__non_exhaustive": null},
            "config": {
                "book": {"authors": [], "language": "en", "multilingual": false,
                    "src": "src", "title": "DummyBook"},
                "output": {"epub": {"curly-quotes": true}}},
            "destination": destination
        });
        RenderContext::from_json(json_ctx.to_string().as_bytes())
    }

    #[test]
    fn test_compute_asset_path_by_src_and_link_to_full_path() {
        let book_source_root_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/dummy/src");
        let mut book_chapter_dir = PathBuf::from(book_source_root_dir);
        book_chapter_dir.push(Path::new("chapter_1.md"));

        let link = "./asset1.jpg";
        let asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_chapter_dir);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        let full_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            full_path.as_path(),
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/dummy/src")
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

        let upper_folder_path = &[UPPER_PARENT_LINUX, UPPER_PARENT, MAIN_SEPARATOR_STR, &"/"];
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
            (["media", "book", "src", "asset1.jpg"]).iter().collect::<PathBuf>().as_os_str()
        );

        link = "asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            asset_path.as_path(),
            ["media", "book", "src", "asset1.jpg"].iter().collect::<PathBuf>()
        );

        link = "../upper/assets/asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            asset_path.as_path(),
            ["media", "book", "upper", "assets", "asset1.jpg"].iter().collect::<PathBuf>()
        );

        link = "assets/asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            asset_path.as_path(),
            ["media", "book", "src", "assets", "asset1.jpg"].iter().collect::<PathBuf>()
        );

        link = "./assets/asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            asset_path.as_path(),
            ["media", "book", "src", "assets", "asset1.jpg"].iter().collect::<PathBuf>()
        );

        book_or_chapter_src = ["media", "book", "src", "chapter1"].iter().collect::<PathBuf>();

        link = "../assets/asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            asset_path.as_path(),
            ["media", "book", "src", "assets", "asset1.jpg"].iter().collect::<PathBuf>()
        );

        book_or_chapter_src = ["media", "book", "src", "chapter1", "inner"].iter().collect::<PathBuf>();
        link = "../../assets/asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            asset_path.as_path(),
            ["media", "book", "src", "assets", "asset1.jpg"].iter().collect::<PathBuf>()
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
            (["media", "book", "src", "asset1.jpg"]).iter().collect::<PathBuf>().as_os_str()
        );

        link = "asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            asset_path.as_path(),
            ["media", "book", "src", "asset1.jpg"].iter().collect::<PathBuf>()
        );

        link = "..\\upper\\assets\\asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            asset_path.as_path(),
            ["media", "book", "upper", "assets", "asset1.jpg"].iter().collect::<PathBuf>()
        );

        link = "assets\\asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            asset_path.as_path(),
            ["media", "book", "src", "assets", "asset1.jpg"].iter().collect::<PathBuf>()
        );

        link = ".\\assets\\asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            asset_path.as_path(),
            ["media", "book", "src", "assets", "asset1.jpg"].iter().collect::<PathBuf>()
        );

        book_or_chapter_src = ["media", "book", "src", "chapter1"].iter().collect::<PathBuf>();

        link = "..\\assets\\asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            asset_path.as_path(),
            ["media", "book", "src", "assets", "asset1.jpg"].iter().collect::<PathBuf>()
        );

        book_or_chapter_src = ["media", "book", "src", "chapter1", "inner"].iter().collect::<PathBuf>();
        link = "..\\..\\assets\\asset1.jpg";
        let mut asset_path = Asset::compute_asset_path_by_src_and_link(link, &book_or_chapter_src);
        let normalized_link = utils::normalize_path(PathBuf::from(link).as_path());
        asset_path = asset_path.join(normalized_link); // compose final result
        assert_eq!(
            asset_path.as_path(),
            ["media", "book", "src", "assets", "asset1.jpg"].iter().collect::<PathBuf>()
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn incorrect_compute_asset_path_by_src_and_link() {
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
    fn incorrect_compute_asset_path_by_src_and_link_windows() {
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
