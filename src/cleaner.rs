//! Deletion logic. The only place in the program that removes files.

use crate::categories::Category;
use crate::safety::is_safe_target;
use std::path::Path;

/// Result of cleaning a category.
pub struct Cleaned {
    pub freed: u64,
    pub errors: Vec<String>,
}

/// Delete the contents of every (safe, existing) root of a category.
pub fn clean_category(cat: &Category) -> Cleaned {
    let mut freed = 0u64;
    let mut errors = Vec::new();

    for root in &cat.roots {
        if !is_safe_target(root) {
            errors.push(format!("refused unsafe path: {}", root.display()));
            continue;
        }
        let (f, mut e) = purge_contents(root);
        freed += f;
        errors.append(&mut e);
    }

    Cleaned { freed, errors }
}

/// Delete everything *inside* `root`, keeping `root` itself.
fn purge_contents(root: &Path) -> (u64, Vec<String>) {
    let mut freed = 0u64;
    let mut errors = Vec::new();

    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        // Missing dir = nothing to do; unreadable = report once and move on.
        Err(_) => return (0, errors),
    };

    for entry in entries.filter_map(Result::ok) {
        let (f, mut e) = purge_path(&entry.path());
        freed += f;
        errors.append(&mut e);
    }

    (freed, errors)
}

/// Recursively delete a single path (file, dir, or symlink) and return the
/// number of bytes actually reclaimed plus any errors hit along the way.
///
/// Symlinks are removed as links — never followed — so deletion can't escape
/// the intended tree.
fn purge_path(path: &Path) -> (u64, Vec<String>) {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) => return (0, vec![format!("{}: {e}", path.display())]),
    };
    let file_type = meta.file_type();

    if file_type.is_symlink() {
        // Remove the link itself. Try file first, then dir (Windows dir-links).
        if std::fs::remove_file(path).is_err() {
            let _ = std::fs::remove_dir(path);
        }
        return (0, Vec::new());
    }

    if file_type.is_file() {
        let len = meta.len();
        return match std::fs::remove_file(path) {
            Ok(()) => (len, Vec::new()),
            Err(e) => (0, vec![format!("{}: {e}", path.display())]),
        };
    }

    // Directory: delete children first, then the (now-empty) directory.
    let mut freed = 0u64;
    let mut errors = Vec::new();

    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.filter_map(Result::ok) {
            let (f, mut e) = purge_path(&entry.path());
            freed += f;
            errors.append(&mut e);
        }
    }

    if let Err(e) = std::fs::remove_dir(path) {
        errors.push(format!("{}: {e}", path.display()));
    }

    (freed, errors)
}
