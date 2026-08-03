//! Concrete MiniApp runtime probe — platform-aware implementation.
//!
//! The `MiniAppRuntimeProbe` trait is defined in `bitfun-product-domains`
//! (Contracts layer). This module provides the default implementation that
//! uses `which`, filesystem checks, and `CREATE_NO_WINDOW` on Windows.

use std::path::{Path, PathBuf};

use bitfun_product_domains::miniapp::runtime::{
    detect_runtime_with_probe, DetectedRuntime, MiniAppRuntimeProbe,
};
use bitfun_services_core::process_manager::create_command;

pub struct DefaultMiniAppRuntimeProbe;

impl MiniAppRuntimeProbe for DefaultMiniAppRuntimeProbe {
    fn find_on_path(&self, name: &str) -> Option<PathBuf> {
        which::which(name).ok()
    }

    fn home_dir(&self) -> Option<PathBuf> {
        home_dir()
    }

    fn is_executable(&self, path: &Path) -> bool {
        is_executable(path)
    }

    fn version_dirs(&self, root: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(root)
            .map(|read| read.flatten().map(|entry| entry.path()).collect())
            .unwrap_or_default()
    }

    fn runtime_version(&self, path: &Path) -> Option<String> {
        get_version(path).ok()
    }
}

pub fn detect_runtime() -> Option<DetectedRuntime> {
    detect_runtime_with_probe(&DefaultMiniAppRuntimeProbe)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn is_executable(path: &Path) -> bool {
    path.is_file()
}

fn get_version(executable: &Path) -> Result<String, std::io::Error> {
    let output = create_command(executable).arg("--version").output()?;
    if output.status.success() {
        let version = String::from_utf8_lossy(&output.stdout);
        Ok(version.trim().to_string())
    } else {
        Err(std::io::Error::other("version check failed"))
    }
}
