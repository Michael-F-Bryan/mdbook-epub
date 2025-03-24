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
    assets: &'a HashMap<String, Asset>,
    depth: usize,
}

impl<'a> AssetRemoteLinkFilter<'a> {
    pub(crate) fn new(assets: &'a HashMap<String, Asset>, depth: usize) -> Self {
        Self { assets, depth }
    }

    /// Do processing of chapter's content and replace 'remote link' by 'local file name'
    pub(crate) fn apply(&self, event: Event<'a>) -> Event<'a> {
        // trace!("AssetLinkFilter: Processing Event = {:?}", &event);
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
        let filename = fsp
            .into_string()
            .unwrap_or_else(|orig| orig.to_string_lossy().to_string());
        (0..self.depth)
            .map(|_| "..")
            .chain(iter::once(filename.as_str()))
            .collect::<Vec<_>>()
            .join("/")
    }
}
