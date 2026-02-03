//! Config command - View and manage configuration
//!
//! Provides commands for viewing and modifying Prism configuration:
//! - List all configuration with sources
//! - Get specific configuration values
//! - Set configuration values (local or global)

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Subcommand;
use codeprysm_config::{ConfigLoader, PrismConfig};
use serde::Serialize;

use super::resolve_workspace;
use crate::GlobalOptions;

/// Config management commands
#[derive(Subcommand, Debug)]
pub enum ConfigCommand {
    /// List all configuration values with their sources
    List(ListArgs),

    /// Get a specific configuration value
    Get(GetArgs),

    /// Set a configuration value
    Set(SetArgs),

    /// Show configuration file paths
    Path(PathArgs),

    /// Initialize configuration directory and files
    Init(InitArgs),
}

/// Arguments for the list command
#[derive(clap::Args, Debug)]
pub struct ListArgs {
    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Show only effective values (hide sources)
    #[arg(long)]
    effective: bool,
}

/// Arguments for the get command
#[derive(clap::Args, Debug)]
pub struct GetArgs {
    /// Configuration key (e.g., "backend.qdrant.url")
    key: String,

    /// Output as JSON
    #[arg(long)]
    json: bool,
}

/// Arguments for the set command
#[derive(clap::Args, Debug)]
pub struct SetArgs {
    /// Configuration key (e.g., "backend.qdrant.url")
    key: String,

    /// Value to set
    value: String,

    /// Set in global config (~/.codeprysm/config.toml) instead of local
    #[arg(long)]
    global: bool,
}

/// Arguments for the path command
#[derive(clap::Args, Debug)]
pub struct PathArgs {
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

/// Arguments for the init command
#[derive(clap::Args, Debug)]
pub struct InitArgs {
    /// Path to initialize (defaults to current directory)
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Force overwrite existing configuration
    #[arg(long, short = 'f')]
    force: bool,

    /// Interactive mode - prompt for configuration options
    #[arg(long, short = 'i')]
    interactive: bool,

    /// Configuration template to use
    #[arg(long, value_enum, default_value = "basic")]
    template: ConfigTemplate,
}

/// Configuration template types
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ConfigTemplate {
    /// Basic template with common settings
    Basic,
    /// Monorepo template with include_patterns examples
    Monorepo,
    /// Minimal template (empty sections)
    Minimal,
}

/// Configuration value with source information
#[derive(Debug, Clone, Serialize)]
pub struct ConfigValue {
    /// Configuration key
    pub key: String,
    /// Current value
    pub value: serde_json::Value,
    /// Source of this value (default, global, local, cli)
    pub source: String,
}

/// Configuration paths
#[derive(Debug, Clone, Serialize)]
pub struct ConfigPaths {
    /// Global config file path
    pub global: Option<PathBuf>,
    /// Local config file path
    pub local: PathBuf,
    /// Whether global config exists
    pub global_exists: bool,
    /// Whether local config exists
    pub local_exists: bool,
}

/// Execute the config command
pub async fn execute(cmd: ConfigCommand, global: GlobalOptions) -> Result<()> {
    match cmd {
        ConfigCommand::List(args) => execute_list(args, global).await,
        ConfigCommand::Get(args) => execute_get(args, global).await,
        ConfigCommand::Set(args) => execute_set(args, global).await,
        ConfigCommand::Path(args) => execute_path(args, global).await,
        ConfigCommand::Init(args) => execute_init(args, global).await,
    }
}

async fn execute_list(args: ListArgs, global: GlobalOptions) -> Result<()> {
    let workspace_path = resolve_workspace(&global).await?;
    let mut loader = ConfigLoader::new();

    // Load configs from different sources
    let default_config = PrismConfig::default();
    let global_config = loader.load_global()?.unwrap_or_default();
    let local_config = loader.load_local(&workspace_path)?.unwrap_or_default();
    let effective = loader.load(&workspace_path, None)?;

    if args.json {
        if args.effective {
            println!("{}", serde_json::to_string_pretty(&effective)?);
        } else {
            let values = collect_config_values(&default_config, &global_config, &local_config);
            println!("{}", serde_json::to_string_pretty(&values)?);
        }
    } else {
        print_config_list(
            &default_config,
            &global_config,
            &local_config,
            &loader,
            &workspace_path,
        );
    }

    Ok(())
}

async fn execute_get(args: GetArgs, global: GlobalOptions) -> Result<()> {
    let workspace_path = resolve_workspace(&global).await?;
    let mut loader = ConfigLoader::new();
    let config = loader.load(&workspace_path, None)?;

    let value = get_config_value(&config, &args.key)
        .ok_or_else(|| anyhow::anyhow!("Unknown configuration key: {}", args.key))?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        match value {
            serde_json::Value::String(s) => println!("{}", s),
            serde_json::Value::Bool(b) => println!("{}", b),
            serde_json::Value::Number(n) => println!("{}", n),
            serde_json::Value::Null => println!("null"),
            other => println!("{}", serde_json::to_string_pretty(&other)?),
        }
    }

    Ok(())
}

async fn execute_set(args: SetArgs, global: GlobalOptions) -> Result<()> {
    let workspace_path = resolve_workspace(&global).await?;
    let mut loader = ConfigLoader::new();

    // Load existing config
    let mut config = if args.global {
        loader.load_global().ok().flatten().unwrap_or_default()
    } else {
        loader
            .load_local(&workspace_path)
            .ok()
            .flatten()
            .unwrap_or_default()
    };

    // Set the value
    set_config_value(&mut config, &args.key, &args.value)
        .context(format!("Failed to set configuration key: {}", args.key))?;

    // Save config
    if args.global {
        loader.save_global(&config)?;
        println!("Set {} = {} in global config", args.key, args.value);
    } else {
        loader.save_local(&workspace_path, &config)?;
        println!("Set {} = {} in local config", args.key, args.value);
    }

    Ok(())
}

async fn execute_path(args: PathArgs, global: GlobalOptions) -> Result<()> {
    let workspace_path = resolve_workspace(&global).await?;
    let loader = ConfigLoader::new();

    let global_path = loader.global_config_path();
    let local_path = loader.local_config_path(&workspace_path);

    let paths = ConfigPaths {
        global: global_path.clone(),
        local: local_path.clone(),
        global_exists: global_path.as_ref().map(|p| p.exists()).unwrap_or(false),
        local_exists: local_path.exists(),
    };

    if args.json {
        println!("{}", serde_json::to_string_pretty(&paths)?);
    } else {
        println!("Configuration Paths");
        println!("===================\n");

        if let Some(ref gp) = paths.global {
            let status = if paths.global_exists {
                "exists"
            } else {
                "not found"
            };
            println!("Global: {} ({})", gp.display(), status);
        } else {
            println!("Global: not available (no home directory)");
        }

        let status = if paths.local_exists {
            "exists"
        } else {
            "not found"
        };
        println!("Local:  {} ({})", paths.local.display(), status);
    }

    Ok(())
}

/// Execute the config init command
async fn execute_init(args: InitArgs, global: GlobalOptions) -> Result<()> {
    let quiet = global.quiet;

    // Resolve workspace path
    let workspace_path = if args.path.is_absolute() {
        args.path.clone()
    } else {
        std::env::current_dir()?.join(&args.path)
    };

    let workspace_path = workspace_path
        .canonicalize()
        .context("Failed to resolve workspace path")?;

    // Determine .codeprysm directory location
    let temp_config = PrismConfig::default();
    let prism_dir = temp_config.prism_dir(&workspace_path);
    let config_path = prism_dir.join("config.toml");

    // Check if config already exists
    if config_path.exists() && !args.force {
        anyhow::bail!(
            "Configuration already exists at {}\nUse --force to overwrite",
            config_path.display()
        );
    }

    // Backup existing config if force overwriting
    if config_path.exists() && args.force {
        let backup_path = prism_dir.join("config.toml.backup");
        std::fs::copy(&config_path, &backup_path)
            .context("Failed to create backup")?;
        if !quiet {
            println!("📦 Backed up existing config to {}", backup_path.display());
        }
    }

    // Create .codeprysm directory if needed
    if !prism_dir.exists() {
        std::fs::create_dir_all(&prism_dir)
            .context("Failed to create .codeprysm directory")?;
        if !quiet {
            println!("📁 Created {}", prism_dir.display());
        }
    }

    // Generate configuration content
    let config_content = if args.interactive {
        generate_interactive_config(&workspace_path, quiet)?
    } else {
        generate_template_config(args.template, &workspace_path)?
    };

    // Write configuration file
    std::fs::write(&config_path, config_content)
        .context("Failed to write configuration file")?;

    if !quiet {
        println!("✅ Created {}", config_path.display());
        println!();
        print_next_steps(&config_path, args.template);
    }

    Ok(())
}

/// Generate configuration based on template type
fn generate_template_config(template: ConfigTemplate, workspace: &std::path::Path) -> Result<String> {
    match template {
        ConfigTemplate::Basic => Ok(generate_basic_template(workspace)),
        ConfigTemplate::Monorepo => Ok(generate_monorepo_template(workspace)),
        ConfigTemplate::Minimal => Ok(generate_minimal_template()),
    }
}

/// Generate basic configuration template
fn generate_basic_template(workspace: &std::path::Path) -> String {
    let workspace_name = workspace
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("workspace");

    format!(r#"# CodePrysm Local Configuration
# Workspace: {}
#
# This file overrides global settings (~/.codeprysm/config.toml) for this workspace.
# Edit this file before running 'codeprysm init' to customize analysis.

[analysis]
# Maximum file size to analyze (in KB)
max_file_size_kb = 1024

# File patterns to exclude during analysis
# These are applied in addition to .gitignore rules
exclude_patterns = [
    "**/node_modules/**",
    "**/target/**",
    "**/.git/**",
    "**/vendor/**",
    "**/__pycache__/**",
    "**/dist/**",
    "**/build/**",
]

# File patterns to include during analysis
# If specified, ONLY files matching these patterns will be analyzed
# Useful for large monorepos - analyze specific packages/directories only
#
# Examples:
#   include_patterns = ["packages/frontend/**", "packages/backend/**", "shared/**"]
#   include_patterns = ["src/**", "lib/**"]
#
include_patterns = []

# Detect and index components (package.json, Cargo.toml, etc.)
detect_components = true

# Parallel processing threads (0 = auto-detect based on CPU cores)
parallelism = 0

[storage]
# Directory for CodePrysm data (relative to workspace root)
prism_dir = ".codeprysm"

# Enable compression for graph storage
compression = true

# Maximum partition size in MB (affects memory usage during indexing)
max_partition_size_mb = 50

# [backend.qdrant]
# Uncomment to customize Qdrant settings for this workspace
# url = "http://localhost:6334"
# collection_prefix = "codeprysm"

# [embedding]
# Uncomment to use a different embedding provider for this workspace
# provider = "local"  # Options: local, azure-ml, openai, onnx
"#, workspace_name)
}

/// Generate monorepo-focused configuration template
fn generate_monorepo_template(workspace: &std::path::Path) -> String {
    let workspace_name = workspace
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("workspace");

    format!(r#"# CodePrysm Local Configuration - Monorepo Template
# Workspace: {}
#
# This template is optimized for large monorepos with multiple packages/projects.

[analysis]
# ⚠️ IMPORTANT: Configure include_patterns for large monorepos
#
# For monorepos with many packages, specify which directories to analyze.
# This dramatically reduces analysis time and focuses search on relevant code.
#
# Example: Analyze only frontend and backend packages + shared utilities
include_patterns = [
    "packages/frontend/**",
    "packages/backend/**",
    "packages/shared/**",
    # Add more packages as needed
]

# Exclude patterns (applied after include_patterns)
exclude_patterns = [
    "**/node_modules/**",
    "**/target/**",
    "**/.git/**",
    "**/dist/**",
    "**/build/**",
    "**/__tests__/**",
    "**/*.test.ts",
    "**/*.spec.ts",
]

# Enable component detection to understand package dependencies
detect_components = true

# Increase parallelism for large repos (0 = auto)
parallelism = 0

[storage]
prism_dir = ".codeprysm"
compression = true
max_partition_size_mb = 100  # Larger partitions for monorepos
"#, workspace_name)
}

/// Generate minimal configuration template
fn generate_minimal_template() -> String {
    r#"# CodePrysm Local Configuration - Minimal Template
# Only essential settings are included.

[analysis]
include_patterns = []
exclude_patterns = []

[storage]
prism_dir = ".codeprysm"
"#.to_string()
}

/// Generate configuration interactively by asking user questions
fn generate_interactive_config(workspace: &std::path::Path, quiet: bool) -> Result<String> {
    use std::io::{self, Write};

    if quiet {
        // Can't do interactive mode in quiet mode
        return Ok(generate_basic_template(workspace));
    }

    println!("🔧 Interactive Configuration Setup");
    println!("====================================\n");

    // Question 1: Monorepo?
    print!("Is this a monorepo with multiple packages/projects? (y/N): ");
    io::stdout().flush()?;
    let mut is_monorepo = String::new();
    io::stdin().read_line(&mut is_monorepo)?;
    let is_monorepo = is_monorepo.trim().eq_ignore_ascii_case("y");

    let mut include_patterns = Vec::new();

    if is_monorepo {
        println!("\nGreat! Let's configure which packages to analyze.");
        println!("You can add multiple patterns (one per line). Press Enter on empty line to finish.\n");

        loop {
            print!("Include pattern (e.g., 'packages/frontend/**'): ");
            io::stdout().flush()?;
            let mut pattern = String::new();
            io::stdin().read_line(&mut pattern)?;
            let pattern = pattern.trim();

            if pattern.is_empty() {
                break;
            }

            include_patterns.push(pattern.to_string());
            println!("  ✓ Added: {}", pattern);
        }

        if include_patterns.is_empty() {
            println!("\n⚠️  No patterns specified. Will analyze entire repository.");
            println!("   (You can edit config.toml later to add patterns)\n");
        }
    }

    // Question 2: Component detection?
    print!("\nEnable component detection (Cargo.toml, package.json, etc.)? (Y/n): ");
    io::stdout().flush()?;
    let mut detect_components = String::new();
    io::stdin().read_line(&mut detect_components)?;
    let detect_components = !detect_components.trim().eq_ignore_ascii_case("n");

    // Generate config based on answers
    let workspace_name = workspace
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("workspace");

    let include_patterns_str = if include_patterns.is_empty() {
        "include_patterns = []".to_string()
    } else {
        let patterns = include_patterns
            .iter()
            .map(|p| format!("    \"{}\",", p))
            .collect::<Vec<_>>()
            .join("\n");
        format!("include_patterns = [\n{}\n]", patterns)
    };

    Ok(format!(r#"# CodePrysm Local Configuration
# Workspace: {}
# Generated via interactive mode

[analysis]
{}

exclude_patterns = [
    "**/node_modules/**",
    "**/target/**",
    "**/.git/**",
    "**/vendor/**",
    "**/__pycache__/**",
    "**/dist/**",
    "**/build/**",
]

detect_components = {}

[storage]
prism_dir = ".codeprysm"
compression = true
"#, workspace_name, include_patterns_str, detect_components))
}

/// Print helpful next steps after config creation
fn print_next_steps(config_path: &std::path::Path, template: ConfigTemplate) {
    println!("📝 Configuration file created!");
    println!();

    match template {
        ConfigTemplate::Monorepo => {
            println!("Next steps:");
            println!("  1. Edit {} to configure include_patterns", config_path.display());
            println!("     (Specify which packages/directories to analyze)");
            println!("  2. Run: codeprysm init");
            println!();
            println!("💡 Tip: For monorepos, include_patterns dramatically speeds up initialization");
        }
        ConfigTemplate::Basic => {
            println!("Next steps:");
            println!("  1. (Optional) Edit {} to customize settings", config_path.display());
            println!("  2. Run: codeprysm init");
            println!();
            println!("💡 Tip: For large repositories, consider using include_patterns");
        }
        ConfigTemplate::Minimal => {
            println!("Next steps:");
            println!("  1. Edit {} to add configuration", config_path.display());
            println!("  2. Run: codeprysm init");
        }
    }
}

/// Get a configuration value by key path
fn get_config_value(config: &PrismConfig, key: &str) -> Option<serde_json::Value> {
    let json = serde_json::to_value(config).ok()?;
    let parts: Vec<&str> = key.split('.').collect();

    let mut current = &json;
    for part in parts {
        match current.get(part) {
            Some(v) => current = v,
            None => return None,
        }
    }

    Some(current.clone())
}

/// Set a configuration value by key path
fn set_config_value(config: &mut PrismConfig, key: &str, value: &str) -> Result<()> {
    match key {
        // Storage
        "storage.prism_dir" => config.storage.prism_dir = PathBuf::from(value),
        "storage.compression" => config.storage.compression = value.parse()?,
        "storage.max_partition_size_mb" => config.storage.max_partition_size_mb = value.parse()?,

        // Backend
        "backend.qdrant.url" => config.backend.qdrant.url = value.to_string(),
        "backend.qdrant.api_key" => config.backend.qdrant.api_key = Some(value.to_string()),
        "backend.qdrant.collection_prefix" => {
            config.backend.qdrant.collection_prefix = value.to_string()
        }
        "backend.qdrant.hnsw_enabled" => config.backend.qdrant.hnsw_enabled = value.parse()?,

        // Analysis
        "analysis.max_file_size_kb" => config.analysis.max_file_size_kb = value.parse()?,
        "analysis.detect_components" => config.analysis.detect_components = value.parse()?,
        "analysis.parallelism" => config.analysis.parallelism = value.parse()?,

        // Workspace
        "workspace.cross_workspace_search" => {
            config.workspace.cross_workspace_search = value.parse()?
        }

        // Logging
        "logging.level" => config.logging.level = value.to_string(),

        _ => anyhow::bail!("Unknown or read-only configuration key: {}", key),
    }

    Ok(())
}

/// Collect configuration values with source information
fn collect_config_values(
    default: &PrismConfig,
    global: &PrismConfig,
    local: &PrismConfig,
) -> Vec<ConfigValue> {
    let mut values = Vec::new();

    // Convert to JSON for comparison
    let default_json = serde_json::to_value(default).unwrap();
    let global_json = serde_json::to_value(global).unwrap();
    let local_json = serde_json::to_value(local).unwrap();

    // Flatten and collect
    flatten_config("", &local_json, &global_json, &default_json, &mut values);

    values
}

/// Recursively flatten config into key-value pairs with sources
fn flatten_config(
    prefix: &str,
    local: &serde_json::Value,
    global: &serde_json::Value,
    default: &serde_json::Value,
    values: &mut Vec<ConfigValue>,
) {
    match local {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                let new_prefix = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", prefix, key)
                };

                let global_val = global.get(key).unwrap_or(&serde_json::Value::Null);
                let default_val = default.get(key).unwrap_or(&serde_json::Value::Null);

                flatten_config(&new_prefix, value, global_val, default_val, values);
            }
        }
        _ => {
            // Determine source
            let source = if local != default && local != global {
                "local"
            } else if global != default {
                "global"
            } else {
                "default"
            };

            values.push(ConfigValue {
                key: prefix.to_string(),
                value: local.clone(),
                source: source.to_string(),
            });
        }
    }
}

/// Print configuration in a human-readable format
fn print_config_list(
    default: &PrismConfig,
    global: &PrismConfig,
    local: &PrismConfig,
    loader: &ConfigLoader,
    workspace: &std::path::Path,
) {
    println!("Prism Configuration");
    println!("===================\n");

    // Show paths
    if let Some(gp) = loader.global_config_path() {
        let status = if gp.exists() { "" } else { " (not found)" };
        println!("Global config: {}{}", gp.display(), status);
    }
    let lp = loader.local_config_path(workspace);
    let status = if lp.exists() { "" } else { " (not found)" };
    println!("Local config:  {}{}\n", lp.display(), status);

    // Print sections
    println!("[storage]");
    print_value(
        "prism_dir",
        &local.storage.prism_dir,
        &global.storage.prism_dir,
        &default.storage.prism_dir,
    );
    print_value(
        "compression",
        &local.storage.compression,
        &global.storage.compression,
        &default.storage.compression,
    );
    print_value(
        "max_partition_size_mb",
        &local.storage.max_partition_size_mb,
        &global.storage.max_partition_size_mb,
        &default.storage.max_partition_size_mb,
    );

    println!("\n[backend.qdrant]");
    print_value(
        "url",
        &local.backend.qdrant.url,
        &global.backend.qdrant.url,
        &default.backend.qdrant.url,
    );
    print_value(
        "collection_prefix",
        &local.backend.qdrant.collection_prefix,
        &global.backend.qdrant.collection_prefix,
        &default.backend.qdrant.collection_prefix,
    );
    print_value(
        "hnsw_enabled",
        &local.backend.qdrant.hnsw_enabled,
        &global.backend.qdrant.hnsw_enabled,
        &default.backend.qdrant.hnsw_enabled,
    );

    println!("\n[analysis]");
    print_value(
        "max_file_size_kb",
        &local.analysis.max_file_size_kb,
        &global.analysis.max_file_size_kb,
        &default.analysis.max_file_size_kb,
    );
    print_value(
        "detect_components",
        &local.analysis.detect_components,
        &global.analysis.detect_components,
        &default.analysis.detect_components,
    );
    print_value(
        "parallelism",
        &local.analysis.parallelism,
        &global.analysis.parallelism,
        &default.analysis.parallelism,
    );

    println!("\n[workspace]");
    print_value(
        "cross_workspace_search",
        &local.workspace.cross_workspace_search,
        &global.workspace.cross_workspace_search,
        &default.workspace.cross_workspace_search,
    );

    println!("\n[logging]");
    print_value(
        "level",
        &local.logging.level,
        &global.logging.level,
        &default.logging.level,
    );
}

/// Print a configuration value with its source
fn print_value<T: std::fmt::Debug + PartialEq>(key: &str, local: &T, global: &T, default: &T) {
    let source = if local != default && local != global {
        " (local)"
    } else if global != default {
        " (global)"
    } else {
        ""
    };

    println!("  {} = {:?}{}", key, local, source);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_config_value() {
        let config = PrismConfig::default();

        let url = get_config_value(&config, "backend.qdrant.url");
        assert!(url.is_some());
        assert_eq!(url.unwrap(), "http://localhost:6334");

        let invalid = get_config_value(&config, "nonexistent.key");
        assert!(invalid.is_none());
    }

    #[test]
    fn test_set_config_value() {
        let mut config = PrismConfig::default();

        set_config_value(&mut config, "backend.qdrant.url", "http://custom:6334").unwrap();
        assert_eq!(config.backend.qdrant.url, "http://custom:6334");

        set_config_value(&mut config, "logging.level", "debug").unwrap();
        assert_eq!(config.logging.level, "debug");

        set_config_value(&mut config, "storage.compression", "true").unwrap();
        assert!(config.storage.compression);
    }

    #[test]
    fn test_set_config_value_invalid() {
        let mut config = PrismConfig::default();

        // Boolean parse error
        let result = set_config_value(&mut config, "storage.compression", "invalid");
        assert!(result.is_err());

        // Unknown key
        let result = set_config_value(&mut config, "unknown.key", "value");
        assert!(result.is_err());
    }

    #[test]
    fn test_config_paths_serialization() {
        let paths = ConfigPaths {
            global: Some(PathBuf::from("/home/user/.codeprysm/config.toml")),
            local: PathBuf::from("/project/.codeprysm/config.toml"),
            global_exists: true,
            local_exists: false,
        };

        let json = serde_json::to_string(&paths).unwrap();
        assert!(json.contains("\"global_exists\":true"));
        assert!(json.contains("\"local_exists\":false"));
    }

    #[test]
    fn test_config_value_serialization() {
        let value = ConfigValue {
            key: "backend.qdrant.url".to_string(),
            value: serde_json::json!("http://localhost:6334"),
            source: "default".to_string(),
        };

        let json = serde_json::to_string(&value).unwrap();
        assert!(json.contains("\"key\":\"backend.qdrant.url\""));
        assert!(json.contains("\"source\":\"default\""));
    }

    #[test]
    fn test_generate_basic_template() {
        use std::path::PathBuf;
        let temp_path = PathBuf::from("/tmp/test");
        let template = generate_basic_template(&temp_path);

        assert!(template.contains("# CodePrysm Local Configuration"));
        assert!(template.contains("[analysis]"));
        assert!(template.contains("include_patterns = []"));
        assert!(template.contains("exclude_patterns"));
        assert!(template.contains("detect_components = true"));
        assert!(template.contains("[storage]"));
        assert!(template.contains("prism_dir = \".codeprysm\""));
    }

    #[test]
    fn test_generate_monorepo_template() {
        use std::path::PathBuf;
        let temp_path = PathBuf::from("/tmp/monorepo-test");
        let template = generate_monorepo_template(&temp_path);

        assert!(template.contains("Monorepo Template"));
        assert!(template.contains("⚠️ IMPORTANT"));
        assert!(template.contains("include_patterns"));
        assert!(template.contains("packages/frontend/**"));
        assert!(template.contains("packages/backend/**"));
        assert!(template.contains("packages/shared/**"));
        assert!(template.contains("max_partition_size_mb = 100"));
    }

    #[test]
    fn test_generate_minimal_template() {
        let template = generate_minimal_template();

        assert!(template.contains("Minimal Template"));
        assert!(template.contains("[analysis]"));
        assert!(template.contains("[storage]"));
        assert!(template.contains("include_patterns = []"));
        // Should be very short
        assert!(template.len() < 500);
    }

    #[test]
    fn test_generate_template_config_basic() {
        use std::path::PathBuf;
        let temp_path = PathBuf::from("/tmp/test");
        let result = generate_template_config(ConfigTemplate::Basic, &temp_path);

        assert!(result.is_ok());
        let config = result.unwrap();
        assert!(config.contains("Local Configuration"));
    }

    #[test]
    fn test_generate_template_config_monorepo() {
        use std::path::PathBuf;
        let temp_path = PathBuf::from("/tmp/test");
        let result = generate_template_config(ConfigTemplate::Monorepo, &temp_path);

        assert!(result.is_ok());
        let config = result.unwrap();
        assert!(config.contains("Monorepo"));
    }

    #[test]
    fn test_generate_template_config_minimal() {
        use std::path::PathBuf;
        let temp_path = PathBuf::from("/tmp/test");
        let result = generate_template_config(ConfigTemplate::Minimal, &temp_path);

        assert!(result.is_ok());
        let config = result.unwrap();
        assert!(config.contains("Minimal"));
    }
}
