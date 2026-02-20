use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use modkit::api::OpenApiRegistry;
use modkit::{Module, ModuleCtx, RestApiCapability};
use tracing::{debug, info};

use crate::config::FileParserConfig;
use crate::domain::service::{FileParserService, ServiceConfig};
use crate::infra::parsers::{
    DocxParser, HtmlParser, ImageParser, PdfParser, PlainTextParser, PptxParser, StubParser,
    XlsxParser,
};

/// Main module struct for file parsing
#[modkit::module(
    name = "file-parser",
    capabilities = [rest]
)]
pub struct FileParserModule {
    service: OnceLock<Arc<FileParserService>>,
}

impl Default for FileParserModule {
    fn default() -> Self {
        Self {
            service: OnceLock::new(),
        }
    }
}

#[async_trait]
impl Module for FileParserModule {
    #[allow(clippy::cast_possible_truncation)]
    async fn init(&self, ctx: &ModuleCtx) -> anyhow::Result<()> {
        const BYTES_IN_MB: u64 = 1024_u64 * 1024;

        info!("Initializing {} module", Self::MODULE_NAME);

        // Load module configuration
        let cfg: FileParserConfig = ctx.config()?;
        debug!(
            "Loaded file-parser config: max_file_size_mb={}",
            cfg.max_file_size_mb
        );

        // Build parser backends
        let parsers: Vec<Arc<dyn crate::domain::parser::FileParserBackend>> = vec![
            Arc::new(PlainTextParser::new()),
            Arc::new(HtmlParser::new()),
            Arc::new(PdfParser::new()),
            Arc::new(DocxParser::new()),
            Arc::new(XlsxParser::new()),
            Arc::new(PptxParser::new()),
            Arc::new(ImageParser::new()),
            Arc::new(StubParser::new()),
        ];

        info!("Registered {} parser backends", parsers.len());

        // allowed_local_base_dir is mandatory - fail fast if missing.
        let raw_base = cfg.allowed_local_base_dir.ok_or_else(|| {
            anyhow::anyhow!(
                "file-parser: 'allowed_local_base_dir' is required but not set. \
                 Add it to your config under modules.file-parser.config."
            )
        })?;

        // Canonicalize at startup so we only do it once.
        let allowed_local_base_dir = raw_base.canonicalize().map_err(|e| {
            anyhow::anyhow!(
                "allowed_local_base_dir '{}' cannot be resolved: {e}",
                raw_base.display()
            )
        })?;
        if !allowed_local_base_dir.is_dir() {
            return Err(anyhow::anyhow!(
                "allowed_local_base_dir '{}' is not a directory",
                allowed_local_base_dir.display()
            ));
        }
        info!(
            allowed_local_base_dir = %allowed_local_base_dir.display(),
            "Local file parsing restricted to base directory"
        );

        // Create service config from module config
        let service_config = ServiceConfig {
            max_file_size_bytes: usize::try_from(cfg.max_file_size_mb * BYTES_IN_MB)
                .unwrap_or(usize::MAX),
            allowed_local_base_dir,
        };

        // Create file parser service
        let file_parser_service = Arc::new(FileParserService::new(parsers, service_config));

        // Store service for REST usage
        self.service
            .set(file_parser_service)
            .map_err(|_| anyhow::anyhow!("{} module already initialized", Self::MODULE_NAME))?;

        info!("{} module initialized successfully", Self::MODULE_NAME);
        Ok(())
    }
}

impl RestApiCapability for FileParserModule {
    fn register_rest(
        &self,
        _ctx: &ModuleCtx,
        router: axum::Router,
        openapi: &dyn OpenApiRegistry,
    ) -> anyhow::Result<axum::Router> {
        info!("Registering file-parser REST routes");

        let service = self
            .service
            .get()
            .ok_or_else(|| anyhow::anyhow!("Service not initialized"))?
            .clone();

        let router = crate::api::rest::routes::register_routes(router, openapi, service);

        info!("File parser REST routes registered successfully");
        Ok(router)
    }
}
