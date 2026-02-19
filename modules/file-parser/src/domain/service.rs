use std::path::{Path, PathBuf};
use std::sync::Arc;

use bytes::Bytes;
use modkit_macros::domain_model;
use tracing::{debug, info, instrument, warn};

use crate::domain::error::DomainError;
use crate::domain::ir::ParsedDocument;
use crate::domain::parser::FileParserBackend;

/// Mapping of file extensions to MIME types
/// Format: `(extension, mime_type)`
const EXTENSION_MIME_MAPPINGS: &[(&str, &str)] = &[
    ("pdf", "application/pdf"),
    ("html", "text/html"),
    ("htm", "text/html"),
    (
        "docx",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    ),
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("webp", "image/webp"),
    ("gif", "image/gif"),
];

/// File parser service that routes to appropriate backends
#[domain_model]
#[derive(Clone)]
pub struct FileParserService {
    parsers: Vec<Arc<dyn FileParserBackend>>,
    config: ServiceConfig,
}

/// Configuration for the file parser service
#[domain_model]
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub max_file_size_bytes: usize,
    /// Canonicalized base directory for local file access. Only paths that
    /// start with this prefix are allowed by `parse_local`.
    pub allowed_local_base_dir: PathBuf,
}

/// Information about available parsers
#[domain_model]
#[derive(Debug, Clone)]
pub struct FileParserInfo {
    pub supported_extensions: std::collections::HashMap<String, Vec<String>>,
}

impl FileParserService {
    /// Create a new service with the given parsers.
    #[must_use]
    pub fn new(parsers: Vec<Arc<dyn FileParserBackend>>, config: ServiceConfig) -> Self {
        Self { parsers, config }
    }

    /// Get information about available parsers
    #[instrument(skip(self))]
    pub fn info(&self) -> FileParserInfo {
        debug!("Getting parser info");

        let mut supported_extensions = std::collections::HashMap::new();

        for parser in &self.parsers {
            let id = parser.id();
            let extensions: Vec<String> = parser
                .supported_extensions()
                .iter()
                .map(ToString::to_string)
                .collect();
            supported_extensions.insert(id.to_owned(), extensions);
        }

        FileParserInfo {
            supported_extensions,
        }
    }

    /// Parse a file from a local path.
    ///
    /// The requested path is validated before any file-system access:
    /// 1. `..` path components are rejected outright.
    /// 2. The path is canonicalized (resolving symlinks).
    /// 3. The canonical path must fall under `allowed_local_base_dir`.
    #[instrument(skip(self), fields(path = %path.display()))]
    pub async fn parse_local(&self, path: &Path) -> Result<ParsedDocument, DomainError> {
        info!("Parsing file from local path");

        // --- Path traversal protection ---
        // Order matters: validate before any filesystem probe so that
        // unauthorised paths never leak existence information.
        Self::validate_local_path(path)?;

        // Canonicalize to resolve symlinks. This also serves as the
        // existence check - canonicalize fails with NotFound on missing paths.
        let canonical = path.canonicalize().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                DomainError::file_not_found(path.display().to_string())
            } else {
                DomainError::io_error(format!(
                    "Cannot canonicalize path '{}': {e}",
                    path.display()
                ))
            }
        })?;

        // Enforce base directory (after symlink resolution).
        // This runs before any content is read, so an attacker probing
        // paths outside the base dir gets a uniform 403 regardless of
        // whether the path exists.
        if !canonical.starts_with(&self.config.allowed_local_base_dir) {
            warn!(
                requested = %path.display(),
                canonical = %canonical.display(),
                base_dir = %self.config.allowed_local_base_dir.display(),
                "Path traversal blocked: canonical path outside allowed base directory"
            );
            return Err(DomainError::path_traversal_blocked(format!(
                "Access denied: '{}' is outside the allowed base directory",
                path.display()
            )));
        }

        // Extract extension
        let extension = canonical
            .extension()
            .and_then(|s| s.to_str())
            .ok_or_else(|| DomainError::unsupported_file_type("no extension"))?;

        // Find parser
        let parser = self
            .find_parser_by_extension(extension)
            .ok_or_else(|| DomainError::no_parser_available(extension))?;

        // Parse the file
        let document = parser.parse_local_path(&canonical).await.map_err(|e| {
            tracing::error!(?e, "FileParserService: parse_local failed");
            e
        })?;

        debug!("Successfully parsed file from local path");
        Ok(document)
    }

    /// Reject paths that contain `..` components (before any file-system call).
    fn validate_local_path(path: &Path) -> Result<(), DomainError> {
        for component in path.components() {
            if matches!(component, std::path::Component::ParentDir) {
                warn!(
                    path = %path.display(),
                    "Path traversal blocked: '..' component detected"
                );
                return Err(DomainError::path_traversal_blocked(format!(
                    "Access denied: path '{}' contains '..' traversal component",
                    path.display()
                )));
            }
        }
        Ok(())
    }

    /// Parse a file from bytes
    #[instrument(
        skip(self, bytes),
        fields(filename_hint = ?filename_hint, content_type = ?content_type, size = bytes.len())
    )]
    pub async fn parse_bytes(
        &self,
        filename_hint: Option<&str>,
        content_type: Option<&str>,
        bytes: Bytes,
    ) -> Result<ParsedDocument, DomainError> {
        info!("Parsing uploaded file");

        // Check file size
        if bytes.len() > self.config.max_file_size_bytes {
            return Err(DomainError::invalid_request(format!(
                "File size {} exceeds maximum of {} bytes",
                bytes.len(),
                self.config.max_file_size_bytes
            )));
        }

        // Determine extension by priority:
        // 1. From filename (if provided and has extension)
        // 2. From Content-Type (if provided and recognized)
        // 3. Error if both fail
        let extension_from_name = filename_hint
            .and_then(|name| Path::new(name).extension())
            .and_then(|s| s.to_str())
            .map(ToString::to_string);

        let extension = if let Some(ext) = extension_from_name {
            // Priority 1: Use extension from filename
            ext
        } else if let Some(ct) = content_type {
            // Priority 2: Try to infer from Content-Type
            if let Some(ext) = Self::extension_from_content_type(ct) {
                ext
            } else {
                return Err(DomainError::unsupported_file_type(
                    "no extension and unknown content-type",
                ));
            }
        } else {
            // Both failed
            return Err(DomainError::unsupported_file_type(
                "no extension and no content-type",
            ));
        };

        // Find parser
        let parser = self
            .find_parser_by_extension(&extension)
            .ok_or_else(|| DomainError::no_parser_available(&extension))?;

        // Parse the file
        let document = parser
            .parse_bytes(filename_hint, content_type, bytes)
            .await
            .map_err(|e| {
                tracing::error!(?e, "FileParserService: parse_bytes failed");
                e
            })?;

        debug!("Successfully parsed uploaded file");
        Ok(document)
    }

    /// Extract file extension from Content-Type header
    #[must_use]
    pub fn extension_from_content_type(ct: &str) -> Option<String> {
        let mime: mime::Mime = ct.parse().ok()?;
        let essence = mime.essence_str();

        // Special case: application/xhtml+xml maps to html
        if essence == "application/xhtml+xml" {
            return Some("html".to_owned());
        }

        // Find extension by matching MIME type
        EXTENSION_MIME_MAPPINGS
            .iter()
            .find(|(_, mime_type)| *mime_type == essence)
            .map(|(ext, _)| (*ext).to_owned())
    }

    /// Find a parser by file extension
    fn find_parser_by_extension(&self, ext: &str) -> Option<Arc<dyn FileParserBackend>> {
        let ext_lower = ext.to_lowercase();
        self.parsers
            .iter()
            .find(|p| {
                p.supported_extensions()
                    .iter()
                    .any(|e| e.to_lowercase() == ext_lower)
            })
            .cloned()
    }
}
