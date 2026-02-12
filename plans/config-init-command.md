# Implementation Plan: `config init` Command

## Overview

Add a new subcommand `codeprysm config init` to create the `.codeprysm` directory structure and configuration files **before** running `codeprysm init`. This solves the UX problem where users cannot configure `include_patterns` before the (potentially long) graph generation process starts.

---

## Problem Statement

**Current Flow:**
```
User runs: codeprysm init
  ↓
.codeprysm/ created (line 175-178 in init.rs)
  ↓
Graph building starts IMMEDIATELY (line 216)
  ↓
Config file created at END (line 396-410)
```

**Issue:** User cannot configure `include_patterns` before analysis begins. For large monorepos, this means:
- Analyzing all 50 packages when user only needs 2
- Wasting time/resources on unwanted code
- Having to interrupt init process (poor UX)

**Desired Flow:**
```
User runs: codeprysm config init
  ↓
.codeprysm/ created + config.toml with template
  ↓
User edits config.toml to set include_patterns
  ↓
User runs: codeprysm init
  ↓
Graph building uses configured include_patterns
```

---

## Design Overview

### New Command Structure

```
codeprysm config
├── list              (existing)
├── get               (existing)
├── set               (existing)
├── path              (existing)
└── init              (NEW)
    ├── [path]        optional: path to initialize (default: current dir)
    ├── --force       overwrite existing config
    ├── --interactive interactive mode (ask questions)
    └── --template    template type (basic, monorepo, minimal)
```

### Command Behavior

**Simple Usage:**
```bash
codeprysm config init
```
- Creates `.codeprysm/` directory if not exists
- Writes `.codeprysm/config.toml` with commented template
- Returns with helpful next-step message

**Interactive Mode:**
```bash
codeprysm config init --interactive
```
- Asks questions about workspace (monorepo? include patterns? components?)
- Generates customized config based on answers
- Optional: preview discovered code roots

**Force Overwrite:**
```bash
codeprysm config init --force
```
- Overwrites existing config (with backup to `config.toml.backup`)

---

## Implementation Details

### 1. Add Command Variant to ConfigCommand Enum

**File:** `crates/codeprysm-cli/src/commands/config.rs`

**Location:** After line 31

```rust
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

    /// Initialize configuration directory and files (NEW)
    Init(InitArgs),
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
```

### 2. Update Command Router

**File:** `crates/codeprysm-cli/src/commands/config.rs`

**Location:** In `execute()` function (line 104)

```rust
/// Execute the config command
pub async fn execute(cmd: ConfigCommand, global: GlobalOptions) -> Result<()> {
    match cmd {
        ConfigCommand::List(args) => execute_list(args, global).await,
        ConfigCommand::Get(args) => execute_get(args, global).await,
        ConfigCommand::Set(args) => execute_set(args, global).await,
        ConfigCommand::Path(args) => execute_path(args, global).await,
        ConfigCommand::Init(args) => execute_init(args, global).await,  // NEW
    }
}
```

### 3. Implement `execute_init()` Function

**File:** `crates/codeprysm-cli/src/commands/config.rs`

**Location:** After `execute_path()` function (after line 237)

```rust
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
    let mut loader = ConfigLoader::new();
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
```

### 4. Implement Configuration Template Generators

**File:** `crates/codeprysm-cli/src/commands/config.rs`

**Location:** After `execute_init()`

```rust
/// Generate configuration based on template type
fn generate_template_config(template: ConfigTemplate, workspace: &Path) -> Result<String> {
    match template {
        ConfigTemplate::Basic => Ok(generate_basic_template(workspace)),
        ConfigTemplate::Monorepo => Ok(generate_monorepo_template(workspace)),
        ConfigTemplate::Minimal => Ok(generate_minimal_template()),
    }
}

/// Generate basic configuration template
fn generate_basic_template(workspace: &Path) -> String {
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
fn generate_monorepo_template(workspace: &Path) -> String {
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
```

### 5. Implement Interactive Configuration

**File:** `crates/codeprysm-cli/src/commands/config.rs`

**Location:** After template generators

```rust
/// Generate configuration interactively by asking user questions
fn generate_interactive_config(workspace: &Path, quiet: bool) -> Result<String> {
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
```

### 6. Implement Next Steps Helper

**File:** `crates/codeprysm-cli/src/commands/config.rs`

**Location:** After interactive config function

```rust
/// Print helpful next steps after config creation
fn print_next_steps(config_path: &Path, template: ConfigTemplate) {
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
```

---

## Integration with `codeprysm init`

### Update init.rs to Handle Pre-Existing Config

**File:** `crates/codeprysm-cli/src/commands/init.rs`

**No breaking changes needed!** The current init already:
1. Loads config from `.codeprysm/config.toml` if it exists (via `load_config()`)
2. Uses `config.analysis.exclude_patterns` in BuilderConfig (line 197)
3. Writes config only if it doesn't exist (line 396)

**Enhancement (optional):** Add informational message when config exists:

```rust
// After line 135 in init.rs
let config = load_config(&global, &workspace_path)?;

// Add this:
if local_config_path.exists() && !quiet {
    println!("ℹ️  Using existing configuration from {}", local_config_path.display());
}
```

---

## Testing Plan

### Unit Tests

**File:** `crates/codeprysm-cli/src/commands/config.rs` (at end)

```rust
#[cfg(test)]
mod config_init_tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_generate_basic_template() {
        let temp = TempDir::new().unwrap();
        let template = generate_basic_template(temp.path());

        assert!(template.contains("[analysis]"));
        assert!(template.contains("include_patterns = []"));
        assert!(template.contains("exclude_patterns"));
        assert!(template.contains("detect_components = true"));
    }

    #[test]
    fn test_generate_monorepo_template() {
        let temp = TempDir::new().unwrap();
        let template = generate_monorepo_template(temp.path());

        assert!(template.contains("Monorepo"));
        assert!(template.contains("include_patterns"));
        assert!(template.contains("packages/frontend/**"));
    }

    #[test]
    fn test_generate_minimal_template() {
        let template = generate_minimal_template();

        assert!(template.contains("[analysis]"));
        assert!(template.contains("[storage]"));
        // Should be very short
        assert!(template.len() < 500);
    }

    #[tokio::test]
    async fn test_execute_init_creates_directory() {
        let temp = TempDir::new().unwrap();

        let args = InitArgs {
            path: temp.path().to_path_buf(),
            force: false,
            interactive: false,
            template: ConfigTemplate::Basic,
        };

        let global = GlobalOptions {
            workspace: None,
            config: None,
            verbose: false,
            quiet: true,  // Suppress output in tests
            qdrant_url: "http://localhost:6334".to_string(),
            embedding_provider: None,
        };

        let result = execute_init(args, global).await;
        assert!(result.is_ok());

        let config_path = temp.path().join(".codeprysm/config.toml");
        assert!(config_path.exists());
    }

    #[tokio::test]
    async fn test_execute_init_force_creates_backup() {
        let temp = TempDir::new().unwrap();
        let prism_dir = temp.path().join(".codeprysm");
        let config_path = prism_dir.join("config.toml");

        // Create existing config
        std::fs::create_dir_all(&prism_dir).unwrap();
        std::fs::write(&config_path, "# existing config").unwrap();

        let args = InitArgs {
            path: temp.path().to_path_buf(),
            force: true,
            interactive: false,
            template: ConfigTemplate::Basic,
        };

        let global = GlobalOptions {
            workspace: None,
            config: None,
            verbose: false,
            quiet: true,
            qdrant_url: "http://localhost:6334".to_string(),
            embedding_provider: None,
        };

        let result = execute_init(args, global).await;
        assert!(result.is_ok());

        // Check backup was created
        let backup_path = prism_dir.join("config.toml.backup");
        assert!(backup_path.exists());

        let backup_content = std::fs::read_to_string(&backup_path).unwrap();
        assert_eq!(backup_content, "# existing config");
    }

    #[tokio::test]
    async fn test_execute_init_without_force_fails_if_exists() {
        let temp = TempDir::new().unwrap();
        let prism_dir = temp.path().join(".codeprysm");
        let config_path = prism_dir.join("config.toml");

        // Create existing config
        std::fs::create_dir_all(&prism_dir).unwrap();
        std::fs::write(&config_path, "# existing").unwrap();

        let args = InitArgs {
            path: temp.path().to_path_buf(),
            force: false,
            interactive: false,
            template: ConfigTemplate::Basic,
        };

        let global = GlobalOptions {
            workspace: None,
            config: None,
            verbose: false,
            quiet: true,
            qdrant_url: "http://localhost:6334".to_string(),
            embedding_provider: None,
        };

        let result = execute_init(args, global).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }
}
```

### Integration Tests

**File:** `crates/codeprysm-cli/tests/config_init_integration.rs` (new file)

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn test_config_init_creates_directory() {
    let temp = TempDir::new().unwrap();

    Command::cargo_bin("codeprysm")
        .unwrap()
        .args(["config", "init", temp.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created"));

    let config_path = temp.path().join(".codeprysm/config.toml");
    assert!(config_path.exists());
}

#[test]
fn test_config_init_then_init_workflow() {
    let temp = TempDir::new().unwrap();

    // Create a test source file
    std::fs::write(temp.path().join("main.py"), "print('hello')").unwrap();

    // Step 1: config init
    Command::cargo_bin("codeprysm")
        .unwrap()
        .args(["config", "init", temp.path().to_str().unwrap()])
        .assert()
        .success();

    // Verify config exists
    let config_path = temp.path().join(".codeprysm/config.toml");
    assert!(config_path.exists());

    // Step 2: init (should use existing config)
    Command::cargo_bin("codeprysm")
        .unwrap()
        .args(["init", temp.path().to_str().unwrap(), "--no-index"])
        .assert()
        .success();

    // Verify graph was created
    let manifest_path = temp.path().join(".codeprysm/manifest.json");
    assert!(manifest_path.exists());
}

#[test]
fn test_config_init_monorepo_template() {
    let temp = TempDir::new().unwrap();

    Command::cargo_bin("codeprysm")
        .unwrap()
        .args([
            "config", "init",
            temp.path().to_str().unwrap(),
            "--template", "monorepo"
        ])
        .assert()
        .success();

    let config_content = std::fs::read_to_string(
        temp.path().join(".codeprysm/config.toml")
    ).unwrap();

    assert!(config_content.contains("Monorepo"));
    assert!(config_content.contains("packages/frontend/**"));
}

#[test]
fn test_config_init_force_overwrites() {
    let temp = TempDir::new().unwrap();
    let prism_dir = temp.path().join(".codeprysm");
    std::fs::create_dir_all(&prism_dir).unwrap();

    let config_path = prism_dir.join("config.toml");
    std::fs::write(&config_path, "# original").unwrap();

    Command::cargo_bin("codeprysm")
        .unwrap()
        .args([
            "config", "init",
            temp.path().to_str().unwrap(),
            "--force"
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Backed up"));

    // Backup should exist
    let backup_path = prism_dir.join("config.toml.backup");
    assert!(backup_path.exists());

    let backup_content = std::fs::read_to_string(&backup_path).unwrap();
    assert_eq!(backup_content, "# original");
}
```

---

## Documentation Updates

### 1. Update CLAUDE.md

**File:** `CLAUDE.md`

**Section:** Developer Setup (after line 36)

Add:

```markdown
### First-Time Setup for Large Repositories

For large monorepos or repositories where you only want to analyze specific directories:

1. Create configuration first: `just config-init` (or `codeprysm config init`)
2. Edit `.codeprysm/config.toml` to set `include_patterns`
3. Initialize graph: `just init`

**Example config for monorepo:**
```toml
[analysis]
include_patterns = [
    "packages/frontend/**",
    "packages/backend/**",
    "shared/**"
]
```
```

### 2. Update Getting Started Guide

**File:** `docs/getting-started.md`

**Add new section after "Quick Start":**

```markdown
## Large Repository Setup

If you're working with a large monorepo or only need to analyze specific directories:


### Step 1: Create Configuration Directory

```bash
codeprysm config init

# Or for monorepo with helpful template:
codeprysm config init --template monorepo

# Or interactive mode (asks questions):
codeprysm config init --interactive
```

This creates `.codeprysm/config.toml` without starting the analysis.

### Step 2: Edit Configuration

Edit `.codeprysm/config.toml` to specify which directories to analyze:

```toml
[analysis]
# Only analyze these directories
include_patterns = [
    "packages/frontend/**",
    "packages/backend/**",
    "shared/utils/**"
]
```

### Step 3: Initialize with Configuration

```bash
codeprysm init
```

Now the graph generation will only process the directories you specified!

### Why This Matters

For a monorepo with 50 packages:
- **Without include_patterns**: Analyzes all 50 packages (~30 minutes)
- **With include_patterns** (3 packages): Analyzes only 3 packages (~2 minutes)
```

### 3. Add to justfile

**File:** `justfile`

**Add after the `init` recipe:**

```makefile
# Create configuration directory and template
config-init DIR=".":
    cargo run --release -- config init {{DIR}}

# Create configuration with monorepo template
config-init-monorepo DIR=".":
    cargo run --release -- config init {{DIR}} --template monorepo

# Create configuration interactively
config-init-interactive DIR=".":
    cargo run --release -- config init {{DIR}} --interactive
```

---

## CLI Help Text

### Main Help

```
$ codeprysm --help

Commands:
  ...
  config      View and manage configuration
```

### Config Help

```
$ codeprysm config --help

View and manage configuration

Usage: codeprysm config <COMMAND>

Commands:
  list  List all configuration values with their sources
  get   Get a specific configuration value
  set   Set a configuration value
  path  Show configuration file paths
  init  Initialize configuration directory and files
  help  Print this message or the help of the given subcommand(s)
```

### Config Init Help

```
$ codeprysm config init --help

Initialize configuration directory and files

Usage: codeprysm config init [OPTIONS] [PATH]

Arguments:
  [PATH]  Path to initialize (defaults to current directory) [default: .]

Options:
  -f, --force          Force overwrite existing configuration
  -i, --interactive    Interactive mode - prompt for configuration options
      --template <TEMPLATE>
          Configuration template to use
          [default: basic]
          [possible values: basic, monorepo, minimal]
  -h, --help           Print help
```

---

## Example Usage Scenarios

### Scenario 1: Developer with Large Monorepo

```bash
# Clone a monorepo with 50 packages
git clone https://github.com/company/monorepo
cd monorepo

# Create config first
codeprysm config init --template monorepo

# Edit config to analyze only 2 packages
vim .codeprysm/config.toml
# Set: include_patterns = ["packages/my-team-frontend/**", "packages/my-team-api/**"]

# Initialize (only analyzes those 2 packages)
codeprysm init

# Result: Fast initialization, focused search
```

### Scenario 2: Quick Interactive Setup

```bash
cd my-project

codeprysm config init --interactive
# Prompts:
#   Is this a monorepo? (y/N): y
#   Include pattern: packages/web/**
#   Include pattern: packages/shared/**
#   Include pattern: [Enter to finish]
#   Enable component detection? (Y/n): y

# Config created with answers

codeprysm init
```

### Scenario 3: Minimal Config for Quick Testing

```bash
cd test-repo

codeprysm config init --template minimal

# Creates bare-bones config
# Customize as needed, then:
codeprysm init
```

### Scenario 4: Team Shared Config

```bash
# Team lead creates config
codeprysm config init --template monorepo
vim .codeprysm/config.toml
# Configure for team's needs

# Commit config to repo
git add .codeprysm/config.toml
git commit -m "Add CodePrysm config for team"

# Team members just run init (uses committed config)
git clone <repo>
codeprysm init
```

---

## Implementation Checklist

### Phase 1: Core Implementation (4-6 hours)

- [ ] Add `InitArgs` struct to `config.rs`
- [ ] Add `ConfigTemplate` enum
- [ ] Update `ConfigCommand` enum with `Init` variant
- [ ] Update `execute()` router
- [ ] Implement `execute_init()` function
- [ ] Implement `generate_basic_template()`
- [ ] Implement `generate_monorepo_template()`
- [ ] Implement `generate_minimal_template()`
- [ ] Implement `print_next_steps()`
- [ ] Test manually with all templates

### Phase 2: Interactive Mode (2-3 hours)

- [ ] Implement `generate_interactive_config()`
- [ ] Add stdin/stdout prompting
- [ ] Handle empty responses
- [ ] Test interactive mode manually

### Phase 3: Testing (3-4 hours)

- [ ] Write unit tests for template generation
- [ ] Write unit tests for `execute_init()`
- [ ] Write integration tests (CLI tests)
- [ ] Test force overwrite behavior
- [ ] Test backup creation
- [ ] Test error cases (existing config, invalid path)

### Phase 4: Documentation (2-3 hours)

- [ ] Update `CLAUDE.md`
- [ ] Update `docs/getting-started.md`
- [ ] Add new section for large repos
- [ ] Add recipes to `justfile`
- [ ] Update CLI help text
- [ ] Add examples to config templates

### Phase 5: Polish (1-2 hours)

- [ ] Add emoji/formatting to output
- [ ] Verify quiet mode works
- [ ] Test all templates
- [ ] Test interactive mode UX
- [ ] Review error messages

**Total Estimated Time: 12-18 hours**

---

## Future Enhancements (Not in Initial Implementation)

### 1. Discovery Preview

Add `--preview` flag to show what would be discovered:

```bash
codeprysm config init --preview
# Output:
#   Would discover:
#     - packages/frontend (git repo, ~500 files)
#     - packages/backend (git repo, ~300 files)
#     - scripts (code dir, ~20 files)
#
#   Configure include_patterns to limit scope.
```

### 2. Template Customization

Allow users to save custom templates:

```bash
codeprysm config template save my-team-template
codeprysm config init --template my-team-template
```

### 3. Wizard Mode

More comprehensive interactive setup:

```bash
codeprysm config init --wizard
# Asks 10+ questions about repo structure, languages, etc.
```

### 4. Auto-Detect Monorepo

Detect workspace manifests and suggest include patterns:

```bash
codeprysm config init --auto-detect
# Detects:
#   - Cargo workspace with 5 crates
#   - package.json workspaces with 10 packages
# Suggests:
#   include_patterns = ["crates/**", "packages/**"]
```

---

## Success Criteria

✅ User can create `.codeprysm/config.toml` before running `init`

✅ Templates provide helpful examples for common scenarios

✅ Interactive mode makes setup easy for first-time users

✅ Documentation clearly explains the workflow

✅ Backward compatible (existing users unaffected)

✅ All tests pass

✅ Works with workspace management features

---

## Risks & Mitigation

### Risk 1: Users confused about when to use `config init` vs `init`

**Mitigation:**
- Clear documentation with decision tree
- Helpful error messages
- `codeprysm init` could detect large repos and suggest `config init` first

### Risk 2: Config schema changes break old configs

**Mitigation:**
- Config loader already handles missing fields (uses defaults)
- Templates include comments explaining fields
- Versioning in config file (future)

### Risk 3: Interactive mode is clunky on some terminals

**Mitigation:**
- Make interactive mode optional
- Provide alternative template-based approach
- Test on Windows, macOS, Linux

---

## Appendix: Config File Structure Reference

```toml
# Complete config schema for reference

[storage]
prism_dir = ".codeprysm"
compression = true
max_partition_size_mb = 50

[backend.qdrant]
url = "http://localhost:6334"
api_key = null  # Optional
collection_prefix = "codeprysm"
vector_dimension = 768
hnsw_enabled = true

[embedding]
provider = "local"  # local | azure-ml | openai | onnx

[analysis]
max_file_size_kb = 1024
exclude_patterns = ["**/node_modules/**"]
include_patterns = []  # NEW: focus analysis on specific dirs
detect_components = true
parallelism = 0

[workspace]
cross_workspace_search = true

[logging]
level = "info"
format = "text"
```
