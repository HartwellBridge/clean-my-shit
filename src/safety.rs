//! Defensive guard rails. Nothing gets deleted unless `is_safe_target` says so.
//!
//! The categories are already built from known cache/temp locations, but this is
//! a second, independent line of defense: even if a path is somehow mangled
//! (empty env var, weird join), we refuse to purge anything that is not clearly
//! a per-user cache/temp/trash directory deep inside the home folder or the
//! OS temp area.

use std::path::{Component, Path};

/// True only for paths we consider safe to wipe the *contents* of.
pub fn is_safe_target(root: &Path) -> bool {
    // Must be an absolute path with no `..` trickery.
    if !root.is_absolute() {
        return false;
    }
    if root
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return false;
    }

    // Windows Recycle Bin lives at a drive root (e.g. C:\$Recycle.Bin) and is
    // intentionally allowed, identified by its reserved folder name.
    if root.components().any(|c| c.as_os_str() == "$Recycle.Bin") {
        return true;
    }

    // The per-user OS temp directory itself (and anything beneath it).
    let temp = std::env::temp_dir();
    if paths_equal(root, &temp) || root.starts_with(&temp) {
        return true;
    }

    #[cfg(windows)]
    if let Some(win_temp) = windows_temp() {
        if paths_equal(root, &win_temp) || root.starts_with(&win_temp) {
            return true;
        }
    }

    // Anything strictly inside the user's home directory (never the home root
    // itself). Requires at least one extra path component below home.
    if let Some(home) = dirs::home_dir() {
        if root.starts_with(&home) && !paths_equal(root, &home) {
            return root.components().count() > home.components().count();
        }
    }

    false
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    a.components().eq(b.components())
}

#[cfg(windows)]
fn windows_temp() -> Option<std::path::PathBuf> {
    let root = std::env::var_os("SystemRoot")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("C:\\Windows"));
    Some(root.join("Temp"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn rejects_relative_and_root() {
        assert!(!is_safe_target(Path::new("relative/path")));
        assert!(!is_safe_target(Path::new("/")));
    }

    #[test]
    fn rejects_parent_dir_components() {
        if let Some(home) = dirs::home_dir() {
            let sneaky = home.join("..").join("..").join("etc");
            assert!(!is_safe_target(&sneaky));
        }
    }

    #[test]
    fn rejects_home_root_but_allows_below() {
        if let Some(home) = dirs::home_dir() {
            assert!(!is_safe_target(&home));
            let below: PathBuf = home.join("Library").join("Caches");
            assert!(is_safe_target(&below));
        }
    }
}
