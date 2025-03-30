use crate::resources::asset::{Asset, AssetKind};
use crate::resources::retrieve::ContentRetriever;
use crate::utils::encode_non_ascii_symbols;
use html_parser::{Dom, Node};
use pulldown_cmark::{CowStr, Event, Tag};
use std::collections::HashMap;
use std::ffi::OsString;
use std::iter;
use std::path::Path;
use url::Url;

/// Filter is used for replacing remote urls with local images downloaded from internet
pub struct AssetRemoteLinkFilter<'a> {
    // Keeps pairs: 'remote url' | 'asset'
    assets: &'a mut HashMap<String, Asset>,
    depth: usize,
    download_handler: &'a dyn ContentRetriever,
}

impl<'a> AssetRemoteLinkFilter<'a> {
    pub(crate) fn new(
        assets: &'a mut HashMap<String, Asset>,
        depth: usize,
        handler: &'a dyn ContentRetriever,
    ) -> Self {
        Self {
            assets,
            depth,
            download_handler: handler,
        }
    }

    /// Do processing of chapter's content and replace 'remote link' by 'local file name'
    pub(crate) fn apply(&mut self, event: Event<'a>) -> Event<'a> {
        debug!("AssetLinkFilter: Processing Event = {:?}", &event);
        match event {
            Event::Start(Tag::Image {
                link_type,
                ref dest_url,
                ref title,
                ref id,
            }) => {
                let encoded_link_key = encode_non_ascii_symbols(&dest_url);
                if let Some(mut asset) = self.assets.get_mut(&encoded_link_key).cloned() {
                    debug!("filter loops over all assets: by {}", &dest_url);
                    if let AssetKind::Remote(ref _remote_url) = asset.source {
                        debug!("Compare: {} vs {}", &asset.original_link, &dest_url);
                        // Проверяем, совпадает ли remote_url с dest_url
                        if asset.original_link.as_str() == dest_url.as_ref() {
                            debug!("1. Found URL '{}' by Event", &dest_url);
                            match self.download_handler.download(&mut asset) {
                                Ok(_) => {
                                    println!(
                                        "SUCCESSFULLY downloaded resource by URL '{}'",
                                        &dest_url
                                    );
                                    let new = asset.filename.to_string_lossy().to_string();
                                    debug!(
                                        "Created new Event for URL '{}' and new Url = {}",
                                        &dest_url, &new
                                    );
                                    return Event::Start(Tag::Image {
                                        link_type,
                                        dest_url: CowStr::from(new),
                                        title: title.to_owned(),
                                        id: id.to_owned(),
                                    });
                                }
                                Err(error) => {
                                    error!(
                                        "Can't download resource by URL '{}' for chapter '{}'. Error = {}",
                                        &dest_url, &title, error
                                    );
                                }
                            }
                        }
                    }
                }
                event
            }
            Event::Html(ref html) => {
                let mut found_links = Vec::new();
                if let Ok(dom) = Dom::parse(&html.clone().into_string()) {
                    for item in dom.children {
                        match item {
                            Node::Element(ref element) if element.name == "img" => {
                                if let Some(dest) = &element.attributes["src"] {
                                    if Url::parse(dest).is_ok() {
                                        debug!("Found a valid remote img src:\"{}\".", dest);
                                        let encoded_link_key = encode_non_ascii_symbols(&dest);

                                        // TODO: processing here....

                                        found_links.push(dest.to_owned());
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                if found_links.is_empty() {
                    event
                } else {
                    found_links.dedup();
                    let mut content = html.clone().into_string();
                    debug!("2. found_links\n'{:?}'", &found_links);
                    for original_link in found_links {
                        let encoded_link_key = encode_non_ascii_symbols(&original_link);
                        debug!("original_link = '{}'", &original_link);
                        debug!("1. assets\n'{:?}'", &self.assets);

                        if let Some(asset) = self.assets.get(&encoded_link_key) {
                            // let new = self.path_prefix(asset.filename.as_path());
                            let depth = self.depth;
                            let new = Self::compute_path_prefix(depth, asset.filename.as_path());

                            debug!("old content before replacement\n{}", &content);
                            debug!(
                                "{:?}, link '{}' is replaced by '{}'",
                                asset, &original_link, &new
                            );
                            // REAL SRC REPLACING happens here...
                            content = content.replace(&original_link, new.as_str());
                            trace!("new content after replacement\n{}", &content);
                        } else {
                            error!("Asset was not found by original_link: {}", original_link);
                            unreachable!("{original_link} should be replaced, but it doesn't.");
                        }
                    }
                    Event::Html(CowStr::from(content))
                }
            }
            _ => event,
        }
    }

    fn compute_path_prefix(depth: usize, path: &Path) -> String {
        let mut fsp = OsString::new();
        for (i, component) in path.components().enumerate() {
            if i > 0 {
                fsp.push("/");
            }
            fsp.push(component);
        }
        let filename = fsp
            .into_string()
            .unwrap_or_else(|orig| orig.to_string_lossy().to_string());
        (0..depth)
            .map(|_| "..")
            .chain(iter::once(filename.as_str()))
            .collect::<Vec<_>>()
            .join("/")
    }
}
