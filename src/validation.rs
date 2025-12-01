use crate::Config;
use crate::errors::Error;
use epub_builder::EpubVersion;
use mdbook_core::config::Config as MdConfig;
use std::path::Path;

pub(crate) fn validate_config_epub_version(
    epub_config: &Config,
) -> Result<Option<EpubVersion>, Error> {
    let option_version = match epub_config.epub_version {
        Some(2) => Some(EpubVersion::V20),
        Some(3) => Some(EpubVersion::V30),
        Some(v) => {
            return Err(Error::EpubDocCreate(format!(
                "Unsupported epub version specified in book.toml: {}",
                v
            )));
        }
        None => None,
    };
    Ok(option_version)
}

pub(crate) fn validate_config_title_file_name(mdbook_config: &MdConfig) -> Result<String, Error> {
    match mdbook_config.book.title.clone() {
        Some(title) => {
            // check if title is valid file name
            is_valid_filename(&title)
                .then_some(title.clone())
                .ok_or(Error::EpubBookNameOrPath(title))
        }
        None => Err(Error::EpubBookNameOrPath("".to_string())),
    }
}

/// Checks if a string can be used as a filename in Linux, macOS, and Windows.
pub fn is_valid_filename(filename: &str) -> bool {
    if filename.is_empty() {
        return false;
    }

    // Characters forbidden in Windows
    let forbidden_windows = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    if filename.chars().any(|c| forbidden_windows.contains(&c)) {
        return false;
    }

    // Windows does not allow reserved device names
    let reserved_windows = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if reserved_windows
        .iter()
        .any(|&r| r.eq_ignore_ascii_case(filename))
    {
        return false;
    }

    // Linux and macOS only forbid the null character
    if filename.contains('\0') {
        return false;
    }

    // Check for invalid or excessively long filenames
    if filename.len() > 255 {
        return false;
    }

    // Ensure the filename does not contain path components
    if Path::new(filename).components().count() != 1 {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::is_valid_filename;

    #[test]
    fn test_valid_filenames() {
        assert!(is_valid_filename("file.txt"));
        assert!(is_valid_filename("hello_world"));
        assert!(is_valid_filename("my-document.md"));
        assert!(is_valid_filename("data_123"));
        assert!(is_valid_filename("my-document.md_data_123"));
    }

    #[test]
    fn test_invalid_filenames() {
        assert!(!is_valid_filename("")); // Empty string
        assert!(!is_valid_filename("CON")); // Reserved Windows names
        assert!(!is_valid_filename("NUL"));
        assert!(!is_valid_filename("COM1"));
        assert!(!is_valid_filename("LPT1"));
        assert!(!is_valid_filename("file:name.txt")); // Forbidden characters
        assert!(!is_valid_filename("hello/world"));
        assert!(!is_valid_filename("book / name"));
        assert!(!is_valid_filename("book // name"));
        assert!(!is_valid_filename("some\\path"));
        assert!(!is_valid_filename("this\0hasnull")); // Null character
        assert!(!is_valid_filename(&"a".repeat(256))); // Too long filename
    }
}
