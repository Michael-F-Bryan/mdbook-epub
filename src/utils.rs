use pulldown_cmark::{Options, Parser};
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};
use url::Url;

pub(crate) fn create_new_pull_down_parser(text: &str) -> Parser<'_, '_> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_FOOTNOTES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    Parser::new_ext(text, opts)
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
