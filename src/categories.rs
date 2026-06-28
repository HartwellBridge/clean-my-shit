//! Platform-specific definitions of what can be cleaned.
//!
//! A category is either a set of cache *roots* whose contents we purge, or a
//! "find" rule that discovers matching directories anywhere under the home dir
//! (e.g. `node_modules`). Either way it resolves to a list of target paths.

use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Risk {
    /// Safe to delete; regenerated automatically by the OS or apps.
    Safe,
    /// Deletion is irreversible, or removes regenerable project state.
    Caution,
}

/// How a category's deletable targets are produced.
#[derive(Clone)]
pub enum Source {
    /// Delete the immediate contents of these directories (the dirs stay).
    Contents(Vec<PathBuf>),
    /// Walk `base` and match directories named `name`; each match is deleted
    /// whole. Matches are not descended into. `sibling`, if set, requires a
    /// file of that name next to the match (e.g. `Cargo.toml` beside `target`).
    FindDirs {
        base: PathBuf,
        name: &'static str,
        sibling: Option<&'static str>,
        max_depth: usize,
    },
}

#[derive(Clone)]
pub struct Category {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub source: Source,
    pub risk: Risk,
    pub default_on: bool,
}

/// Build the category list for the current platform, dropping any that can't
/// resolve on this machine.
pub fn categories() -> Vec<Category> {
    let mut cats = platform_categories();
    cats.extend(dev_find_categories());
    cats.retain(|c| match &c.source {
        Source::Contents(roots) => !roots.is_empty(),
        Source::FindDirs { base, .. } => base.is_dir(),
    });
    cats
}

/// Join path parts onto the user's home directory. Returns 0 or 1 paths.
fn home(parts: &[&str]) -> Vec<PathBuf> {
    match dirs::home_dir() {
        Some(mut p) => {
            for part in parts {
                p.push(part);
            }
            vec![p]
        }
        None => vec![],
    }
}

/// Join path parts onto %LOCALAPPDATA% / local data dir.
#[cfg(any(target_os = "windows", not(any(target_os = "macos", target_os = "windows"))))]
fn local(parts: &[&str]) -> Vec<PathBuf> {
    match dirs::data_local_dir() {
        Some(mut p) => {
            for part in parts {
                p.push(part);
            }
            vec![p]
        }
        None => vec![],
    }
}

fn cat(
    id: &'static str,
    name: &'static str,
    description: &'static str,
    source: Source,
    risk: Risk,
    default_on: bool,
) -> Category {
    Category {
        id,
        name,
        description,
        source,
        risk,
        default_on,
    }
}

/// Cross-platform "find scattered developer junk" categories. Off by default
/// (Caution) — these remove regenerable project state, not just caches.
fn dev_find_categories() -> Vec<Category> {
    let Some(home) = dirs::home_dir() else {
        return vec![];
    };
    vec![
        cat(
            "node_modules",
            "node_modules folders",
            "JavaScript dependencies anywhere under your home. Restore with `npm install`.",
            Source::FindDirs {
                base: home.clone(),
                name: "node_modules",
                sibling: None,
                max_depth: 9,
            },
            Risk::Caution,
            false,
        ),
        cat(
            "rust_target",
            "Rust build output",
            "`target/` directories next to a Cargo.toml. Rebuild with `cargo build`.",
            Source::FindDirs {
                base: home.clone(),
                name: "target",
                sibling: Some("Cargo.toml"),
                max_depth: 9,
            },
            Risk::Caution,
            false,
        ),
        cat(
            "py_cache",
            "Python __pycache__",
            "Compiled Python bytecode caches. Regenerated automatically.",
            Source::FindDirs {
                base: home,
                name: "__pycache__",
                sibling: None,
                max_depth: 9,
            },
            Risk::Safe,
            false,
        ),
    ]
}

// ---------------------------------------------------------------------------
// macOS
// ---------------------------------------------------------------------------
#[cfg(target_os = "macos")]
fn platform_categories() -> Vec<Category> {
    let mut device_support = Vec::new();
    for kind in ["iOS DeviceSupport", "watchOS DeviceSupport", "tvOS DeviceSupport"] {
        device_support.extend(home(&["Library", "Developer", "Xcode", kind]));
    }

    vec![
        cat(
            "user_caches",
            "Application Caches",
            "~/Library/Caches — app, browser, Homebrew, pip & yarn caches.",
            Source::Contents(home(&["Library", "Caches"])),
            Risk::Safe,
            true,
        ),
        cat(
            "app_logs",
            "Application Logs",
            "~/Library/Logs — diagnostic logs written by apps.",
            Source::Contents(home(&["Library", "Logs"])),
            Risk::Safe,
            true,
        ),
        cat(
            "temp_files",
            "Temporary Files",
            "Per-user temp directory ($TMPDIR).",
            Source::Contents(vec![std::env::temp_dir()]),
            Risk::Safe,
            true,
        ),
        cat(
            "xcode_derived",
            "Xcode Derived Data",
            "Build intermediates & indexes. Rebuilt on next build.",
            Source::Contents(home(&["Library", "Developer", "Xcode", "DerivedData"])),
            Risk::Safe,
            true,
        ),
        cat(
            "xcode_device_support",
            "Xcode Device Support",
            "Symbols cached per connected device/OS. Re-fetched when needed.",
            Source::Contents(device_support),
            Risk::Safe,
            true,
        ),
        cat(
            "simulator_caches",
            "iOS Simulator Caches",
            "CoreSimulator caches.",
            Source::Contents(home(&["Library", "Developer", "CoreSimulator", "Caches"])),
            Risk::Safe,
            true,
        ),
        cat(
            "npm_cache",
            "npm Cache",
            "~/.npm/_cacache — re-downloaded on demand.",
            Source::Contents(home(&[".npm", "_cacache"])),
            Risk::Safe,
            true,
        ),
        cat(
            "cargo_cache",
            "Cargo Registry Cache",
            "~/.cargo/registry/cache — downloaded crate archives.",
            Source::Contents(home(&[".cargo", "registry", "cache"])),
            Risk::Safe,
            true,
        ),
        cat(
            "trash",
            "Trash",
            "~/.Trash — emptying is irreversible.",
            Source::Contents(home(&[".Trash"])),
            Risk::Caution,
            false,
        ),
    ]
}

// ---------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------
#[cfg(target_os = "windows")]
fn platform_categories() -> Vec<Category> {
    let windows_temp = {
        let root = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("C:\\Windows"));
        vec![root.join("Temp")]
    };

    let mut browser = Vec::new();
    browser.extend(local(&["Google", "Chrome", "User Data", "Default", "Cache"]));
    browser.extend(local(&["Google", "Chrome", "User Data", "Default", "Code Cache"]));
    browser.extend(local(&["Microsoft", "Edge", "User Data", "Default", "Cache"]));
    browser.extend(local(&["Microsoft", "Edge", "User Data", "Default", "Code Cache"]));

    vec![
        cat(
            "user_temp",
            "User Temp Files",
            "%LOCALAPPDATA%\\Temp — per-user scratch files.",
            Source::Contents(local(&["Temp"])),
            Risk::Safe,
            true,
        ),
        cat(
            "windows_temp",
            "Windows Temp Files",
            "C:\\Windows\\Temp — system scratch files (some may be in use).",
            Source::Contents(windows_temp),
            Risk::Safe,
            true,
        ),
        cat(
            "crash_dumps",
            "Crash Dumps",
            "%LOCALAPPDATA%\\CrashDumps — saved crash reports.",
            Source::Contents(local(&["CrashDumps"])),
            Risk::Safe,
            true,
        ),
        cat(
            "browser_caches",
            "Browser Caches",
            "Chrome & Edge on-disk caches. Bookmarks/history untouched.",
            Source::Contents(browser),
            Risk::Safe,
            true,
        ),
        cat(
            "npm_cache",
            "npm Cache",
            "%LOCALAPPDATA%\\npm-cache — re-downloaded on demand.",
            Source::Contents(local(&["npm-cache"])),
            Risk::Safe,
            true,
        ),
        cat(
            "pip_cache",
            "pip Cache",
            "%LOCALAPPDATA%\\pip\\Cache — downloaded Python wheels.",
            Source::Contents(local(&["pip", "Cache"])),
            Risk::Safe,
            true,
        ),
        cat(
            "cargo_cache",
            "Cargo Registry Cache",
            "~/.cargo/registry/cache — downloaded crate archives.",
            Source::Contents(home(&[".cargo", "registry", "cache"])),
            Risk::Safe,
            true,
        ),
        cat(
            "recycle_bin",
            "Recycle Bin",
            "C:\\$Recycle.Bin — emptying is irreversible.",
            Source::Contents(vec![PathBuf::from("C:\\$Recycle.Bin")]),
            Risk::Caution,
            false,
        ),
    ]
}

// ---------------------------------------------------------------------------
// Linux / other
// ---------------------------------------------------------------------------
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn platform_categories() -> Vec<Category> {
    let user_cache = match dirs::cache_dir() {
        Some(p) => vec![p],
        None => vec![],
    };

    vec![
        cat(
            "user_caches",
            "Application Caches",
            "~/.cache — app caches.",
            Source::Contents(user_cache),
            Risk::Safe,
            true,
        ),
        cat(
            "temp_files",
            "Temporary Files",
            "Per-user temp directory.",
            Source::Contents(vec![std::env::temp_dir()]),
            Risk::Safe,
            true,
        ),
        cat(
            "npm_cache",
            "npm Cache",
            "~/.npm/_cacache — re-downloaded on demand.",
            Source::Contents(home(&[".npm", "_cacache"])),
            Risk::Safe,
            true,
        ),
        cat(
            "cargo_cache",
            "Cargo Registry Cache",
            "~/.cargo/registry/cache — downloaded crate archives.",
            Source::Contents(home(&[".cargo", "registry", "cache"])),
            Risk::Safe,
            true,
        ),
        cat(
            "trash",
            "Trash",
            "~/.local/share/Trash — emptying is irreversible.",
            Source::Contents(home(&[".local", "share", "Trash", "files"])),
            Risk::Caution,
            false,
        ),
    ]
}
