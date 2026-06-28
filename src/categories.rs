//! Platform-specific definitions of what can be cleaned.
//!
//! Every category is just a list of directory *roots* whose contents we purge.
//! Roots are deliberately non-overlapping so sizes aren't double-counted, and
//! we only ever target caches, temp files, logs and trash — never documents.

use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Risk {
    /// Safe to delete; regenerated automatically by the OS or apps.
    Safe,
    /// Deletion is irreversible or may slow the next launch of something.
    Caution,
}

#[derive(Clone)]
pub struct Category {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// Directories whose *contents* are deleted (the directory itself stays).
    pub roots: Vec<PathBuf>,
    pub risk: Risk,
    pub default_on: bool,
}

/// Build the category list for the current platform. Categories with no
/// resolvable roots are dropped.
pub fn categories() -> Vec<Category> {
    let mut cats = platform_categories();
    cats.retain(|c| !c.roots.is_empty());
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

/// Join path parts onto %LOCALAPPDATA% (Windows) / local data dir.
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
    roots: Vec<PathBuf>,
    risk: Risk,
    default_on: bool,
) -> Category {
    Category {
        id,
        name,
        description,
        roots,
        risk,
        default_on,
    }
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
            home(&["Library", "Caches"]),
            Risk::Safe,
            true,
        ),
        cat(
            "app_logs",
            "Application Logs",
            "~/Library/Logs — diagnostic logs written by apps.",
            home(&["Library", "Logs"]),
            Risk::Safe,
            true,
        ),
        cat(
            "temp_files",
            "Temporary Files",
            "Per-user temp directory ($TMPDIR).",
            vec![std::env::temp_dir()],
            Risk::Safe,
            true,
        ),
        cat(
            "xcode_derived",
            "Xcode Derived Data",
            "Build intermediates & indexes. Rebuilt on next build.",
            home(&["Library", "Developer", "Xcode", "DerivedData"]),
            Risk::Safe,
            true,
        ),
        cat(
            "xcode_device_support",
            "Xcode Device Support",
            "Symbols cached per connected device/OS. Re-fetched when needed.",
            device_support,
            Risk::Safe,
            true,
        ),
        cat(
            "simulator_caches",
            "iOS Simulator Caches",
            "CoreSimulator caches.",
            home(&["Library", "Developer", "CoreSimulator", "Caches"]),
            Risk::Safe,
            true,
        ),
        cat(
            "npm_cache",
            "npm Cache",
            "~/.npm/_cacache — re-downloaded on demand.",
            home(&[".npm", "_cacache"]),
            Risk::Safe,
            true,
        ),
        cat(
            "cargo_cache",
            "Cargo Registry Cache",
            "~/.cargo/registry/cache — downloaded crate archives.",
            home(&[".cargo", "registry", "cache"]),
            Risk::Safe,
            true,
        ),
        cat(
            "trash",
            "Trash",
            "~/.Trash — emptying is irreversible.",
            home(&[".Trash"]),
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
            local(&["Temp"]),
            Risk::Safe,
            true,
        ),
        cat(
            "windows_temp",
            "Windows Temp Files",
            "C:\\Windows\\Temp — system scratch files (some may be in use).",
            windows_temp,
            Risk::Safe,
            true,
        ),
        cat(
            "crash_dumps",
            "Crash Dumps",
            "%LOCALAPPDATA%\\CrashDumps — saved crash reports.",
            local(&["CrashDumps"]),
            Risk::Safe,
            true,
        ),
        cat(
            "browser_caches",
            "Browser Caches",
            "Chrome & Edge on-disk caches. Bookmarks/history untouched.",
            browser,
            Risk::Safe,
            true,
        ),
        cat(
            "npm_cache",
            "npm Cache",
            "%LOCALAPPDATA%\\npm-cache — re-downloaded on demand.",
            local(&["npm-cache"]),
            Risk::Safe,
            true,
        ),
        cat(
            "pip_cache",
            "pip Cache",
            "%LOCALAPPDATA%\\pip\\Cache — downloaded Python wheels.",
            local(&["pip", "Cache"]),
            Risk::Safe,
            true,
        ),
        cat(
            "cargo_cache",
            "Cargo Registry Cache",
            "~/.cargo/registry/cache — downloaded crate archives.",
            home(&[".cargo", "registry", "cache"]),
            Risk::Safe,
            true,
        ),
        cat(
            "recycle_bin",
            "Recycle Bin",
            "C:\\$Recycle.Bin — emptying is irreversible.",
            vec![PathBuf::from("C:\\$Recycle.Bin")],
            Risk::Caution,
            false,
        ),
    ]
}

// ---------------------------------------------------------------------------
// Linux / other (lets the app build & run on dev machines too)
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
            user_cache,
            Risk::Safe,
            true,
        ),
        cat(
            "temp_files",
            "Temporary Files",
            "Per-user temp directory.",
            vec![std::env::temp_dir()],
            Risk::Safe,
            true,
        ),
        cat(
            "npm_cache",
            "npm Cache",
            "~/.npm/_cacache — re-downloaded on demand.",
            home(&[".npm", "_cacache"]),
            Risk::Safe,
            true,
        ),
        cat(
            "cargo_cache",
            "Cargo Registry Cache",
            "~/.cargo/registry/cache — downloaded crate archives.",
            home(&[".cargo", "registry", "cache"]),
            Risk::Safe,
            true,
        ),
        cat(
            "trash",
            "Trash",
            "~/.local/share/Trash — emptying is irreversible.",
            home(&[".local", "share", "Trash", "files"]),
            Risk::Caution,
            false,
        ),
    ]
}
