use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read},
    path::Path,
};

#[cfg(test)]
use mockall::automock;

use crate::Error;
use crate::resources::asset::{Asset, AssetKind};

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
                let mut file = OpenOptions::new().create(true).truncate(true).write(true).open(dest)?;
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
    use tempfile::TempDir;

    use crate::errors::Error;
    use crate::resources::asset::Asset;

    use super::ContentRetriever;

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

    #[test]
    fn download_parametrized_avatar_image() {
        use std::io::Read;

        struct TestHandler;
        impl ContentRetriever for TestHandler {
            fn retrieve(&self, _url: &str) -> Result<BoxRead, Error> {
                Ok(Box::new("Downloaded content".as_bytes()))
            }
        }
        let cr = TestHandler {};
        let a = temp_remote_asset("https://avatars.githubusercontent.com/u/274803?v=4").unwrap();
        let r = cr.download(&a);
        assert!(r.is_ok());

        let mut buffer = String::new();
        let mut f = std::fs::File::open(&a.location_on_disk).unwrap();
        f.read_to_string(&mut buffer).unwrap();
        assert_eq!(buffer, "Downloaded content");
    }

    fn temp_remote_asset(url: &str) -> Result<Asset, Error> {
        let tmp_dir = TempDir::new().unwrap();
        let dest_dir = tmp_dir.path().join("mdbook-epub");
        Asset::from_url(url::Url::parse(url).unwrap(), dest_dir.as_path())
    }
}
