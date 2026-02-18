//! File scanning functionality for GTS documentation validation

use std::path::{Path, PathBuf};

use glob::Pattern;
use walkdir::WalkDir;

use crate::error::DocValidationError;
use crate::scan_json::scan_json_file;
use crate::scan_markdown::scan_markdown_file;
use crate::scan_yaml::scan_yaml_file;

/// File patterns to scan
const FILE_PATTERNS: &[&str] = &["*.md", "*.json", "*.yaml", "*.yml"];

/// Directories to skip
const SKIP_DIRS: &[&str] = &["target", "node_modules", ".git", "vendor", ".gts-spec"];

/// Files to skip (relative paths)
const SKIP_FILES: &[&str] = &["docs/api/api.json"];

/// Check if a path matches any of the exclude patterns
fn matches_exclude(path: &Path, exclude_patterns: &[Pattern]) -> bool {
    let path_str = path.to_string_lossy();
    for pattern in exclude_patterns {
        if pattern.matches(&path_str)
            || path
                .file_name()
                .is_some_and(|name| pattern.matches(&name.to_string_lossy()))
        {
            return true;
        }
    }
    false
}

/// Check if path contains any skip directories
fn in_skip_dir(path: &Path) -> bool {
    for component in path.components() {
        if let std::path::Component::Normal(name) = component
            && SKIP_DIRS.iter().any(|skip| name.to_string_lossy() == *skip)
        {
            return true;
        }
    }
    false
}

/// Check if file matches any of the file patterns
fn matches_file_pattern(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();
        let with_dot = format!("*.{ext_str}");
        FILE_PATTERNS.iter().any(|p| *p == with_dot)
    } else {
        false
    }
}

/// Find all files to scan in the given paths
#[must_use]
pub fn find_files(paths: &[PathBuf], exclude: &[String], verbose: bool) -> Vec<PathBuf> {
    let mut files = Vec::new();

    // Parse exclude patterns
    let exclude_patterns: Vec<Pattern> = exclude
        .iter()
        .filter_map(|p| match Pattern::new(p) {
            Ok(pat) => Some(pat),
            Err(e) => {
                if verbose {
                    eprintln!("Warning: Invalid exclude pattern '{p}': {e}");
                }
                None
            }
        })
        .collect();

    for path in paths {
        if path.is_file() {
            if matches_file_pattern(path) && !matches_exclude(path, &exclude_patterns) {
                files.push(path.clone());
            }
        } else if path.is_dir() {
            for entry in WalkDir::new(path)
                .follow_links(true)
                .into_iter()
                .filter_map(Result::ok)
            {
                let file_path = entry.path();

                // Skip directories in skip list
                if in_skip_dir(file_path) {
                    continue;
                }

                // Only process files
                if !file_path.is_file() {
                    continue;
                }

                // Check file pattern
                if !matches_file_pattern(file_path) {
                    continue;
                }

                // Check exclude patterns
                if matches_exclude(file_path, &exclude_patterns) {
                    if verbose {
                        eprintln!("  Excluding: {}", file_path.display());
                    }
                    continue;
                }

                // Check against skip files
                let rel_path = file_path.to_string_lossy();
                if SKIP_FILES.iter().any(|skip| rel_path.contains(skip)) {
                    if verbose {
                        eprintln!("  Skipping: {}", file_path.display());
                    }
                    continue;
                }

                files.push(file_path.to_path_buf());
            }
        }
    }

    files.sort();
    files.dedup();
    files
}

/// Scan a single file for GTS identifiers
pub fn scan_file(
    path: &Path,
    vendor: Option<&str>,
    verbose: bool,
    max_file_size: u64,
    scan_keys: bool,
    strict: bool,
) -> Vec<DocValidationError> {
    if verbose {
        eprintln!("  Scanning: {}", path.display());
    }

    match path.extension().and_then(|e| e.to_str()) {
        Some("md") => scan_markdown_file(path, vendor, verbose, max_file_size, strict),
        Some("json") => scan_json_file(path, vendor, verbose, max_file_size, scan_keys),
        Some("yaml" | "yml") => scan_yaml_file(path, vendor, verbose, max_file_size, scan_keys),
        _ => {
            if verbose {
                eprintln!("  Skipping {} (unsupported extension)", path.display());
            }
            vec![]
        }
    }
}
