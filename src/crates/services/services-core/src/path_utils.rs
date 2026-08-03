//! Path and permission utilities with no heavy dependencies.
//!
//! This module is **not** feature-gated. It collects functions that only need
//! `std` (plus the lightweight `dirs`, `sha2`, `hex` crates already declared as
//! non-optional dependencies) so that consumers without the `filesystem` feature
//! can still use them. Functions that require `tokio::fs` or the `windows`
//! crate stay in their gated modules under `filesystem/`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Permission helpers (moved from `filesystem/permissions.rs`).
// These use only `std::os::unix`/`std::os::windows` traits — no `tokio` or
// `windows` crate — so they belong in the non-gated module.
// ---------------------------------------------------------------------------

/// Sets file permissions to the given mode (Unix), no-op on non-Unix.
/// Synchronous version using `std::fs`.
pub fn set_mode(path: &Path, mode: u32) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
    }

    #[cfg(not(unix))]
    {
        let _ = (path, mode);
        Ok(())
    }
}

/// Returns the file mode bits (Unix), or `None` on non-Unix.
pub fn file_mode(metadata: &std::fs::Metadata) -> Option<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        Some(metadata.permissions().mode())
    }

    #[cfg(not(unix))]
    {
        let _ = metadata;
        None
    }
}

/// Formats file permissions as an rwx string (e.g. `rwxr-xr-x` on Unix,
/// `r--` or `rw-` on Windows).
pub fn permissions_string(metadata: &std::fs::Metadata) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode();

        let user = format!(
            "{}{}{}",
            if mode & 0o400 != 0 { "r" } else { "-" },
            if mode & 0o200 != 0 { "w" } else { "-" },
            if mode & 0o100 != 0 { "x" } else { "-" }
        );
        let group = format!(
            "{}{}{}",
            if mode & 0o040 != 0 { "r" } else { "-" },
            if mode & 0o020 != 0 { "w" } else { "-" },
            if mode & 0o010 != 0 { "x" } else { "-" }
        );
        let other = format!(
            "{}{}{}",
            if mode & 0o004 != 0 { "r" } else { "-" },
            if mode & 0o002 != 0 { "w" } else { "-" },
            if mode & 0o001 != 0 { "x" } else { "-" }
        );

        format!("{}{}{}", user, group, other)
    }

    #[cfg(not(unix))]
    {
        let readonly = metadata.permissions().readonly();
        (if readonly { "r--" } else { "rw-" }).to_string()
    }
}

// ---------------------------------------------------------------------------
// Path inspection helpers (moved from `filesystem/path_inspection.rs`).
// These use only `std` platform traits — no `tokio` or `windows` crate.
// `path_has_multiple_hard_links` stays in `path_inspection.rs` because it
// needs the `windows::Win32` API on Windows.
// ---------------------------------------------------------------------------

/// Returns whether a path uses a Windows device namespace (`\\?\` or `\\.\`).
/// Device paths bypass normal path resolution and can access raw devices.
/// Always returns `false` on non-Windows platforms.
pub fn is_device_path(path: &Path) -> bool {
    #[cfg(windows)]
    {
        let normalized = path.to_string_lossy().replace('/', "\\");
        normalized.starts_with(r"\\?\") || normalized.starts_with(r"\\.\")
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        false
    }
}

/// Returns whether a relative path contains a Windows alternate data stream
/// marker (`:` in any component). On non-Windows platforms always returns
/// `false` since ADS is an NTFS-only feature.
pub fn has_alternate_data_stream(path: &str) -> bool {
    #[cfg(windows)]
    {
        path.split('/').any(|component| component.contains(':'))
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        false
    }
}

/// Compares two workspace-relative path strings for equality. On Windows the
/// comparison is case-insensitive because the filesystem is case-insensitive;
/// on all other platforms it is case-sensitive.
pub fn workspace_relative_path_eq(left: &str, right: &str) -> bool {
    #[cfg(windows)]
    {
        left.to_lowercase() == right.to_lowercase()
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

/// Strips a workspace root prefix from a path, returning the relative
/// remainder with forward slashes. On Windows, if the case-sensitive
/// `strip_prefix` fails, falls back to a case-insensitive comparison.
pub fn local_workspace_relative_path(path: &Path, root: &Path) -> Option<String> {
    if let Ok(relative) = path.strip_prefix(root) {
        return Some(relative.to_string_lossy().replace('\\', "/"));
    }

    #[cfg(windows)]
    {
        let path = path.to_string_lossy().replace('\\', "/");
        let root = root
            .to_string_lossy()
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_string();
        if path.eq_ignore_ascii_case(&root) {
            return Some(String::new());
        }
        let prefix = format!("{root}/");
        if path
            .get(..prefix.len())
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(&prefix))
        {
            return path.get(prefix.len()..).map(str::to_string);
        }
    }

    #[cfg(not(windows))]
    {
        let _ = (path, root);
    }

    None
}

/// Normalizes path separators in a workspace-relative path string by replacing
/// backslashes with forward slashes. On Windows this ensures `\` separators
/// become `/` for consistent workspace-relative comparisons. On Unix, backslash
/// is a valid filename character and is left untouched.
pub fn normalize_path_separators(path: &str) -> String {
    #[cfg(windows)]
    {
        path.replace('\\', "/")
    }
    #[cfg(not(windows))]
    {
        path.to_string()
    }
}

/// Returns whether a file is a symlink or a Windows reparse point.
/// On all platforms, `is_symlink()` is checked. On Windows, the
/// `FILE_ATTRIBUTE_REPARSE_POINT` flag is also inspected to catch junction
/// points and other reparse-point aliases that `is_symlink()` does not report.
pub fn is_symlink_or_reparse_point(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    false
}

/// Converts a path to its platform-native byte representation.
///
/// On Unix this uses `OsStr::as_bytes()`; on Windows it uses
/// `OsStr::encode_wide()` with little-endian bytes; on other platforms it
/// falls back to `to_string_lossy().as_bytes()`.
pub fn path_to_native_bytes(path: &Path) -> Vec<u8> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes().to_vec()
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        path.as_os_str()
            .encode_wide()
            .flat_map(u16::to_le_bytes)
            .collect()
    }

    #[cfg(not(any(unix, windows)))]
    {
        path.to_string_lossy().as_bytes().to_vec()
    }
}

/// Normalizes a path string for case-insensitive filesystems.
///
/// On Windows, lowercases the string because NTFS is case-insensitive.
/// On other platforms, returns the string unchanged.
pub fn normalize_path_case(s: &str) -> String {
    #[cfg(windows)]
    {
        s.to_ascii_lowercase()
    }

    #[cfg(not(windows))]
    {
        s.to_string()
    }
}

/// Returns a deduplication key for a filesystem path.
///
/// On Windows the key is lowercased because NTFS is case-insensitive.
/// On other platforms the original OS string is preserved byte-for-byte.
pub fn path_search_key(path: &Path) -> OsString {
    #[cfg(windows)]
    {
        OsString::from(path.to_string_lossy().to_ascii_lowercase())
    }

    #[cfg(not(windows))]
    {
        path.as_os_str().to_os_string()
    }
}

/// Returns platform-specific executable search directories.
///
/// On macOS this includes Homebrew `bin`/`sbin` and versioned node formula
/// paths. On other platforms this returns an empty vector.
pub fn system_executable_search_paths() -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let mut paths = Vec::new();
        for prefix in ["/opt/homebrew", "/usr/local"] {
            paths.push(PathBuf::from(format!("{prefix}/bin")));
            paths.push(PathBuf::from(format!("{prefix}/sbin")));
            for node in ["node", "node@18", "node@20", "node@22", "node@24"] {
                paths.push(PathBuf::from(format!("{prefix}/opt/{node}/bin")));
            }
        }
        paths
    }

    #[cfg(not(target_os = "macos"))]
    {
        Vec::new()
    }
}

/// Generates candidate executable paths in a directory for a command name.
///
/// On Windows, appends each extension from `PATHEXT` (defaulting to
/// `.EXE;.BAT;.CMD`) unless the command already has an extension.
/// On other platforms, returns just `directory.join(command)`.
pub fn executable_candidates(directory: &Path, command: &str) -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let command_path = PathBuf::from(command);
        if command_path.extension().is_some() {
            return vec![directory.join(command)];
        }
        let extensions =
            std::env::var_os("PATHEXT").unwrap_or_else(|| OsString::from(".EXE;.BAT;.CMD"));
        extensions
            .to_string_lossy()
            .split(';')
            .filter(|extension| !extension.is_empty())
            .map(|extension| directory.join(format!("{command}{extension}")))
            .collect()
    }

    #[cfg(not(windows))]
    {
        vec![directory.join(command)]
    }
}

// ---------------------------------------------------------------------------
// Lightweight-dep helpers (dirs, sha2, hex — non-optional dependencies).
// ---------------------------------------------------------------------------

/// Returns the platform-specific base directory for application data.
///
/// - Windows: `%APPDATA%` (Roaming), e.g. `C:\Users\xxx\AppData\Roaming`
/// - macOS: `~/Library/Application Support`
/// - Other (Linux etc.): `~/.local/share`
///
/// Falls back to platform-appropriate default paths when environment
/// variables are not set.
pub fn app_data_dir() -> PathBuf {
    if cfg!(target_os = "windows") {
        dirs::data_dir().unwrap_or_else(|| PathBuf::from("C:\\ProgramData"))
    } else if cfg!(target_os = "macos") {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("Library")
            .join("Application Support")
    } else {
        dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("/tmp"))
    }
}

/// Computes a SHA-256 hex digest of the platform-native byte representation
/// of a path.
///
/// On Unix this uses `OsStr::as_bytes()`; on Windows it uses
/// `OsStr::encode_wide()` with little-endian bytes so that paths differing
/// only in unpaired UTF-16 surrogates produce different digests; on other
/// platforms it falls back to `to_string_lossy().as_bytes()`.
pub fn native_path_digest(path: &Path) -> String {
    use sha2::{Digest, Sha256};

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        hex::encode(Sha256::digest(path.as_os_str().as_bytes()))
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let mut hasher = Sha256::new();
        for unit in path.as_os_str().encode_wide() {
            hasher.update(unit.to_le_bytes());
        }
        hex::encode(hasher.finalize())
    }

    #[cfg(not(any(unix, windows)))]
    {
        hex::encode(Sha256::digest(path.to_string_lossy().as_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn normalize_path_separators_replaces_backslashes() {
        assert_eq!(
            normalize_path_separators(r"src\windows\name.rs"),
            "src/windows/name.rs"
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn normalize_path_separators_preserves_literal_backslashes() {
        assert_eq!(
            normalize_path_separators(r"src/literal\name.rs"),
            r"src/literal\name.rs"
        );
    }

    #[test]
    fn native_path_digest_is_deterministic() {
        let path = Path::new("/some/path");
        assert_eq!(native_path_digest(path), native_path_digest(path));
    }

    #[cfg(windows)]
    #[test]
    fn native_path_digest_distinguishes_lossy_utf16_paths() {
        use std::os::windows::ffi::OsStringExt;

        let first = PathBuf::from(OsString::from_wide(&[
            b'C' as u16,
            b':' as u16,
            b'\\' as u16,
            0xd800,
        ]));
        let second = PathBuf::from(OsString::from_wide(&[
            b'C' as u16,
            b':' as u16,
            b'\\' as u16,
            0xd801,
        ]));

        assert_eq!(first.to_string_lossy(), second.to_string_lossy());
        assert_ne!(native_path_digest(&first), native_path_digest(&second));
    }
}
