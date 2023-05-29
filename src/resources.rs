use html_parser::{Dom, Node};
use mdbook::book::BookItem;
use mdbook::renderer::RenderContext;
use mime_guess::Mime;
use pulldown_cmark::{Event, Options, Parser, Tag};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Component, MAIN_SEPARATOR_STR, Path, PathBuf};
use url::Url;
use const_format::concatcp;

use crate::Error;

const UPPER_PARENT: &str = "../";
const UPPER_PARENT_STARTS_SLASH: &str = concatcp!(MAIN_SEPARATOR_STR, "..", MAIN_SEPARATOR_STR);

pub(crate) fn find(ctx: &RenderContext) -> Result<HashMap<String, Asset>, Error> {
    let mut assets: HashMap<String, Asset> = HashMap::new();
    debug!("Finding resources by:\n{:?}", ctx.config);
    let src_dir = ctx.root.join(&ctx.config.book.src).canonicalize()?;
    // let src_dir = ctx.root.join(&ctx.config.book.src);

    debug!(
        "Start iteration over a [{:?}] sections in src_dir = {:?}",
        ctx.book.sections.len(),
        src_dir
    );
    for section in ctx.book.iter() {
        match *section {
            BookItem::Chapter(ref ch) => {
                let mut assets_count = 0;
                debug!("Searching links and assets for: '{}'", ch.name);
                if ch.path.is_none() {
                    debug!("'{}' is a draft chapter and should be no content.", ch.name);
                    continue;
                }
                for link in assets_in_markdown(&ch.content)? {
                    let asset = match Url::parse(&link) {
                        Ok(url) => Asset::from_url(url, &ctx.destination),
                        Err(_) => Asset::from_local(&link, &src_dir, ch.path.as_ref().unwrap()),
                    }?;
/*                    let relative = asset.location_on_disk.strip_prefix(&src_dir);
                    match relative {
                        Ok(relative_link_path) => {
                            let link_key: String = String::from(relative_link_path.file_name().unwrap().to_str().unwrap());
                            debug!("Adding asset by link '{:?}' : {:#?}", relative_link_path, &asset);
                            assets.insert(link_key, asset);
                            assets_count += 1;
                        },
                        _ => {
                            error!("We can't add asset outside of book /src/, {:?}", &asset);
                        }
                    }*/
                    // TODO: that way is not correct for EPUB generation, needs change
                    debug!("Adding asset by link '{}' : {:#?}", &link, &asset);
                    assets.insert(link, asset);
                    assets_count += 1;
                }
                debug!("Found '{}' links and assets for: {}", assets_count, ch);
            }
            BookItem::Separator => trace!("Skip separator."),
            BookItem::PartTitle(ref title) => trace!("Skip part title: {}.", title),
        }
    }
    debug!("Added '{}' links and assets in total", assets.len());
    Ok(assets)
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum AssetKind {
    Remote(Url),
    Local(PathBuf),
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) struct Asset {
    /// The asset's absolute location on disk.
    pub(crate) location_on_disk: PathBuf,
    /// The asset's filename relative to the `src/` directory. If it's a remote
    /// asset it relative to the destination where the book generated.
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

    fn from_url(url: Url, dest_dir: &Path) -> Result<Asset, Error> {
        let filename = hash_link(&url);
        let dest_dir = normalize_path(dest_dir);
        let full_filename = dest_dir.join("cache").join(filename);
        // Will fetch assets to normalized path later. fs::canonicalize() only works for existed path.
        let absolute_location = normalize_path(full_filename.as_path());
        let filename = absolute_location.strip_prefix(dest_dir).unwrap();
        let asset = Asset::new(filename, &absolute_location, AssetKind::Remote(url));
        trace!("{:#?}", asset);
        Ok(asset)
    }

    fn from_local(link: &str, src_dir: &Path, chapter_path: &Path) -> Result<Asset, Error> {
        debug!("Composing local asset path for {:?} + {:?} in chapter = {:?}", src_dir, link, chapter_path);
        let chapter_path = src_dir.join(chapter_path);
        // let relative_link = normalize_path(PathBuf::from(link).as_path());
        // Since chapter_path is some file and joined with src_dir, it's safe to
        // unwrap parent here.
        // let parent = chapter_path.parent().unwrap();

        // let full_filename = parent.join(&relative_link);
        let full_filename = Self::join_src_with_link_to_full_path(link, &chapter_path);
        // let full_filename = src_dir.join(&relative_link);

        debug!("Joined full_filename = {:?}", &full_filename.display());
        let absolute_location = full_filename
            .canonicalize()
            .map_err(|this_error| Error::AssetFileNotFound(
                format!("Asset was not found: '{link}' by '{}', error = {}", &full_filename.display(), this_error)))?;
        if !absolute_location.is_file() || absolute_location.is_symlink() {
            return Err(Error::AssetFile(absolute_location));
        }
        // Use filename as embedded file path with content from absolute_location.
        debug!("Extracting file name from = {:?}", &full_filename.display());
        let filename = absolute_location.as_path().file_name().unwrap().to_str().unwrap();

/*        let filename = if full_filename.is_symlink() {
            debug!(
                "Strip symlinked asset '{:?}' prefix without canonicalized path.",
                &relative_link
            );
            full_filename.strip_prefix(src_dir).unwrap()
        } else {
            absolute_location.strip_prefix(src_dir).unwrap()
        };*/
        let asset = Asset::new(
            filename,
            &absolute_location,
            AssetKind::Local(PathBuf::from(link)),
        );
        debug!("[{:#?}] = {:?} : {:?}", asset.source, asset.filename, asset.location_on_disk);
        Ok(asset)
    }

    // Analyses input 'link' and composes full path to it using chapter dir data
    // can pop one folder above the book's src or above one internal sub folder
    fn join_src_with_link_to_full_path(link: &str, chapter_dir: &PathBuf) -> PathBuf {
        let mut reassigned_asset_root: PathBuf = PathBuf::from(chapter_dir);
        let normalized_link = normalize_path(PathBuf::from(link).as_path());
        // if chapter is a MD file, remove if from path
        if chapter_dir.is_file() {
            reassigned_asset_root.pop();
        }
        // if link points to upper folder
        if  !link.is_empty() &&
            (link.starts_with(MAIN_SEPARATOR_STR)
                || link.starts_with(UPPER_PARENT)
                || link.starts_with(UPPER_PARENT_STARTS_SLASH)) {
            reassigned_asset_root.pop(); // remove an one folder from asset's path
        }
        reassigned_asset_root.join(normalized_link) // compose final result
    }
}

fn assets_in_markdown(src: &str) -> Result<Vec<String>, Error> {
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
                                    found.push(dest.clone());
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
    if !found.is_empty() {
        trace!("Assets found in content : {:?}", found);
    }
    Ok(found)
}

pub(crate) fn hash_link(url: &Url) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let path = PathBuf::from(url.path());
    let ext = path
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or_else(|| panic!("Unable to extract file ext from {url}"));
    format!("{:x}.{}", hasher.finish(), ext)
}

// From cargo/util/paths.rs
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut components = path.components().peekable();
    let mut ret = if let Some(c @ Component::Prefix(..)) = components.peek().cloned() {
        components.next();
        PathBuf::from(c.as_os_str())
    } else {
        PathBuf::new()
    };

    for component in components {
        match component {
            Component::Prefix(..) => unreachable!(),
            Component::RootDir => {
                ret.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                ret.pop();
            }
            Component::Normal(c) => {
                ret.push(c);
            }
        }
    }
    ret
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
                    debug!("Cache file {:?} to {} already exists.", dest, url);
                } else {
                    if let Some(cache_dir) = dest.parent() {
                        fs::create_dir_all(cache_dir)?;
                    }
                    debug!("Downloading asset : {}", url);
                    let mut file = OpenOptions::new().create(true).write(true).open(dest)?;
                    let mut resp = self.retrieve(url.as_str())?;
                    io::copy(&mut resp, &mut file)?;
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
                    Ok(Box::new("donwload content".as_bytes()))
                }
            }
            let cr = TestHandler {};
            let a = temp_remote_asset("https://mdbook-epub.org/image.svg").unwrap();
            let r = cr.download(&a);

            assert!(r.is_ok());
            let mut buffer = String::new();
            let mut f = std::fs::File::open(&a.location_on_disk).unwrap();
            f.read_to_string(&mut buffer).unwrap();
            assert_eq!(buffer, "donwload content");
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
    fn find_images() {
        let parent_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/dummy/src");
        let upper_parent_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/dummy");
        let src =
            "![Image 1](./rust-logo.png)\n[a link](to/nowhere) ![Image 2][2]\n\n[2]: reddit.svg\n\
            \n\n<img alt=\"Rust Logo in html\" src=\"reddit.svg\" class=\"center\" style=\"width: 20%;\" />\n\n\
            \n\n![Image 4](assets/rust-logo.png)\n[a link](to/nowhere)
            ![Image 4](../third_party/wikimedia/Epub_logo_color.svg)\n[a link](to/nowhere)";
        let should_be = vec![
            upper_parent_dir.join("third_party/wikimedia/Epub_logo_color.svg").canonicalize().unwrap(),
            parent_dir.join("rust-logo.png").canonicalize().unwrap(),
            parent_dir.join("assets/rust-logo.png").canonicalize().unwrap(),
            parent_dir.join("reddit.svg").canonicalize().unwrap(),
        ];

        let got = assets_in_markdown(src)
            .unwrap()
            .into_iter()
            .map(|a| parent_dir.join(a).canonicalize().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(got, should_be);
    }

    #[test]
    fn find_local_asset() {
        let link = "./rust-logo.png";
        let link2 = "assets/rust-logo.png";
        let link3 = "../third_party/wikimedia/Epub_logo_color.svg";
        let temp = tempdir::TempDir::new("mdbook-epub").unwrap();
        let dest_dir = temp.path().to_string_lossy().to_string();
        let chapters = json!([{
            "Chapter": {
            "name": "Chapter 1",
            "content": format!("# Chapter 1\r\n\r\n![Image]({link})\r\n![Image]({link2})\r\n![Image]({link3})"),
            "number": [1],
            "sub_items": [],
            "path": "chapter_1.md",
            "parent_names": []}
        }]);
        let ctx = ctx_with_chapters(&chapters, &dest_dir).unwrap();

        let mut assets = find(&ctx).unwrap();
        assert!(assets.len() == 3);

        fn assert_asset(a: Asset, link: &str, ctx: &RenderContext) {
            let link_as_path = normalize_path(&PathBuf::from(link));
            let mut src_path = PathBuf::from(&ctx.config.book.src);
            if link.starts_with(UPPER_PARENT) || link.starts_with(UPPER_PARENT_STARTS_SLASH) {
                src_path.pop();
            }
            // let filename = normalize_path(&path);
            let filename = link_as_path.file_name().unwrap();
            let absolute_location = PathBuf::from(&ctx.root)
                // .join(&ctx.config.book.src)
                .join(&src_path)
                .join(&link_as_path)
                .canonicalize()
                .expect("Asset Location is not found");

            let source = AssetKind::Local(PathBuf::from(link));
            let should_be = Asset::new(filename, absolute_location, source);
            assert_eq!(a, should_be);
        }
        assert_asset(assets.remove(link).unwrap(), link, &ctx);
        assert_asset(assets.remove(link2).unwrap(), link2, &ctx);
        assert_asset(assets.remove(link3).unwrap(), link3, &ctx);
    }

    #[test]
    fn find_remote_asset() {
        let link = "https://www.rust-lang.org/static/images/rust-logo-blk.svg";
        let link2 = "https://www.rust-lang.org/static/images/rust-logo-blk.png";
        let link_parsed = Url::parse(link).unwrap();
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
        let got = assets.remove(link).unwrap();

        let filename = PathBuf::from("cache").join(hash_link(&link_parsed));
        let absolute_location = temp.path().join(&filename);
        let source = AssetKind::Remote(link_parsed);
        let should_be = Asset::new(filename, absolute_location, source);
        assert_eq!(got, should_be);
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
            Asset::from_local("a.png", Path::new("tests/dummy/src"), Path::new("ch/a.md"))
                .unwrap_err()
                .to_string()
        );
    }

    #[test]
    #[should_panic(expected =
    "Asset was not found: 'wikimedia' by 'tests/dummy/third_party/a.md/wikimedia', error = No such file or directory (os error 2)")]
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
    fn test_join_chapter_file_with_link_to_full_path() {
        let book_source_root_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/dummy/src");
        let mut book_chapter_dir = PathBuf::from(book_source_root_dir);
        book_chapter_dir.push(Path::new("chapter_1.md"));

        let link = "./asset1.jpg";
        let asset_path = Asset::join_src_with_link_to_full_path(link, &book_chapter_dir);
        assert_eq!(asset_path.as_path(), Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/dummy/src").join("asset1.jpg"));
    }

        #[test]
    fn test_join_chapter_dir_with_link_to_full_path() {
        let mut book_or_chapter_src = PathBuf::from("/media/book/src");

        let link = "./asset1.jpg";
        let asset_path = Asset::join_src_with_link_to_full_path(link, &book_or_chapter_src);
        assert_eq!(asset_path.as_path(), Path::new("/media/book/src/asset1.jpg"));

        let link = "asset1.jpg";
        let asset_path = Asset::join_src_with_link_to_full_path(link, &book_or_chapter_src);
        assert_eq!(asset_path.as_path(), Path::new("/media/book/src/asset1.jpg"));

        let link = "../upper/assets/asset1.jpg";
        let asset_path = Asset::join_src_with_link_to_full_path(link, &book_or_chapter_src);
        assert_eq!(asset_path.as_path(), Path::new("/media/book/upper/assets/asset1.jpg"));

        let link = "assets/asset1.jpg";
        let asset_path = Asset::join_src_with_link_to_full_path(link, &book_or_chapter_src);
        assert_eq!(asset_path.as_path(), Path::new("/media/book/src/assets/asset1.jpg"));

        let link = "./assets/asset1.jpg";
        let asset_path = Asset::join_src_with_link_to_full_path(link, &book_or_chapter_src);
        assert_eq!(asset_path.as_path(), Path::new("/media/book/src/assets/asset1.jpg"));

        book_or_chapter_src = PathBuf::from("/media/book/src/chapter1");

        let link = "../assets/asset1.jpg";
        let asset_path = Asset::join_src_with_link_to_full_path(link, &book_or_chapter_src);
        assert_eq!(asset_path.as_path(), Path::new("/media/book/src/assets/asset1.jpg"));

        let link = "../assets/asset1.jpg";
        let asset_path = Asset::join_src_with_link_to_full_path(link, &book_or_chapter_src);
        assert_eq!(asset_path.as_path(), Path::new("/media/book/src/assets/asset1.jpg"));
    }

    #[test]
    fn incorrect_input_join_chapter_with_link_to_full_path() {
        let book_or_chapter_src = PathBuf::from("/media/book/src");

        let link = "/assets/asset1.jpg";
        let asset_path = Asset::join_src_with_link_to_full_path(link, &book_or_chapter_src);
        assert_eq!(asset_path.as_path(), Path::new("/assets/asset1.jpg"));

        let link = "/../assets/asset1.jpg";
        let asset_path = Asset::join_src_with_link_to_full_path(link, &book_or_chapter_src);
        assert_eq!(asset_path.as_path(), Path::new("/assets/asset1.jpg"));
    }
}
