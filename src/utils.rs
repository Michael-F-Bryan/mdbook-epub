use pulldown_cmark::{Options, Parser};
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};
use url::Url;
use uuid::Uuid;

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

/// Generate file name + extension from supplied remote URL.
/// If url does not contain file extension because of 'parametrized url'
/// then file's extension is generated as UUID4 value and file name
/// is hashed from URL itself.
pub(crate) fn hash_link(url: &Url) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let path = PathBuf::from(url.path());
    let _generated_file_extension = Uuid::new_v4().to_string();
    let ext = path
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or_else(|| _generated_file_extension.as_str());
    format!("{:x}.{}", hasher.finish(), ext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_named_url_with_extention() {
        let test_url = "https://www.rust-lang.org/static/images/rust-logo-blk.svg";
        let hashed_filename = hash_link(&test_url.parse::<Url>().unwrap());
        assert_eq!("b20b2723e874918.svg", hashed_filename);
    }

    #[test]
    fn test_hash_parametrized_url_no_extension() {
        let test_avatar_url = "https://avatars.githubusercontent.com/u/274803?v=4";
        let hashed_filename = hash_link(&test_avatar_url.parse::<Url>().unwrap());
        assert!(hashed_filename.starts_with("4dbdb25800b6fa1b."));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_normalize_path() {
        let link = "./asset1.jpg";
        let link_as_path = normalize_path(Path::new(link));
        assert_eq!(
            link_as_path.as_path().to_str().unwrap(),
            "asset1.jpg"
        );

        let link = "../asset1.jpg";
        let link_as_path = normalize_path(Path::new(link));
        assert_eq!(
            link_as_path.as_path().to_str().unwrap(),
            "asset1.jpg"
        );

        let link = "../upper/assets/asset1.jpg";
        let link_as_path = normalize_path(Path::new(link));
        assert_eq!(
            link_as_path.as_path().to_str().unwrap(),
            "upper/assets/asset1.jpg"
        );

        let link = "assets/asset1.jpg";
        let link_as_path = normalize_path(Path::new(link));
        assert_eq!(
            link_as_path.as_path().to_str().unwrap(),
            "assets/asset1.jpg"
        );

        let link = "./assets/asset1.jpg";
        let link_as_path = normalize_path(Path::new(link));
        assert_eq!(
            link_as_path.as_path().to_str().unwrap(),
            "assets/asset1.jpg"
        );

        let link = "../assets/asset1.jpg";
        let link_as_path = normalize_path(Path::new(link));
        assert_eq!(
            link_as_path.as_path().to_str().unwrap(),
            "assets/asset1.jpg"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_normalize_path_win() {
        let link = ".\\asset1.jpg";
        let link_as_path = normalize_path(Path::new(link));
        assert_eq!(
            link_as_path.as_path().to_str().unwrap(),
            "asset1.jpg"
        );

        let link = "..\\asset1.jpg";
        let link_as_path = normalize_path(Path::new(link));
        assert_eq!(
            link_as_path.as_path().to_str().unwrap(),
            "asset1.jpg"
        );

        let link = "..\\upper\\assets\\asset1.jpg";
        let link_as_path = normalize_path(Path::new(link));
        assert_eq!(
            link_as_path.as_path().to_str().unwrap(),
            "upper\\assets\\asset1.jpg"
        );

        let link = "assets\\asset1.jpg";
        let link_as_path = normalize_path(Path::new(link));
        assert_eq!(
            link_as_path.as_path().to_str().unwrap(),
            "assets\\asset1.jpg"
        );

        let link = ".\\assets\\asset1.jpg";
        let link_as_path = normalize_path(Path::new(link));
        assert_eq!(
            link_as_path.as_path().to_str().unwrap(),
            "assets\\asset1.jpg"
        );

        let link = "..\\assets\\asset1.jpg";
        let link_as_path = normalize_path(Path::new(link));
        assert_eq!(
            link_as_path.as_path().to_str().unwrap(),
            "assets\\asset1.jpg"
        );
    }
}