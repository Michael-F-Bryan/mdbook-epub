use crate::Error;
use crate::resources::asset::{Asset, AssetKind};
use infer::Infer;
use mime_guess::Mime;
#[cfg(test)]
use mockall::automock;
use std::fmt::{Display, Formatter};
use std::io::Cursor;
use std::path::PathBuf;
use std::str::FromStr;
use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read},
    path::Path,
};

/// Struct to keep file (image) data 'mime type' after recognizing downloaded content
#[allow(dead_code)]
pub struct RetrievedContent {
    /// Data content itself
    pub reader: Box<dyn Read + Send + Sync + 'static>,
    /// Mime type as string
    pub mime_type: String,
    /// File extension
    pub extension: String,
    /// Additional field to store the content's size in bytes
    pub size: Option<u64>,
}

impl RetrievedContent {
    #[allow(dead_code)]
    pub fn new(
        reader: Box<dyn Read + Send + Sync + 'static>,
        mime_type: String,
        extension: String,
        size: Option<u64>,
    ) -> Self {
        Self {
            reader,
            mime_type,
            extension,
            size,
        }
    }
}

impl Display for RetrievedContent {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let size_info = match self.size {
            Some(size) => format!("{} bytes", size),
            None => "unknown size".to_string(),
        };
        write!(
            f,
            "RetrievedContent {{ mime_type: {}, extension: {}, size: {} }}",
            self.mime_type, self.extension, size_info
        )
    }
}

/// Struct will be used for later updating Asset fields
#[derive(Debug)]
pub(crate) struct UpdatedAssetData {
    pub(crate) mimetype: Mime,
    pub(crate) location_on_disk: PathBuf,
    pub(crate) filename: PathBuf,
}

impl Default for UpdatedAssetData {
    fn default() -> Self {
        UpdatedAssetData {
            mimetype: Mime::from_str("plain/txt").unwrap(),
            location_on_disk: PathBuf::new(),
            filename: PathBuf::new(),
        }
    }
}

/// Trait will be implemented by component to do:
/// - download remote resource bytes content
/// - recognize downloaded content mime type
/// - reading data from local file
#[cfg_attr(test, automock)]
pub(crate) trait ContentRetriever {
    fn download(&self, asset: &Asset) -> Result<UpdatedAssetData, Error>;
    fn read(&self, path: &Path, buffer: &mut Vec<u8>) -> Result<(), Error> {
        File::open(path)?.read_to_end(buffer)?;
        Ok(())
    }
    fn retrieve(&self, url: &str) -> Result<RetrievedContent, Error>;
}

#[derive(Clone, Debug)]
pub(crate) struct ResourceHandler;
impl ContentRetriever for ResourceHandler {
    fn download(&self, asset: &Asset) -> Result<UpdatedAssetData, Error> {
        debug!(
            "ContentRetriever is going to download asset to dest location = '{:?}'",
            asset.location_on_disk
        );
        if let AssetKind::Remote(url) = &asset.source {
            let dest = &asset.location_on_disk;
            debug!("Initial asset dest location = '{:?}'", dest);
            if dest.is_file() {
                debug!("Cache file {:?} to '{}' already exists.", dest, url);
                return Ok(UpdatedAssetData {
                    mimetype: asset.mimetype.clone(),
                    location_on_disk: asset.location_on_disk.clone(),
                    filename: asset.filename.clone(),
                });
            } else {
                if let Some(cache_dir) = dest.parent() {
                    fs::create_dir_all(cache_dir)?;
                }
                debug!("Downloading asset by: {}", url);
                let mut retrieved_content = self.retrieve(url.as_str())?;
                debug!("Retrieved content: \n{}", &retrieved_content);
                let mimetype = Mime::from_str(retrieved_content.mime_type.as_str())?;
                debug!("Mime from content: \n{:?}", &mimetype);

                let mut new_filename = asset.filename.clone();
                let mut new_location_on_disk = asset.location_on_disk.clone();
                if new_filename.extension().is_none() {
                    new_filename = PathBuf::from(format!(
                        "{}.{}",
                        new_filename.as_os_str().to_str().unwrap(),
                        retrieved_content.extension
                    ));
                    new_location_on_disk = PathBuf::from(format!(
                        "{}.{}",
                        new_location_on_disk.as_os_str().to_str().unwrap(),
                        retrieved_content.extension
                    ));
                    debug!("asset file location: '{:?}'", &new_location_on_disk);
                }

                let mut file = OpenOptions::new()
                    .create(true)
                    .truncate(true)
                    .write(true)
                    .open(&new_location_on_disk)?;
                debug!("File on disk: \n{:?}", &file);
                io::copy(&mut retrieved_content.reader, &mut file)?;
                debug!(
                    "Downloaded asset by '{}' : {:?}",
                    url, &new_location_on_disk
                );

                return Ok(UpdatedAssetData {
                    mimetype,
                    location_on_disk: new_location_on_disk,
                    filename: new_filename,
                });
            }
        }
        Ok(UpdatedAssetData {
            mimetype: asset.mimetype.clone(),
            location_on_disk: asset.location_on_disk.clone(),
            filename: asset.filename.clone(),
        })
    }

    fn retrieve(&self, url: &str) -> Result<RetrievedContent, Error> {
        let res = ureq::get(url).call()?;
        match res.status().as_u16() {
            200 => {
                let mut bytes: Vec<u8> = Vec::with_capacity(1000);
                let (_, body) = res.into_parts();
                let _ = body.into_reader().read_to_end(&mut bytes);

                let infer = Infer::new();
                let kind = infer.get(&bytes).ok_or_else(|| {
                    Error::AssetFileNotFound(format!(
                        "Could not determine mime-type for resource: {url}"
                    ))
                })?;

                let mime_type = kind.mime_type().to_string();
                let extension = kind.extension().to_string();

                debug!(
                    "Detected MIME type: {}, Extension: {} for URL: {}",
                    mime_type, extension, url
                );

                let content_len = bytes.len() as u64;
                // Cursor owns bytes data and implements Read
                let reader: Box<dyn Read + Send + Sync + 'static> = Box::new(Cursor::new(bytes));

                Ok(RetrievedContent {
                    reader,
                    mime_type,
                    extension,
                    size: Some(content_len),
                })
            }
            404 => Err(Error::AssetFileNotFound(format!(
                "Missing remote resource: {url}"
            ))),
            _ => unreachable!("Unexpected response status for '{url}'"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use tempfile::TempDir;

    use crate::errors::Error;
    use crate::resources::asset::Asset;

    use super::{ContentRetriever, RetrievedContent, UpdatedAssetData};

    #[test]
    fn test_download_success() {
        use std::io::Read;

        struct TestHandler;
        impl ContentRetriever for TestHandler {
            fn download(&self, asset: &Asset) -> Result<UpdatedAssetData, Error> {
                Ok(UpdatedAssetData::default())
            }
            fn retrieve(&self, _url: &str) -> Result<RetrievedContent, Error> {
                let content = "Downloaded content".as_bytes();
                Ok(RetrievedContent::new(
                    Box::new(Cursor::new(content)),
                    "text/plain".to_string(),
                    "txt".to_string(),
                    Some(content.len() as u64),
                ))
            }
        }
        let cr = TestHandler {};
        let mut a = temp_remote_asset("https://mdbook-epub.org/image.svg").unwrap();
        let r = cr.download(&mut a);

        assert!(r.is_ok());
        let mut buffer = String::new();
        let mut f = std::fs::File::open(&a.location_on_disk).unwrap();
        f.read_to_string(&mut buffer).unwrap();
        assert_eq!(buffer, "Downloaded content");
    }

    #[test]
    fn test_download_fail_when_resource_not_exist() {
        struct TestHandler;
        impl ContentRetriever for TestHandler {
            fn download(&self, asset: &Asset) -> Result<UpdatedAssetData, Error> {
                Ok(UpdatedAssetData::default())
            }
            fn retrieve(&self, url: &str) -> Result<RetrievedContent, Error> {
                Err(Error::AssetFileNotFound(format!(
                    "Missing remote resource: {url}"
                )))
            }
        }
        let cr = TestHandler {};
        let mut a = temp_remote_asset("https://mdbook-epub.org/not-exist.svg").unwrap();
        let r = cr.download(&mut a);

        assert!(r.is_err());
        assert!(matches!(r.unwrap_err(), Error::AssetFileNotFound(_)));
    }

    #[test]
    #[should_panic(expected = "NOT 200 or 404")]
    fn test_download_fail_with_unexpected_status() {
        struct TestHandler;
        impl ContentRetriever for TestHandler {
            fn download(&self, asset: &Asset) -> Result<UpdatedAssetData, Error> {
                Ok(UpdatedAssetData::default())
            }
            fn retrieve(&self, _url: &str) -> Result<RetrievedContent, Error> {
                panic!("NOT 200 or 404")
            }
        }
        let cr = TestHandler {};
        let mut a = temp_remote_asset("https://mdbook-epub.org/bad.svg").unwrap();
        let r = cr.download(&mut a);

        panic!("{}", r.unwrap_err().to_string());
    }

    #[test]
    fn test_download_parametrized_avatar_image() {
        use std::io::Read;

        struct TestHandler;
        impl ContentRetriever for TestHandler {
            fn download(&self, asset: &Asset) -> Result<UpdatedAssetData, Error> {
                Ok(UpdatedAssetData::default())
            }
            fn retrieve(&self, _url: &str) -> Result<RetrievedContent, Error> {
                let content = "Downloaded content".as_bytes();
                Ok(RetrievedContent::new(
                    Box::new(Cursor::new(content)),
                    "text/plain".to_string(),
                    "txt".to_string(),
                    Some(content.len() as u64),
                ))
            }
        }
        let cr = TestHandler {};
        let mut a =
            temp_remote_asset("https://avatars.githubusercontent.com/u/274803?v=4").unwrap();
        let r = cr.download(&mut a);
        assert!(r.is_ok());

        let mut buffer = String::new();
        let mut f = std::fs::File::open(&a.location_on_disk).unwrap();
        f.read_to_string(&mut buffer).unwrap();
        assert_eq!(buffer, "Downloaded content");
    }

    fn temp_remote_asset(url: &str) -> Result<Asset, Error> {
        let tmp_dir = TempDir::new().unwrap();
        let dest_dir = tmp_dir.path().join("mdbook-epub");
        Asset::from_url(
            "unique_remote_url",
            url::Url::parse(url).unwrap(),
            dest_dir.as_path(),
        )
    }
}
