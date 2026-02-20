mod registered_modules;

use anyhow::Result;
use clap::{Parser, Subcommand};
use mimalloc::MiMalloc;
use modkit::bootstrap::{
    AppConfig, dump_effective_modules_config_json, dump_effective_modules_config_yaml,
    host::{build_console_layer, init_logging_unified},
    list_module_names, run_migrate, run_server,
};

use std::path::PathBuf;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

/// `HyperSpot` Server - modular platform for AI services
#[derive(Parser)]
#[command(name = "hyperspot-server")]
#[command(about = "HyperSpot Server - modular platform for AI services")]
#[command(version = "0.1.0")]
#[allow(clippy::struct_excessive_bools)]
struct Cli {
    /// Path to configuration file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Port override for HTTP server (overrides config)
    #[arg(short, long)]
    port: Option<u16>,

    /// Print effective configuration (YAML) and exit
    #[arg(long)]
    print_config: bool,

    /// List all configured module names and exit
    #[arg(long)]
    list_modules: bool,

    /// Dump effective per-module configuration (YAML) and exit
    #[arg(long)]
    dump_modules_config_yaml: bool,

    /// Dump effective per-module configuration (JSON) and exit
    #[arg(long)]
    dump_modules_config_json: bool,

    /// Log verbosity level (-v info, -vv debug, -vvv trace)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the server
    Run,
    /// Validate configuration and exit
    Check,
    /// Run database migrations and exit (for cloud deployments)
    Migrate,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Layered config:
    // 1) defaults -> 2) YAML (if provided) -> 3) env (APP__*) -> 4) CLI overrides
    // Also normalizes + creates server.home_dir.
    let mut config = AppConfig::load_or_default(&cli.config)?;
    config.apply_cli_overrides(cli.verbose);

    // Build OpenTelemetry layer before logging
    // Convert TracingConfig from modkit::bootstrap to modkit's type (they have identical structure)
    #[cfg(feature = "otel")]
    let modkit_tracing_config: Option<modkit::telemetry::TracingConfig> = config
        .tracing
        .as_ref()
        .and_then(|tc| serde_json::to_value(tc).ok())
        .and_then(|v| serde_json::from_value(v).ok());
    #[cfg(feature = "otel")]
    let otel_layer = if let Some(tc) = modkit_tracing_config.as_ref()
        && tc.enabled
    {
        Some(modkit::telemetry::init::init_tracing(tc)?)
    } else {
        None
    };
    #[cfg(not(feature = "otel"))]
    let otel_layer = None;

    // Build tokio-console layer before logging (when feature is enabled)
    let console_layer = build_console_layer(config.tokio_console.as_ref());

    // Initialize logging + otel + tokio-console in one Registry
    init_logging_unified(&config.logging, &config.server.home_dir, otel_layer, console_layer);

    // One-time connectivity probe
    #[cfg(feature = "otel")]
    if let Some(tc) = modkit_tracing_config.as_ref()
        && let Err(e) = modkit::telemetry::init::otel_connectivity_probe(tc)
    {
        tracing::error!(error = %e, "OTLP connectivity probe failed");
    }

    // Smoke test span to confirm traces flow to Jaeger
    tracing::info_span!("startup_check", app = "hyperspot").in_scope(|| {
        tracing::info!("startup span alive - traces should be visible in Jaeger");
    });

    tracing::info!("HyperSpot Server starting");

    // Print config and exit if requested
    if cli.print_config {
        println!("Effective configuration:\n{}", config.to_yaml()?);
        return Ok(());
    }

    // List all configured modules and exit if requested
    if cli.list_modules {
        let modules = list_module_names(&config);
        println!("Configured modules ({}):", modules.len());
        for module in modules {
            println!("  - {module}");
        }
        return Ok(());
    }

    // Dump modules config in YAML format and exit if requested
    if cli.dump_modules_config_yaml {
        let yaml = dump_effective_modules_config_yaml(&config)?;
        println!("{yaml}");
        return Ok(());
    }

    // Dump modules config in JSON format and exit if requested
    if cli.dump_modules_config_json {
        let json = dump_effective_modules_config_json(&config)?;
        println!("{json}");
        return Ok(());
    }

    // Dispatch subcommands (default: run)
    match cli.command.as_ref().unwrap_or(&Commands::Run) {
        Commands::Run => run_server(config).await,
        Commands::Check => check_config(&config),
        Commands::Migrate => run_migrate(config).await,
    }
}

fn check_config(config: &AppConfig) -> Result<()> {
    tracing::info!("Checking configuration...");
    // If load_layered/load_or_default succeeded and home_dir normalized, we're good.
    println!("Configuration is valid");
    println!("{}", config.to_yaml()?);
    Ok(())
}
