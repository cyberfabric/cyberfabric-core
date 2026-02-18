//! GTS Documentation Validator (DE0903)
//!
//! This tool validates GTS (Global Type System) identifiers in documentation files
//! (.md, .json, .yaml), complementing the Rust-based DE0901 lint that validates
//! GTS identifiers in source code.
//!
//! # Usage
//!
//! ```bash
//! # Validate docs with vendor filter
//! gts-docs-validator --vendor x docs modules libs examples
//!
//! # With exclusions
//! gts-docs-validator --vendor x --exclude "target/*" --exclude "docs/api/*" .
//!
//! # JSON output
//! gts-docs-validator --vendor x --json docs
//! ```

// CLI tools are expected to print to stdout/stderr
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::exit,
    clippy::expect_used
)]

mod error;
mod normalize;
mod scan_json;
mod scan_markdown;
mod scan_yaml;
mod scanner;
mod validator;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use colored::Colorize;

use crate::error::DocValidationError;
use crate::scanner::{find_files, scan_file};

/// GTS Documentation Validator (DE0903)
///
/// Validates GTS identifiers in .md, .json, and .yaml files.
/// Ensures all GTS IDs follow the correct format and optionally validates
/// that they use a specific vendor.
#[derive(Parser, Debug)]
#[command(name = "gts-docs-validator")]
#[command(author, version, about, long_about = None)]
#[allow(clippy::struct_excessive_bools)]
struct Cli {
    /// Paths to scan (files or directories)
    /// Defaults to: docs, modules, libs, examples
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,

    /// Expected vendor for all GTS IDs (validates vendor matches)
    /// Example: --vendor x ensures all IDs use vendor "x"
    ///
    /// Note: Example vendors are always tolerated: acme, globex, example, demo, test, sample, tutorial
    #[arg(long)]
    vendor: Option<String>,

    /// Exclude patterns (can be specified multiple times)
    /// Supports glob patterns. Example: --exclude "target/*" --exclude "docs/api/*"
    #[arg(long, short = 'e', action = clap::ArgAction::Append)]
    exclude: Vec<String>,

    /// Output results as JSON
    #[arg(long)]
    json: bool,

    /// Show verbose output including file scanning progress
    #[arg(long, short = 'v')]
    verbose: bool,

    /// Maximum file size in bytes (default: 10 MB)
    #[arg(long, default_value = "10485760")]
    max_file_size: u64,

    /// Scan JSON/YAML object keys for GTS identifiers (default: off)
    #[arg(long)]
    scan_keys: bool,

    /// Strict mode: require all GTS identifiers to be perfectly formatted
    /// (default: off, uses stricter discovery regex that catches malformed IDs)
    ///
    /// When enabled, uses permissive regex to catch and report ALL malformed GTS IDs.
    /// When disabled (default), only validates well-formed patterns (fewer false positives).
    #[arg(long)]
    strict: bool,
}

/// Default directories to scan if none specified
const DEFAULT_SCAN_DIRS: &[&str] = &["docs", "modules", "libs", "examples"];

fn main() -> ExitCode {
    let cli = Cli::parse();

    // Determine paths to scan
    let paths: Vec<PathBuf> = if cli.paths.is_empty() {
        DEFAULT_SCAN_DIRS
            .iter()
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .collect()
    } else {
        // Validate explicitly provided paths exist
        for path in &cli.paths {
            if !path.exists() {
                eprintln!("Error: Path does not exist: {}", path.display());
                return ExitCode::FAILURE;
            }
        }
        cli.paths.clone()
    };

    if paths.is_empty() {
        eprintln!("No existing paths to scan. Provide paths explicitly.");
        return ExitCode::FAILURE;
    }

    if cli.verbose {
        let path_list: Vec<_> = paths.iter().map(|p| p.display().to_string()).collect();
        eprintln!("Scanning paths: {}", path_list.join(", "));
        if let Some(ref vendor) = cli.vendor {
            eprintln!("Expected vendor: {vendor}");
        }
    }

    // Find all files to scan
    let files = find_files(&paths, &cli.exclude, cli.verbose);

    if cli.verbose {
        eprintln!("Found {} files to scan", files.len());
    }

    if files.is_empty() {
        eprintln!("No files found to scan. Check paths and exclusion patterns.");
        return ExitCode::FAILURE;
    }

    // Scan all files and collect errors
    let mut errors: Vec<DocValidationError> = Vec::new();

    for file_path in &files {
        let file_errors = scan_file(
            file_path,
            cli.vendor.as_deref(),
            cli.verbose,
            cli.max_file_size,
            cli.scan_keys,
            cli.strict,
        );
        errors.extend(file_errors);
    }

    // Output results
    if cli.json {
        print_json_results(&errors, files.len());
    } else {
        print_results(&errors, files.len(), cli.verbose);
    }

    if errors.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn print_json_results(errors: &[DocValidationError], files_scanned: usize) {
    let output = serde_json::json!({
        "files_scanned": files_scanned,
        "errors_count": errors.len(),
        "ok": errors.is_empty(),
        "errors": errors
    });
    let json = serde_json::to_string_pretty(&output).expect("Failed to serialize results");
    println!("{json}");
}

fn print_results(errors: &[DocValidationError], files_scanned: usize, verbose: bool) {
    println!();
    println!("{}", "=".repeat(80));
    println!("  {}", "GTS DOCUMENTATION VALIDATOR (DE0903)".bold());
    println!("{}", "=".repeat(80));
    println!();
    println!("  Files scanned: {files_scanned}");
    println!("  Errors found:  {}", errors.len());
    println!();

    if !errors.is_empty() {
        println!("{}", "-".repeat(80));
        println!("  {}", "ERRORS".red().bold());
        println!("{}", "-".repeat(80));

        // Print errors
        for error in errors {
            println!("{}", error.format_human_readable().red());

            if verbose && !error.context.is_empty() {
                println!("  Context: {}", error.context.dimmed());
            }
        }
        println!();
    }

    println!("{}", "=".repeat(80));
    if errors.is_empty() {
        println!(
            "{}",
            format!("\u{2713} All {files_scanned} files passed validation").green()
        );
    } else {
        println!(
            "{}",
            format!("\u{2717} {} invalid GTS identifiers found", errors.len()).red()
        );
        println!();
        println!("  To fix:");
        println!("    - Schema IDs must end with ~ (e.g., gts.x.core.type.v1~)");
        println!("    - Each segment needs 5 parts: vendor.package.namespace.type.version");
        println!("    - No hyphens allowed, use underscores");
        println!("    - Wildcards (*) only in filter/pattern contexts");
        if errors.iter().any(|e| e.error.contains("Vendor mismatch")) {
            println!("    - Ensure all GTS IDs use the expected vendor");
        }
    }
    println!("{}", "=".repeat(80));
}
