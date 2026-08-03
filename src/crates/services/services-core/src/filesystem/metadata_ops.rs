//! Filesystem metadata operations that require the `filesystem` feature.
//!
//! These functions need platform-native APIs (`windows::Win32` on Windows,
//! `tokio::fs` for async I/O) and therefore must stay behind the
//! `filesystem` feature gate. The lightweight, pure-std equivalents
//! (`set_mode`, `file_mode`, `permissions_string`, etc.) live in the
//! non-gated `crate::path_utils` module.

use std::path::Path;

/// Returns whether a regular filesystem entry has more than one hard link.
/// Callers that use paths as a security boundary can reject such aliases.
pub fn path_has_multiple_hard_links(path: &Path) -> std::io::Result<bool> {
    let metadata = std::fs::metadata(path)?;
    if metadata.is_dir() {
        return Ok(false);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(metadata.nlink() > 1)
    }

    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
        };

        let file = std::fs::File::open(path)?;
        let mut information = BY_HANDLE_FILE_INFORMATION::default();
        unsafe {
            GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut information)
                .map_err(|error| std::io::Error::other(error.to_string()))?;
        }
        Ok(information.nNumberOfLinks > 1)
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = (path, metadata);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "hard-link identity checks are unavailable on this platform",
        ))
    }
}

/// Sets file permissions to the given mode (Unix), no-op on non-Unix.
/// Asynchronous version using `tokio::fs`, for callers that previously
/// used `tokio::fs::set_permissions`.
pub async fn set_mode_async(path: &std::path::Path, mode: u32) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).await
    }

    #[cfg(not(unix))]
    {
        let _ = (path, mode);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_link_file_has_no_multiple_hard_links() {
        let dir = tempfile::tempdir().expect("create temp directory");
        let file = dir.path().join("file.txt");
        std::fs::write(&file, "content").expect("write file");

        let result =
            path_has_multiple_hard_links(&file).expect("metadata should succeed for regular file");
        assert!(
            !result,
            "a freshly created file should not report multiple hard links"
        );
    }

    #[test]
    #[cfg(any(unix, windows))]
    fn hard_linked_file_reports_multiple_links() {
        let dir = tempfile::tempdir().expect("create temp directory");
        let original = dir.path().join("original.txt");
        let alias = dir.path().join("alias.txt");
        std::fs::write(&original, "content").expect("write original file");
        std::fs::hard_link(&original, &alias).expect("create hard link");

        let result_original = path_has_multiple_hard_links(&original)
            .expect("metadata should succeed for original");
        assert!(
            result_original,
            "original should report multiple hard links after alias is created"
        );

        let result_alias =
            path_has_multiple_hard_links(&alias).expect("metadata should succeed for alias");
        assert!(
            result_alias,
            "alias should report multiple hard links (same inode as original)"
        );
    }

    #[test]
    fn directory_never_reports_multiple_hard_links() {
        let dir = tempfile::tempdir().expect("create temp directory");

        let result = path_has_multiple_hard_links(dir.path())
            .expect("metadata should succeed for directory");
        assert!(
            !result,
            "directories should always return false (early return before link check)"
        );
    }

    #[test]
    fn non_existent_path_returns_error() {
        let dir = tempfile::tempdir().expect("create temp directory");
        let missing = dir.path().join("does_not_exist.txt");

        let result = path_has_multiple_hard_links(&missing);
        assert!(
            result.is_err(),
            "non-existent path should return an error from metadata lookup"
        );
    }
}
