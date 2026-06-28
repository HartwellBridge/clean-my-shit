//! Read-only disk usage measurement.

use crate::categories::Category;
use crate::safety::is_safe_target;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// One immediate child of a category root — the unit shown in previews and the
/// unit that gets deleted.
pub struct Entry {
    pub path: PathBuf,
    pub size: u64,
}

/// Result of scanning a category: total reclaimable bytes plus the per-item
/// breakdown (sorted largest first).
pub struct CatScan {
    pub total: u64,
    pub entries: Vec<Entry>,
}

/// Total size in bytes of all regular files under `root`.
///
/// Symlinks are never followed (and not counted), so we can't be tricked into
/// measuring something outside the target tree.
pub fn dir_size(root: &Path) -> u64 {
    if !root.exists() {
        return 0;
    }
    WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

/// Size of a single immediate child (file len, or recursive size for a dir).
/// Symlinks count as zero — we'd only ever remove the link, not its target.
fn entry_size(path: &Path) -> u64 {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => 0,
        Ok(m) if m.is_file() => m.len(),
        Ok(_) => dir_size(path),
        Err(_) => 0,
    }
}

/// Scan a category: enumerate immediate children of every safe root, size each
/// (children sized in parallel), and return totals + a largest-first list.
pub fn scan_category(cat: &Category) -> CatScan {
    use rayon::prelude::*;

    let mut children: Vec<PathBuf> = Vec::new();
    for root in cat.roots.iter().filter(|r| is_safe_target(r)) {
        if let Ok(read_dir) = std::fs::read_dir(root) {
            children.extend(read_dir.filter_map(Result::ok).map(|e| e.path()));
        }
    }

    let mut entries: Vec<Entry> = children
        .par_iter()
        .map(|path| Entry {
            path: path.clone(),
            size: entry_size(path),
        })
        .filter(|e| e.size > 0)
        .collect();

    entries.sort_by(|a, b| b.size.cmp(&a.size));
    let total = entries.iter().map(|e| e.size).sum();

    CatScan { total, entries }
}
