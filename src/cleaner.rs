//! Deletion logic. The only place in the program that removes files.

use crate::categories::Category;
use crate::safety::is_safe_target;
use crate::scanner;
use std::path::Path;

/// Result of cleaning a category.
pub struct Cleaned {
    pub freed: u64,
    pub errors: Vec<String>,
}

/// Delete every (safe) target of a category and report bytes reclaimed.
///
/// Targets are re-resolved here (not reused from the scan) so the deletion
/// reflects the filesystem as it is right now.
pub fn clean_category(cat: &Category) -> Cleaned {
    let mut freed = 0u64;
    let mut errors = Vec::new();

    for target in scanner::targets(cat) {
        // Belt and suspenders: targets() already filtered, re-check anyway.
        if !is_safe_target(&target) {
            errors.push(format!("refused unsafe path: {}", target.display()));
            continue;
        }
        let (f, mut e) = purge_path(&target);
        freed += f;
        errors.append(&mut e);
    }

    Cleaned { freed, errors }
}

/// Recursively delete a path (file, dir, or symlink) and return bytes reclaimed
/// plus any errors. Symlinks are removed as links — never followed — so
/// deletion can't escape the intended tree.
fn purge_path(path: &Path) -> (u64, Vec<String>) {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) => return (0, vec![format!("{}: {e}", path.display())]),
    };
    let file_type = meta.file_type();

    if file_type.is_symlink() {
        if std::fs::remove_file(path).is_err() {
            let _ = std::fs::remove_dir(path); // Windows dir-symlink
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
