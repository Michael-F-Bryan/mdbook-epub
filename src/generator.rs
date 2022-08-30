use std::{
    collections::HashMap,
    ffi::OsString,
    fmt::{self, Debug, Formatter},
    fs::File,
    io::{Read, Write},
    iter,
    path::{Path, PathBuf},
};

use epub_builder::{EpubBuilder, EpubContent, ZipLibrary};
use handlebars::{Handlebars, RenderError};
use html_parser::{Dom, Node};
use mdbook::book::{BookItem, Chapter};
use mdbook::renderer::RenderContext;
use pulldown_cmark::{html, CowStr, Event, Options, Parser, Tag};
use url::Url;

use crate::config::Config;
use crate::resources::handler::{ContentRetriever, ResourceHandler};
use crate::resources::{self, Asset};
use crate::Error;
use crate::DEFAULT_CSS;

/// The actual EPUB book renderer.
pub struct Generator<'a> {
    ctx: &'a RenderContext,
    builder: EpubBuilder<ZipLibrary>,
    config: Config,
    hbs: Handlebars<'a>,
    assets: HashMap<String, Asset>,
    handler: Box<dyn ContentRetriever>,
}

impl<'a> Generator<'a> {
    pub fn new(ctx: &'a RenderContext) -> Result<Generator<'a>, Error> {
        Self::new_with_handler(ctx, ResourceHandler)
    }

    fn new_with_handler(
        ctx: &'a RenderContext,
        handler: impl ContentRetriever + 'static,
    ) -> Result<Generator<'a>, Error> {
        let handler = Box::new(handler);
        let builder = EpubBuilder::new(ZipLibrary::new()?)?;
        let config = Config::from_render_context(ctx)?;

        let mut hbs = Handlebars::new();
        hbs.register_template_string("index", config.template()?)
            .map_err(|_| Error::TemplateParse)?;

        Ok(Generator {
            builder,
            ctx,
            config,
            hbs,
            assets: HashMap::new(),
            handler,
        })
    }

    fn populate_metadata(&mut self) -> Result<(), Error> {
        self.builder.metadata("generator", "mdbook-epub")?;

        if let Some(title) = self.ctx.config.book.title.clone() {
            self.builder.metadata("title", title)?;
        } else {
            warn!("No `title` attribute found yet all EPUB documents should have a title");
        }

        if let Some(desc) = self.ctx.config.book.description.clone() {
            self.builder.metadata("description", desc)?;
        }

        if !self.ctx.config.book.authors.is_empty() {
            self.builder
                .metadata("author", self.ctx.config.book.authors.join(", "))?;
        }

        self.builder.metadata("generator", env!("CARGO_PKG_NAME"))?;

        if let Some(lang) = self.ctx.config.book.language.clone() {
            self.builder.metadata("lang", lang)?;
        } else {
            self.builder.metadata("lang", "en")?;
        }

        Ok(())
    }

    pub fn generate<W: Write>(mut self, writer: W) -> Result<(), Error> {
        info!("Generating the EPUB book");

        self.populate_metadata()?;
        self.find_assets()?;
        self.generate_chapters()?;

        self.add_cover_image()?;
        self.embed_stylesheets()?;
        self.additional_assets()?;
        self.additional_resources()?;
        self.builder.generate(writer)?;
        info!("Generating the EPUB book - DONE !");
        Ok(())
    }

    /// Find assets for adding to the document later. For remote linked assets, they would be
    /// rendered differently in the document by provided information of assets.
    fn find_assets(&mut self) -> Result<(), Error> {
        let error = String::from("Failed finding/fetch resource taken from content? Look up content for possible error...");
        // resources::find can emit very unclear error based on internal MD content,
        // so let's give a tip to user in error message
        let assets = resources::find(self.ctx).map_err(|e| {
            error!("{}", error);
            e
        })?;
        self.assets.extend(assets);
        Ok(())
    }

    fn generate_chapters(&mut self) -> Result<(), Error> {
        debug!("Rendering Chapters");

        for item in &self.ctx.book.sections {
            if let BookItem::Chapter(ref ch) = *item {
                trace!("Adding chapter \"{}\"", ch);
                self.add_chapter(ch)?;
            }
        }

        Ok(())
    }

    fn add_chapter(&mut self, ch: &Chapter) -> Result<(), Error> {
        let rendered = self.render_chapter(ch)?;

        let content_path = ch.path.as_ref().ok_or_else(|| {
            Error::ContentFileNotFound(format!(
                "Content file was not found for Chapter {}",
                ch.name
            ))
        })?;
        trace!(
            "add a chapter {:?} by a path = {:?}",
            &ch.name,
            content_path
        );
        let path = content_path.with_extension("html").display().to_string();
        let mut content = EpubContent::new(path, rendered.as_bytes()).title(format!("{}", ch));

        let level = ch.number.as_ref().map(|n| n.len() as i32 - 1).unwrap_or(0);
        content = content.level(level);

        self.builder.add_content(content)?;

        // second pass to actually add the sub-chapters
        for sub_item in &ch.sub_items {
            if let BookItem::Chapter(ref sub_ch) = *sub_item {
                trace!("add sub-item = {:?}", sub_ch.name);
                self.add_chapter(sub_ch)?;
            }
        }

        Ok(())
    }

    pub fn new_cmark_parser(text: &str) -> Parser<'_, '_> {
        let mut opts = Options::empty();
        opts.insert(Options::ENABLE_TABLES);
        opts.insert(Options::ENABLE_FOOTNOTES);
        opts.insert(Options::ENABLE_STRIKETHROUGH);
        opts.insert(Options::ENABLE_TASKLISTS);
        Parser::new_ext(text, opts)
    }

    /// Render the chapter into its fully formed HTML representation.
    fn render_chapter(&self, ch: &Chapter) -> Result<String, RenderError> {
        let chapter_dir = if let Some(chapter_file_path) = &ch.path {
            chapter_file_path.parent().ok_or_else(|| {
                RenderError::new(format!("No CSS found by a path = {:?}", ch.path))
            })?
        } else {
            return Err(RenderError::new(format!(
                "Draft chapter: {} could not be rendered.",
                ch.name
            )));
        };
        let mut body = String::new();
        let p = Generator::new_cmark_parser(&ch.content);
        let mut quote_converter = EventQuoteConverter::new(self.config.curly_quotes);
        let ch_depth = chapter_dir.components().count();
        let asset_link_filter = AssetLinkFilter::new(&self.assets, ch_depth);
        let events = p
            .map(|event| quote_converter.convert(event))
            .map(|event| asset_link_filter.apply(event));

        html::push_html(&mut body, events);

        let stylesheet_path = chapter_dir
            .components()
            .map(|_| "..")
            .chain(iter::once("stylesheet.css"))
            .collect::<Vec<_>>()
            .join("/");

        let ctx = json!({ "title": ch.name, "body": body, "stylesheet": stylesheet_path });

        self.hbs.render("index", &ctx)
    }

    /// Generate the stylesheet and add it to the document.
    fn embed_stylesheets(&mut self) -> Result<(), Error> {
        debug!("Embedding stylesheets");

        let stylesheet = self.generate_stylesheet()?;
        self.builder.stylesheet(stylesheet.as_slice())?;

        Ok(())
    }

    fn additional_assets(&mut self) -> Result<(), Error> {
        debug!("Embedding additional assets");

        // TODO: have a list of Asset URLs and try to download all of them (in parallel?)
        // to a temporary location.
        for asset in self.assets.values() {
            self.handler.download(asset)?;
            debug!("Embedding asset : {}", asset.filename.display());
            let mut content = Vec::new();
            self.handler
                .read(&asset.location_on_disk, &mut content)
                .map_err(|_| Error::AssetOpen)?;
            let mt = asset.mimetype.to_string();
            self.builder.add_resource(&asset.filename, &*content, mt)?;
        }
        Ok(())
    }

    fn additional_resources(&mut self) -> Result<(), Error> {
        debug!("Embedding additional resources");

        for path in self.config.additional_resources.iter() {
            debug!("Embedding resource: {:?}", path);

            let full_path: PathBuf;
            if let Ok(full_path_internal) = path.canonicalize() {
                // try process by 'path only' first
                debug!("Found resource by a path = {:?}", full_path_internal);
                full_path = full_path_internal; // OK
            } else {
                debug!("Failed to find resource by path, trying to compose 'root + src + path'...");
                // try process by using 'root + src + path'
                let full_path_composed = self
                    .ctx
                    .root
                    .join(self.ctx.config.book.src.clone())
                    .join(path);
                debug!("Try embed resource by a path = {:?}", full_path_composed);
                if let Ok(full_path_src) = full_path_composed.canonicalize() {
                    full_path = full_path_src; // OK
                } else {
                    // try process by using 'root + path' finally
                    let mut error = format!(
                        "Failed to find resource file by 'root + src + path' = {:?}",
                        full_path_composed
                    );
                    warn!("{:?}", error);
                    debug!("Failed to find resource, trying to compose by 'root + path' only...");
                    let full_path_composed = self.ctx.root.join(path);
                    error = format!(
                        "Failed to find resource file by a root + path = {:?}",
                        full_path_composed
                    );
                    full_path = full_path_composed.canonicalize().expect(&error);
                }
            }
            let mt = mime_guess::from_path(&full_path).first_or_octet_stream();

            let content = File::open(&full_path).map_err(|_| Error::AssetOpen)?;
            debug!("Adding resource: {:?} / {:?} ", path, mt.to_string());
            self.builder.add_resource(path, content, mt.to_string())?;
        }

        Ok(())
    }

    fn add_cover_image(&mut self) -> Result<(), Error> {
        debug!("Adding cover image...");

        if let Some(ref path) = self.config.cover_image {
            let full_path: PathBuf;
            if let Ok(full_path_internal) = path.canonicalize() {
                debug!("Found resource by a path = {:?}", full_path_internal);
                full_path = full_path_internal;
            } else {
                debug!("Failed to find resource, trying to compose path...");
                let full_path_composed = self
                    .ctx
                    .root
                    .join(self.ctx.config.book.src.clone())
                    .join(path);
                debug!("Try cover image by a path = {:?}", full_path_composed);
                let error = format!(
                    "Failed to find cover image by full path-name = {:?}",
                    full_path_composed
                );
                full_path = full_path_composed.canonicalize().expect(&error);
            }
            let mt = mime_guess::from_path(&full_path).first_or_octet_stream();

            let content = File::open(&full_path).map_err(|_| Error::AssetOpen)?;
            debug!("Adding cover image: {:?} / {:?} ", path, mt.to_string());
            self.builder
                .add_cover_image(path, content, mt.to_string())?;
        }

        Ok(())
    }

    /// Concatenate all provided stylesheets into one long stylesheet.
    fn generate_stylesheet(&self) -> Result<Vec<u8>, Error> {
        let mut stylesheet = Vec::new();

        if self.config.use_default_css {
            stylesheet.extend(DEFAULT_CSS.as_bytes());
        }

        for additional_css in &self.config.additional_css {
            debug!("generating stylesheet: {:?}", &additional_css);
            let full_path: PathBuf;
            if let Ok(full_path_internal) = additional_css.canonicalize() {
                debug!("Found stylesheet by a path = {:?}", full_path_internal);
                full_path = full_path_internal;
            } else {
                debug!("Failed to find stylesheet, trying to compose path...");
                let full_path_composed = self.ctx.root.join(additional_css);
                debug!("Try stylesheet by a path = {:?}", full_path_composed);
                let error = format!(
                    "Failed to find stylesheet by full path-name = {:?}",
                    full_path_composed
                );
                full_path = full_path_composed.canonicalize().expect(&error);
            }
            let mut f = File::open(&full_path).map_err(|_| Error::CssOpen(full_path.clone()))?;
            f.read_to_end(&mut stylesheet)
                .map_err(|_| Error::StylesheetRead)?;
        }
        debug!("found style(s) = [{}]", stylesheet.len());
        Ok(stylesheet)
    }
}

impl<'a> Debug for Generator<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Generator")
            .field("ctx", &self.ctx)
            .field("builder", &self.builder)
            .field("config", &self.config)
            .field("assets", &self.assets.keys())
            .finish()
    }
}

struct AssetLinkFilter<'a> {
    assets: &'a HashMap<String, Asset>,
    depth: usize,
}

impl<'a> AssetLinkFilter<'a> {
    fn new(assets: &'a HashMap<String, Asset>, depth: usize) -> Self {
        Self { assets, depth }
    }
    fn apply(&self, event: Event<'a>) -> Event<'a> {
        match event {
            Event::Start(Tag::Image(ty, ref url, ref title)) => {
                if let Some(asset) = self.assets.get(&url.to_string()) {
                    // replace original link with `cache/<hash.ext>` in book.
                    let new = self.path_prefix(asset.filename.as_path());
                    Event::Start(Tag::Image(ty, CowStr::from(new), title.to_owned()))
                } else {
                    event
                }
            }
            Event::Html(ref html) => {
                let mut found = Vec::new();
                if let Ok(dom) = Dom::parse(&html.clone().into_string()) {
                    for item in dom.children {
                        match item {
                            Node::Element(ref element) if element.name == "img" => {
                                if let Some(dest) = &element.attributes["src"] {
                                    if Url::parse(dest).is_ok() {
                                        debug!("Found a valid remote img src:\"{}\".", dest);
                                        found.push(dest.to_owned());
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                if found.is_empty() {
                    event
                } else {
                    found.dedup();
                    let mut content = html.clone().into_string();
                    for link in found {
                        if let Some(asset) = self.assets.get(link.as_str()) {
                            let new = self.path_prefix(asset.filename.as_path());
                            content = content.replace(link.as_str(), &CowStr::from(new));
                        } else {
                            unreachable!("{link} should be replaced, but it doesn't.");
                        }
                    }
                    Event::Html(CowStr::from(content))
                }
            }
            _ => event,
        }
    }
    fn path_prefix(&self, path: &Path) -> String {
        // compatible to Windows, translate to forawrd slash in file path.
        let mut fsp = OsString::new();
        for (i, component) in path.components().enumerate() {
            if i > 0 {
                fsp.push("/");
            }
            fsp.push(component);
        }
        let filename = match fsp.into_string() {
            Ok(s) => s,
            Err(orig) => orig.to_string_lossy().to_string(),
        };
        (0..self.depth)
            .map(|_| "..")
            .chain(iter::once(filename.as_str()))
            .collect::<Vec<_>>()
            .join("/")
    }
}

/// From `mdbook/src/utils/mod.rs`, where this is a private struct.
struct EventQuoteConverter {
    enabled: bool,
    convert_text: bool,
}

impl EventQuoteConverter {
    fn new(enabled: bool) -> Self {
        EventQuoteConverter {
            enabled,
            convert_text: true,
        }
    }

    fn convert<'a>(&mut self, event: Event<'a>) -> Event<'a> {
        if !self.enabled {
            return event;
        }

        match event {
            Event::Start(Tag::CodeBlock(_)) => {
                self.convert_text = false;
                event
            }
            Event::End(Tag::CodeBlock(_)) => {
                self.convert_text = true;
                event
            }
            Event::Text(ref text) if self.convert_text => {
                Event::Text(CowStr::from(convert_quotes_to_curly(text)))
            }
            _ => event,
        }
    }
}

fn convert_quotes_to_curly(original_text: &str) -> String {
    // We'll consider the start to be "whitespace".
    let mut preceded_by_whitespace = true;

    original_text
        .chars()
        .map(|original_char| {
            let converted_char = match original_char {
                '\'' => {
                    if preceded_by_whitespace {
                        '‘'
                    } else {
                        '’'
                    }
                }
                '"' => {
                    if preceded_by_whitespace {
                        '“'
                    } else {
                        '”'
                    }
                }
                _ => original_char,
            };

            preceded_by_whitespace = original_char.is_whitespace();

            converted_char
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use mime_guess::mime;
    use std::path::Path;

    use super::*;
    use crate::resources::{handler::MockContentRetriever, AssetKind};

    #[test]
    fn load_assets() {
        let png = "rust-logo.png";
        let svg = "rust-logo.svg";
        let url = "https://www.rust-lang.org/static/images/rust-logo-blk.svg";
        let content = format!(
            "# Chapter 1\n\n\
            ![Rust Logo]({png})\n\n\
            ![Rust Logo remote]({url})\n\n\
            <img alt=\"Rust Logo in html\" src=\"{svg}\" />\n"
        );
        let destination = tempdir::TempDir::new("mdbook-epub").unwrap();
        let json = ctx_with_template(&content, "src", destination.path()).to_string();
        let ctx = RenderContext::from_json(json.as_bytes()).unwrap();

        let mut mock_client = MockContentRetriever::new();
        mock_client.expect_download().times(3).returning(|_| Ok(()));
        // checks local path of assets
        let book_source = PathBuf::from(&ctx.root)
            .join(&ctx.config.book.src)
            .canonicalize()
            .unwrap();
        let should_be_png = book_source.join(png);
        let should_be_svg = book_source.join(svg);
        let hashed_filename = resources::hash_link(&url.parse::<Url>().unwrap());
        let should_be_url = destination.path().join("cache").join(hashed_filename);
        for should_be in [should_be_svg, should_be_png, should_be_url] {
            mock_client
                .expect_read()
                .times(1)
                .withf(move |path, _| path == should_be)
                .returning(|_, _| Ok(()));
        }

        let mut g = Generator::new_with_handler(&ctx, mock_client).unwrap();
        g.find_assets().unwrap();
        assert_eq!(g.assets.len(), 3);
        g.additional_assets().unwrap();
    }

    #[test]
    fn render_assets() {
        let links = vec![
            "local.webp",
            "http://server/remote.svg",
            "http://server/link.png",
        ];
        let root = tempdir::TempDir::new("mdbook-epub").unwrap();
        let mut assets = HashMap::new();
        assets.insert(
            links[0].to_string(),
            Asset {
                location_on_disk: root.path().join("src").join(links[0]),
                filename: PathBuf::from(links[0]),
                mimetype: "image/webp".parse::<mime::Mime>().unwrap(),
                source: AssetKind::Local(PathBuf::from(links[0])),
            },
        );
        let url = Url::parse(links[1]).unwrap();
        let hashed_filename = resources::hash_link(&url);
        let hashed_path = Path::new("cache").join(&hashed_filename);
        assets.insert(
            links[1].to_string(),
            Asset {
                location_on_disk: root.path().join("book").join(&hashed_path),
                filename: hashed_path,
                mimetype: "image/svg+xml".parse::<mime::Mime>().unwrap(),
                source: AssetKind::Remote(url),
            },
        );
        let markdown_str = format!(
            "Chapter 1\n\
            =====\n\n\
            * [link]({})\n\
            * ![Local Image]({})\n\
            * <img alt=\"Remote Image\" src=\"{}\" >\n",
            links[2], links[0], links[1]
        );

        let filter = AssetLinkFilter::new(&assets, 0);
        let parser = Parser::new(&markdown_str);
        let events = parser.map(|ev| filter.apply(ev));
        let mut html_buf = String::new();
        html::push_html(&mut html_buf, events);

        assert_eq!(
            html_buf,
            format!(
                "<h1>Chapter 1</h1>\n\
                <ul>\n\
                <li><a href=\"{}\">link</a></li>\n\
                <li><img src=\"{}\" alt=\"Local Image\" /></li>\n\
                <li><img alt=\"Remote Image\" src=\"cache/{}\" >\n\
                </li>\n\
                </ul>\n",
                links[2], links[0], hashed_filename
            )
        );
    }

    #[test]
    fn render_remote_assets_in_sub_chapter() {
        let link = "https://mdbook.epub/dummy.svg";
        let dest_dir = tempdir::TempDir::new("mdbook-epub").unwrap();
        let ch1_1 = json!({
            "Chapter": {
                "name": "subchapter",
                "content": format!("# Subchapter\n\n![Image]({link})"),
                "number": [1,1],
                "sub_items": [],
                "path": "chapter_1/subchapter.md",
                "parent_names": ["Chapter 1"]
            }
        });
        let ch1 = json!({
            "Chapter": {
                "name": "Chapter 1",
                "content": format!("# Chapter 1\n\n![Image]({link})"),
                "number": [1],
                "sub_items": [ch1_1],
                "path": "chapter_1/index.md",
                "parent_names": []
            }
        });
        let ch2 = json!({
            "Chapter": {
                "name": "Chapter 2",
                "content": format!("# Chapter 2\n\n![Image]({link})"),
                "number": [2],
                "sub_items": [],
                "path": "chapter_2.md",
                "parent_names": []
            }
        });
        let mut json = ctx_with_template("", "src", dest_dir.path());
        let chvalue = json["book"]["sections"].as_array_mut().unwrap();
        chvalue.clear();
        chvalue.push(ch1);
        chvalue.push(ch2);

        let ctx = RenderContext::from_json(json.to_string().as_bytes()).unwrap();
        let mut g = Generator::new(&ctx).unwrap();
        g.find_assets().unwrap();
        assert_eq!(g.assets.len(), 1);

        let pat = |heading, prefix| {
            format!(
                "<h1>{}</h1>\n<p><img src=\"{}cache/811c431d49ec880b.svg\"",
                heading, prefix
            )
        };
        if let BookItem::Chapter(ref ch) = ctx.book.sections[0] {
            let rendered: String = g.render_chapter(ch).unwrap();
            assert!(rendered.contains(&pat("Chapter 1", "../")));

            if let BookItem::Chapter(ref sub_ch) = ch.sub_items[0] {
                let sub_rendered = g.render_chapter(sub_ch).unwrap();
                assert!(sub_rendered.contains(&pat("Subchapter", "../")));
            } else {
                panic!();
            }
        } else {
            panic!();
        }
        if let BookItem::Chapter(ref ch) = ctx.book.sections[1] {
            let rendered: String = g.render_chapter(ch).unwrap();
            assert!(rendered.contains(&pat("Chapter 2", "")));
        } else {
            panic!();
        }
    }

    #[test]
    #[should_panic]
    fn find_assets_with_wrong_src_dir() {
        let json = ctx_with_template(
            "# Chapter 1\n\n",
            "nosuchsrc",
            tempdir::TempDir::new("mdbook-epub").unwrap().path(),
        )
        .to_string();
        let ctx = RenderContext::from_json(json.as_bytes()).unwrap();
        let mut g = Generator::new(&ctx).unwrap();
        g.find_assets().unwrap();
    }

    fn ctx_with_template(content: &str, source: &str, destination: &Path) -> serde_json::Value {
        json!({
            "version": mdbook::MDBOOK_VERSION,
            "root": "tests/dummy",
            "book": {"sections": [{
                "Chapter": {
                    "name": "Chapter 1",
                    "content": content,
                    "number": [1],
                    "sub_items": [],
                    "path": "chapter_1.md",
                    "parent_names": []
                }}], "__non_exhaustive": null},
            "config": {
                "book": {"authors": [], "language": "en", "multilingual": false,
                    "src": source, "title": "DummyBook"},
                "output": {"epub": {"curly-quotes": true}}},
            "destination": destination
        })
    }
}
