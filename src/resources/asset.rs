use std::path::{MAIN_SEPARATOR_STR, Path, PathBuf};
use url::Url;
use mime_guess::Mime;
use crate::errors::Error;
use crate::resources::resources::{UPPER_FOLDER_PATHS, UPPER_PARENT, UPPER_PARENT_LINUX, UPPER_PARENT_STARTS_SLASH, UPPER_PARENT_STARTS_SLASH_LINUX};
use crate::utils;

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
    pub(crate) fn from_url(url: Url, dest_dir: &Path) -> Result<Asset, Error> {
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
    pub(crate) fn from_local(link: &str, src_dir: &Path, chapter_path: &Path) -> Result<Asset, Error> {
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
        let binding = utils::normalize_path(Path::new(link));
        debug!("Extracting file name from = {:?}, binding = '{binding:?}'", &full_filename.display());
        let filename = if cfg!(target_os = "windows") {
            binding.as_os_str().to_os_string()
                .into_string().expect("Error getting filename for Local Asset").replace('\\', "/")
        } else {
            String::from(binding.as_path().to_str().unwrap())
        };

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
    pub(crate) fn compute_asset_path_by_src_and_link(link: &str, chapter_dir: &PathBuf) -> PathBuf {
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
    pub(crate) fn remove_prefixes(link_to_strip: String, prefixes: &[&str]) -> String {
        let mut stripped_link = link_to_strip.clone();
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
