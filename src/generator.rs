use std::{
    collections::HashMap,
    ffi::OsString,
    fmt::{self, Debug, Formatter},
    fs::File,
    io::{Read, Write},
    iter,
    path::{Path, PathBuf},
};

use epub_builder::{EpubBuilder, EpubContent, EpubVersion, ZipLibrary};
use handlebars::{Handlebars, RenderError, RenderErrorReason};
use html_parser::{Dom, Node};
use mdbook::book::{BookItem, Chapter};
use mdbook::renderer::RenderContext;
use pulldown_cmark::{html, CowStr, Event, Tag, TagEnd};
use url::Url;

use crate::config::Config;
use crate::resources::asset::{Asset, AssetKind};
use crate::resources::resource::{self};
use crate::resources::retrieve::{ContentRetriever, ResourceHandler};
use crate::utils::encode_non_ascii_symbols;
use crate::DEFAULT_CSS;
use crate::{utils, Error};

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
        let config = Config::from_render_context(ctx)?;

        let epub_version = match config.epub_version {
            Some(2) => Some(EpubVersion::V20),
            Some(3) => Some(EpubVersion::V30),
            Some(v) => {
                return Err(Error::EpubDocCreate(format!(
                    "Unsupported epub version specified in book.toml: {}",
                    v
                )))
            }
            None => None,
        };

        let mut builder = EpubBuilder::new(ZipLibrary::new()?)?;
        if let Some(version) = epub_version {
            builder.epub_version(version);
        }

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
        info!("1. populate metadata ==");
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
        info!("8. final generation ==");
        self.builder.generate(writer)?;
        info!("Generating the EPUB book - DONE !");
        Ok(())
    }

    /// Find assets for adding to the document later. For remote linked assets, they would be
    /// rendered differently in the document by provided information of assets.
    fn find_assets(&mut self) -> Result<(), Error> {
        info!("2. find assets ==");
        let error = String::from("Failed finding/fetch resource taken from content? Look up content for possible error...");
        // resources::find can emit very unclear error based on internal MD content,
        // so let's give a tip to user in error message
        let assets = resource::find(self.ctx).map_err(|e| {
            error!("{} Caused by: {}", error, e);
            e
        })?;
        self.assets.extend(assets);
        Ok(())
    }

    fn generate_chapters(&mut self) -> Result<(), Error> {
        info!("3. Generate chapters == ");

        for (idx, item) in self.ctx.book.sections.iter().enumerate() {
            let is_first = idx == 0;
            if let BookItem::Chapter(ref ch) = *item {
                trace!("Adding chapter \"{}\"", ch);
                self.add_chapter(ch, Some(is_first))?;
            }
        }

        Ok(())
    }

    fn add_chapter(&mut self, ch: &Chapter, is_first: Option<bool>) -> Result<(), Error> {
        info!("Adding chapter = '{}'", &ch.name);
        let rendered_result = self.render_chapter(ch);
        // let's skip chapter without content (drafts)
        let rendered = match rendered_result {
            Ok(rendered_content) => rendered_content,
            Err(error_msg) => {
                warn!(
                    "SKIPPED chapter '{}' due to error = {}",
                    &ch.name, error_msg
                );
                return Ok(());
            }
        };

        let content_path = ch.path.as_ref().ok_or_else(|| {
            Error::ContentFileNotFound(format!(
                "Content file was not found for Chapter '{}'",
                ch.name
            ))
        })?;
        trace!(
            "add a chapter '{:?}' by a path = '{:?}'",
            &ch.name,
            content_path
        );
        let path = content_path.with_extension("html").display().to_string();
        let title = if self.config.no_section_label {
            ch.name.clone()
        } else if let Some(ref section_number) = ch.number {
            format! {"{} {}", section_number, ch.name}
        } else {
            ch.name.clone()
        };

        // If this is the first chapter, mark its type as Text (i.e. "bodymatter") for render_nav().
        // This ensures at least one item in the nav.xhtml <nav epub:type="landmarks"><ol> list,
        // otherwise epubcheck shows an error.
        let mut content = match is_first {
            Some(true) => {
                EpubContent::new(path, rendered.as_bytes())
                    .title(title)
                    .reftype(epub_builder::ReferenceType::Text)
            },
            _ => {
                EpubContent::new(path, rendered.as_bytes())
                    .title(title)
            },
        };

        let level = ch.number.as_ref().map(|n| n.len() as i32 - 1).unwrap_or(0);
        content = content.level(level);

        self.builder.add_content(content)?;

        // second pass to actually add the sub-chapters
        for sub_item in &ch.sub_items {
            if let BookItem::Chapter(ref sub_ch) = *sub_item {
                trace!("add sub-item = {:?}", sub_ch.name);
                self.add_chapter(sub_ch, None)?;
            }
        }

        Ok(())
    }

    /// Render the chapter into its fully formed HTML representation.
    fn render_chapter(&self, ch: &Chapter) -> Result<String, RenderError> {
        let chapter_dir = if let Some(chapter_file_path) = &ch.path {
            chapter_file_path.parent().ok_or_else(|| {
                RenderError::from(RenderErrorReason::Other(format!(
                    "No CSS found by a path = {:?}",
                    ch.path
                )))
            })?
        } else {
            return Err(RenderError::from(RenderErrorReason::Other(format!(
                "Draft chapter: '{}' could not be rendered.",
                ch.name
            ))));
        };

        let mut body = String::new();

        if self.config.epub_version == Some(3) && self.config.footnote_backrefs {
            body.push_str(&self.render_with_footnote_backrefs(chapter_dir, ch));
        } else {
            let parser = utils::create_new_pull_down_parser(&ch.content);
            let mut quote_converter = EventQuoteConverter::new(self.config.curly_quotes);
            let ch_depth = chapter_dir.components().count();

            // create 'Remote Assets' copy to be processed by AssetLinkFilter
            let mut remote_assets: HashMap<String, Asset> = HashMap::new();
            for (key, value) in self.assets.clone().into_iter() {
                trace!("{} / {:?}", key, &value);
                if let AssetKind::Remote(ref remote_url) = value.source {
                    trace!(
                        "Adding remote_assets = '{}' / {:?}",
                        remote_url.to_string(),
                        &value
                    );
                    remote_assets.insert(remote_url.to_string(), value);
                }
                /* else if let AssetKind::Local(ref _local_path) = value.source {
                let relative_path = value.filename.to_str().unwrap();
                remote_assets.insert(String::from(relative_path), value);
                } */
            }
            let asset_link_filter = AssetLinkFilter::new(&remote_assets, ch_depth);
            let events = parser
                .map(|event| quote_converter.convert(event))
                .map(|event| asset_link_filter.apply(event));
            trace!("Found Rendering events map = [{:?}]", &events);

            html::push_html(&mut body, events);
        }

        trace!("Chapter content after Events processing = [{:?}]", body);

        let stylesheet_path = chapter_dir
            .components()
            .map(|_| "..")
            .chain(iter::once("stylesheet.css"))
            .collect::<Vec<_>>()
            .join("/");

        let epub_version_3 = self.config.epub_version == Some(3);

        let ctx = json!({
            "epub_version_3": epub_version_3,
            "title": ch.name,
            "body": body,
            "stylesheet": stylesheet_path
        });

        self.hbs.render("index", &ctx)
    }

    fn footnote_reference_event<'b>(&self,
                                    event: Event<'b>,
                                    in_footnote: & mut Vec<Vec<Event<'b>>>,
                                    footnotes: & mut Vec<Vec<Event<'b>>>,
                                    footnote_numbers: &mut HashMap<CowStr<'b>, (usize, usize)>)
                                    -> Option<Event<'b>> {
            match event {
                Event::Start(Tag::FootnoteDefinition(_)) => {
                    in_footnote.push(vec![event]);
                    None
                }
                Event::End(TagEnd::FootnoteDefinition) => {
                    let mut f = in_footnote.pop().unwrap();
                    f.push(event);
                    footnotes.push(f);
                    None
                }
                Event::FootnoteReference(name) => {
                    let n = footnote_numbers.len() + 1;
                    let (n, nr) = footnote_numbers.entry(name.clone()).or_insert((n, 0usize));
                    *nr += 1;
                    // The [] brackets slightly increase the linked area and help to tap on the footnote reference.
                    let html = Event::Html(format!(r##"<sup class="footnote-reference" id="fr-{name}-{nr}"><a href="#fn-{name}">[{n}]</a></sup>"##).into());
                    if in_footnote.is_empty() {
                        Some(html)
                    } else {
                        in_footnote.last_mut().unwrap().push(html);
                        None
                    }
                }
                _ if !in_footnote.is_empty() => {
                    in_footnote.last_mut().unwrap().push(event);
                    None
                }
                _ => Some(event),
            }
    }

    fn render_with_footnote_backrefs(&self, chapter_dir: &Path, ch: &Chapter) -> String {
        let mut body = String::new();

        // Based on
        // https://github.com/pulldown-cmark/pulldown-cmark/blob/master/pulldown-cmark/examples/footnote-rewrite.rs

        // To generate this style, you have to collect the footnotes at the end, while parsing.
        // You also need to count usages.
        let mut footnotes = Vec::new();
        let mut in_footnote = Vec::new();
        let mut footnote_numbers = HashMap::new();

        let parser = utils::create_new_pull_down_parser(&ch.content)
            .filter_map(|event| self.footnote_reference_event(event,
                                                              &mut in_footnote,
                                                              &mut footnotes,
                                                              &mut footnote_numbers));

        let mut quote_converter = EventQuoteConverter::new(self.config.curly_quotes);
        let ch_depth = chapter_dir.components().count();

        // create 'Remote Assets' copy to be processed by AssetLinkFilter
        let mut remote_assets: HashMap<String, Asset> = HashMap::new();
        for (key, value) in self.assets.clone().into_iter() {
            trace!("{} / {:?}", key, &value);
            if let AssetKind::Remote(ref remote_url) = value.source {
                trace!(
                    "Adding remote_assets = '{}' / {:?}",
                    remote_url.to_string(),
                    &value
                );
                remote_assets.insert(remote_url.to_string(), value);
            }
            /* else if let AssetKind::Local(ref _local_path) = value.source {
            let relative_path = value.filename.to_str().unwrap();
            remote_assets.insert(String::from(relative_path), value);
        }*/
        }
        let asset_link_filter = AssetLinkFilter::new(&remote_assets, ch_depth);
        let events = parser
            .map(|event| quote_converter.convert(event))
            .map(|event| asset_link_filter.apply(event));
        trace!("Found Rendering events map = [{:?}]", &events);

        html::push_html(&mut body, events);

        // To make the footnotes look right, we need to sort them by their appearance order, not by
        // the in-tree order of their actual definitions. Unused items are omitted entirely.
        if !footnotes.is_empty() {
            footnotes.retain(|f| match f.first() {
                Some(Event::Start(Tag::FootnoteDefinition(name))) => {
                    footnote_numbers.get(name).unwrap_or(&(0, 0)).1 != 0
                }
                _ => false,
            });
            footnotes.sort_by_cached_key(|f| match f.first() {
                Some(Event::Start(Tag::FootnoteDefinition(name))) => {
                    footnote_numbers.get(name).unwrap_or(&(0, 0)).0
                }
                _ => unreachable!(),
            });

            body.push_str("<div class=\"footnotes\" epub:type=\"footnotes\">\n");

            html::push_html(
                &mut body,
                footnotes.into_iter().flat_map(|fl| {
                    // To write backrefs, the name needs kept until the end of the footnote definition.
                    let mut name = CowStr::from("");

                    let mut has_written_backrefs = false;
                    let fl_len = fl.len();
                    let footnote_numbers = &footnote_numbers;
                    let mut written_footnote_numbers: Vec<usize> = Vec::new();
                    fl.into_iter().enumerate().map(move |(i, f)| match f {
                        Event::Start(Tag::Paragraph) => {
                            let fn_number = footnote_numbers.get(&name).unwrap().0;
                            if written_footnote_numbers.contains(&fn_number) {
                                Event::Html("<p>".into())
                            } else {
                                // At this point we have started rendering a Tag::FootnoteDefinition, so already wrote an
                                // opening <div> tag, and starting to write the paragraphs of the definition.
                                //
                                // If we haven't written this footnote reference number yet, then write it at the beginning of
                                // the paragraph in a <span>.
                                //
                                // This will include the footnote number in the <div>, but NOT as a block element, and
                                // hence it correcly shows up in footnote pop-ups.
                                //
                                // Use a <div> instead of an <aside> tag, because iBooks doesn't display <aside>.
                                //
                                // Tested on: ReadEra and Moon+ Reader on Android, Kindle Paperwhite, iBooks, KOReader on ReMarkable 2.
                                written_footnote_numbers.push(fn_number);
                                Event::Html(format!(r##"<p><span class="footnote-definition-label">[{fn_number}]</span> "##).into())
                            }
                        }
                        Event::Start(Tag::FootnoteDefinition(current_name)) => {
                            name = current_name;
                            has_written_backrefs = false;
                            Event::Html(format!(r##"<div class="footnote-definition" id="fn-{name}" epub:type="footnote">"##).into())
                        }
                        Event::End(TagEnd::FootnoteDefinition) | Event::End(TagEnd::Paragraph)
                            if !has_written_backrefs && i >= fl_len - 2 =>
                        {
                            let usage_count = footnote_numbers.get(&name).unwrap().1;
                            let mut end = String::with_capacity(
                                name.len() + (r##" <a href="#fr--1">↩</a></div>"##.len() * usage_count),
                            );
                            for usage in 1..=usage_count {
                                if usage == 1 {
                                    end.push_str(&format!(r##" <a href="#fr-{name}-{usage}">↩</a>"##));
                                } else {
                                    end.push_str(&format!(r##" <a href="#fr-{name}-{usage}">↩{usage}</a>"##));
                                }
                            }
                            has_written_backrefs = true;
                            if f == Event::End(TagEnd::FootnoteDefinition) {
                                end.push_str("</div>\n");
                            } else {
                                end.push_str("</p>\n");
                            }
                            Event::Html(end.into())
                        }
                        Event::End(TagEnd::FootnoteDefinition) => Event::Html("</div>\n".into()),
                        Event::FootnoteReference(_) => unreachable!("converted to HTML earlier"),
                        f => f,
                    })
                }),
            );

            // Closing div.footnotes
            body.push_str("</div>\n");
        }

        body
    }

    /// Generate the stylesheet and add it to the document.
    fn embed_stylesheets(&mut self) -> Result<(), Error> {
        info!("5. Embedding stylesheets ==");

        let stylesheet = self.generate_stylesheet()?;
        self.builder.stylesheet(stylesheet.as_slice())?;

        Ok(())
    }

    fn additional_assets(&mut self) -> Result<(), Error> {
        info!(
            "6. Embedding, downloading additional assets == [{:?}]",
            self.assets.len()
        );

        // TODO: have a list of Asset URLs and try to download all of them (in parallel?)
        // to a temporary location.
        let mut count = 0;
        for asset in self.assets.values() {
            self.handler.download(asset)?;
            debug!("Adding asset : {:?}", asset);
            let mut content = Vec::new();
            self.handler
                .read(&asset.location_on_disk, &mut content)
                .map_err(|_| Error::AssetOpen)?;
            let mt = asset.mimetype.to_string();
            self.builder.add_resource(&asset.filename, &*content, mt)?;
            count += 1;
        }
        debug!("Embedded '{}' additional assets", count);
        Ok(())
    }

    fn additional_resources(&mut self) -> Result<(), Error> {
        info!("7. Embedding additional resources ==");

        let mut count = 0;
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
                        "Failed to find resource file by 'root + src + path' = {full_path_composed:?}"
                    );
                    warn!("{:?}", error);
                    debug!("Failed to find resource, trying to compose by 'root + path' only...");
                    let full_path_composed = self.ctx.root.join(path);
                    error = format!(
                        "Failed to find resource file by a root + path = {full_path_composed:?}"
                    );
                    full_path = full_path_composed.canonicalize().expect(&error);
                }
            }
            let mt = mime_guess::from_path(&full_path).first_or_octet_stream();

            let content = File::open(&full_path).map_err(|_| Error::AssetOpen)?;
            debug!(
                "Adding resource [{}]: {:?} / {:?} ",
                count,
                path,
                mt.to_string()
            );
            self.builder.add_resource(path, content, mt.to_string())?;
            count += 1;
        }
        debug!("Embedded '{}' additional resources", count);
        Ok(())
    }

    fn add_cover_image(&mut self) -> Result<(), Error> {
        info!("4. Adding cover image ==");

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
                    "Failed to find cover image by full path-name = {full_path_composed:?}"
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
                let error =
                    format!("Failed to find stylesheet by full path-name = {full_path_composed:?}");
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

impl Debug for Generator<'_> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Generator")
            .field("ctx", &self.ctx)
            .field("builder", &self.builder)
            .field("config", &self.config)
            .field("assets", &self.assets.keys())
            .finish()
    }
}

/// Filter is used for replacing remote urls with local images downloaded from internet
struct AssetLinkFilter<'a> {
    // Keeps pairs: 'remote url' | 'asset'
    assets: &'a HashMap<String, Asset>,
    depth: usize,
}

impl<'a> AssetLinkFilter<'a> {
    fn new(assets: &'a HashMap<String, Asset>, depth: usize) -> Self {
        Self { assets, depth }
    }

    /// Do processing of chapter's content and replace 'remote link' by 'local file name'
    fn apply(&self, event: Event<'a>) -> Event<'a> {
        trace!("AssetLinkFilter: Processing Event = {:?}", &event);
        match event {
            Event::Start(Tag::Image {
                link_type,
                ref dest_url,
                ref title,
                ref id,
            }) => {
                if let Some(asset) = self.assets.get(&dest_url.to_string()) {
                    // PREPARE info for replacing original REMOTE link by `<hash>.ext` value inside chapter content
                    debug!("Found URL '{}' by Event", &dest_url);
                    let new = self.path_prefix(asset.filename.as_path());
                    Event::Start(Tag::Image {
                        link_type,
                        dest_url: CowStr::from(new),
                        title: title.to_owned(),
                        id: id.to_owned(),
                    })
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
                        let encoded_link_key = encode_non_ascii_symbols(&link);
                        debug!("encoded_link_key = '{}'", &encoded_link_key);

                        if let Some(asset) = self.assets.get(&encoded_link_key) {
                            let new = self.path_prefix(asset.filename.as_path());
                            trace!("old content before replacement\n{}", &content);
                            trace!("{:?}, link '{}' is replaced by '{}'", asset, &link, &new);
                            // REAL SRC REPLACING happens here...
                            content = content.replace(&link, new.as_str());
                            trace!("new content after replacement\n{}", &content);
                        } else {
                            error!(
                                "Asset was not found by encoded_link key: {}",
                                encoded_link_key
                            );
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
        // compatible to Windows, translate to forward slash in file path.
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
            Event::End(TagEnd::CodeBlock) => {
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
    use tempfile::TempDir;

    use super::*;
    use crate::resources::asset::AssetKind;
    use crate::resources::retrieve::MockContentRetriever;

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
        let tmp_dir = TempDir::new().unwrap();
        let destination = tmp_dir.path().join("mdbook-epub");
        let json = ctx_with_template(&content, "src", destination.as_path()).to_string();
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
        let hashed_filename = utils::hash_link(&url.parse::<Url>().unwrap());
        let should_be_url = destination.as_path().join(hashed_filename);
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
        let links = [
            "local.webp",
            "http://server/remote.svg",
            "http://server/link.png",
        ];
        let tmp_dir = TempDir::new().unwrap();
        let root = tmp_dir.path().join("mdbook-epub");
        let mut assets = HashMap::new();
        assets.insert(
            links[0].to_string(),
            Asset {
                location_on_disk: root.as_path().join("src").join(links[0]),
                filename: PathBuf::from(links[0]),
                mimetype: "image/webp".parse::<mime::Mime>().unwrap(),
                source: AssetKind::Local(PathBuf::from(links[0])),
            },
        );
        let url = Url::parse(links[1]).unwrap();
        let hashed_filename = utils::hash_link(&url);
        let hashed_path = Path::new("cache").join(&hashed_filename);
        assets.insert(
            links[1].to_string(),
            Asset {
                location_on_disk: root.as_path().join("book").join(&hashed_path),
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
        let parser = utils::create_new_pull_down_parser(&markdown_str);
        let events = parser.map(|ev| filter.apply(ev));
        trace!("Events = {:?}", events);
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
        let tmp_dir = TempDir::new().unwrap();
        let dest_dir = tmp_dir.path().join("mdbook-epub");
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
        let mut json = ctx_with_template("", "src", dest_dir.as_path());
        let chvalue = json["book"]["sections"].as_array_mut().unwrap();
        chvalue.clear();
        chvalue.push(ch1);
        chvalue.push(ch2);

        let ctx = RenderContext::from_json(json.to_string().as_bytes()).unwrap();
        let mut g = Generator::new(&ctx).unwrap();
        g.find_assets().unwrap();
        assert_eq!(g.assets.len(), 1);

        let pat = |heading, prefix| {
            format!("<h1>{heading}</h1>\n<p><img src=\"{prefix}811c431d49ec880b.svg\"")
        };
        if let BookItem::Chapter(ref ch) = ctx.book.sections[0] {
            let rendered: String = g.render_chapter(ch).unwrap();
            debug!("{}", &rendered);
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
        let tmp_dir = TempDir::new().unwrap();
        let json = ctx_with_template(
            "# Chapter 1\n\n",
            "nosuchsrc",
            tmp_dir.path().join("mdbook-epub").as_path(),
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
