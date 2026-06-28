//! Read-only disk usage measurement and target discovery.

use crate::categories::{Category, Source};
use crate::safety::is_safe_target;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// One deletable target — shown in previews and removed on clean.
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

/// Total size in bytes of all regular files under `root`. Symlinks are never
/// followed (and not counted).
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

/// Size of a single target (file len, or recursive size for a dir). Symlinks
/// count as zero — we only ever remove the link, not its destination.
fn entry_size(path: &Path) -> u64 {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => 0,
        Ok(m) if m.is_file() => m.len(),
        Ok(_) => dir_size(path),
        Err(_) => 0,
    }
}

/// Resolve a category to the concrete list of paths that would be deleted.
/// Every returned path has already passed the safety guard.
pub fn targets(cat: &Category) -> Vec<PathBuf> {
    let mut out = Vec::new();
    match &cat.source {
        Source::Contents(roots) => {
            for root in roots.iter().filter(|r| is_safe_target(r)) {
                if let Ok(read_dir) = std::fs::read_dir(root) {
                    out.extend(read_dir.filter_map(Result::ok).map(|e| e.path()));
                }
            }
        }
        Source::FindDirs {
            base,
            name,
            sibling,
            max_depth,
        } => {
            find_dirs(base, name, sibling, 0, *max_depth, &mut out);
        }
    }
    out.retain(|p| is_safe_target(p));
    out
}

/// Recursively find directories named `name` under `base`. Matches are recorded
/// but not descended into; symlinks, hidden dirs and known-heavy/irrelevant
/// directories are skipped for speed and safety.
fn find_dirs(
    base: &Path,
    name: &str,
    sibling: &Option<&'static str>,
    depth: usize,
    max_depth: usize,
    out: &mut Vec<PathBuf>,
) {
    if depth > max_depth {
        return;
    }
    let read_dir = match std::fs::read_dir(base) {
        Ok(rd) => rd,
        Err(_) => return,
    };
    for entry in read_dir.filter_map(Result::ok) {
        // file_type() does not follow symlinks: is_dir() is true only for real
        // directories, so symlinked dirs are skipped (no loops, no escapes).
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if !is_dir {
            continue;
        }
        let file_name = entry.file_name();
        let n = file_name.to_string_lossy();
        let path = entry.path();

        if n == name {
            let sibling_ok = sibling
                .map(|s| path.parent().map(|p| p.join(s).exists()).unwrap_or(false))
                .unwrap_or(true);
            if sibling_ok {
                out.push(path);
            }
            continue; // never descend into a match
        }
        if n.starts_with('.') || is_noise(&n) {
            continue;
        }
        find_dirs(&path, name, sibling, depth + 1, max_depth, out);
    }
}

/// Directories not worth descending into when hunting for dev junk.
fn is_noise(name: &str) -> bool {
    matches!(
        name,
        "Library" | "Applications" | "Music" | "Movies" | "Pictures" | "Photos" | "go"
    ) || name.ends_with(".app")
        || name.ends_with(".photoslibrary")
        || name.ends_with(".framework")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_matches_without_descending_into_them() {
        let root = std::env::temp_dir().join(format!("cms_find_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mk = |p: &str| std::fs::create_dir_all(root.join(p)).unwrap();
        mk("proj/node_modules/pkg");
        mk("proj/sub/node_modules");
        mk("proj/node_modules/nested/node_modules"); // inside a match → ignored

        let mut out = Vec::new();
        find_dirs(&root, "node_modules", &None, 0, 9, &mut out);
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(out.len(), 2, "expected 2 top-level matches, got {out:?}");
        assert!(out.iter().all(|p| p.ends_with("node_modules")));
    }

    #[test]
    fn sibling_requirement_is_enforced() {
        let root = std::env::temp_dir().join(format!("cms_sib_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("with/target")).unwrap();
        std::fs::write(root.join("with/Cargo.toml"), b"").unwrap();
        std::fs::create_dir_all(root.join("without/target")).unwrap();

        let mut out = Vec::new();
        find_dirs(&root, "target", &Some("Cargo.toml"), 0, 9, &mut out);
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(out.len(), 1, "only target next to Cargo.toml, got {out:?}");
        assert!(out[0].ends_with("with/target"));
    }
}

/// Scan a category: resolve its targets, size each in parallel, return totals
/// and a largest-first breakdown.
pub fn scan_category(cat: &Category) -> CatScan {
    use rayon::prelude::*;

    let mut entries: Vec<Entry> = targets(cat)
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
