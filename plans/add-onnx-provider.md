# Plan: Add ONNX Runtime Embedding Provider with Multi-Backend Support

## Overview

Add ONNX Runtime as a new embedding provider to CodePrysm with support for CPU, DirectML (Windows GPU), and OpenVINO (Intel hardware) execution providers. This will enable:
- Intel Arc GPU acceleration via DirectML on Windows
- Intel CPU/GPU acceleration via OpenVINO
- Cross-platform CPU inference as a baseline
- Runtime provider selection via configuration

**Status:** Experimental (depends on `ort` crate 2.0.0-rc which is not yet stable)

## Architecture Summary

CodePrysm uses a provider pattern for embeddings with excellent separation:
- **Provider trait** (`EmbeddingProvider`) - 6 async methods all providers must implement
- **Factory** - Instantiates providers based on configuration
- **Config system** - TOML files + env vars + CLI flags with merge priority
- **Build features** - Optional dependencies gated by Cargo features

## Implementation Plan

### Phase 1: Add ONNX Provider Type to Type System

**Files to modify:**

#### 1.1. `crates/codeprysm-search/src/embeddings/provider.rs`
- Add `Onnx` variant to `EmbeddingProviderType` enum (line 16-24)
- Update `Display` impl to handle `Onnx` → `"onnx"` (lines 26-34)
- Add to match arms in any exhaustive pattern matches

#### 1.2. `crates/codeprysm-config/src/lib.rs`
- Add `Onnx` variant to `EmbeddingProviderType` enum (lines 129-137)
- Update `Display` impl (lines 139-147)
- Update `FromStr` impl to parse "onnx" (lines 149-163)
  - Accept: `"onnx"`, `"onnxruntime"`, `"onnx_runtime"`
  - Update error message to include "onnx" as valid value
- Add validation case to `EmbeddingConfig::validate()` (lines 80-123)

### Phase 2: Create ONNX Configuration Structs

**Files to modify:**

#### 2.1. `crates/codeprysm-config/src/lib.rs`
Add after `OpenAISettings` (after line 247):

```rust
/// ONNX Runtime provider settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OnnxSettings {
    /// Path to semantic embedding model (ONNX format)
    pub semantic_model_path: PathBuf,

    /// Path to code embedding model (ONNX format)
    pub code_model_path: PathBuf,

    /// Execution provider to use
    pub execution_provider: OnnxExecutionProvider,

    /// Device ID (for GPU providers, typically 0)
    pub device_id: u32,

    /// Number of threads for CPU inference
    pub num_threads: Option<usize>,

    /// Enable optimizations
    pub enable_optimizations: bool,
}

impl Default for OnnxSettings {
    fn default() -> Self {
        Self {
            semantic_model_path: PathBuf::from("models/jina-semantic.onnx"),
            code_model_path: PathBuf::from("models/jina-code.onnx"),
            execution_provider: OnnxExecutionProvider::Cpu,
            device_id: 0,
            num_threads: None, // Auto-detect
            enable_optimizations: true,
        }
    }
}

/// ONNX Runtime execution provider selection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OnnxExecutionProvider {
    /// CPU with optimizations (default)
    Cpu,
    /// DirectML (Windows DirectX ML for AMD/NVIDIA/Intel GPUs)
    DirectMl,
    /// OpenVINO (Intel CPUs and GPUs including Arc)
    OpenVino,
}
```

#### 2.2. `crates/codeprysm-config/src/lib.rs`
- Add `pub onnx: Option<OnnxSettings>` to `EmbeddingConfig` struct (line 75)
- Update `EmbeddingConfig::validate()` to validate ONNX settings

### Phase 3: Create ONNX Provider Implementation

**New file:** `crates/codeprysm-search/src/embeddings/onnx.rs`

Structure (following `local.rs` pattern):

```rust
//! ONNX Runtime embedding provider (EXPERIMENTAL)
//!
//! **Status:** Experimental - depends on `ort` crate 2.0.0-rc which is not yet stable.
//! API may change in future releases.
//!
//! Provides local inference using ONNX Runtime with multiple execution providers:
//! - **CPU**: Optimized CPU inference with threading
//! - **DirectML**: Windows DirectX ML for Intel Arc/AMD/NVIDIA GPUs
//! - **OpenVINO**: Intel-optimized for CPUs and GPUs
//!
//! Execution providers selected via compile-time features:
//! - `--features onnx-directml` for DirectML support
//! - `--features onnx-openvino` for OpenVINO support

use std::sync::Arc;
use async_trait::async_trait;
use once_cell::sync::OnceCell;
use std::path::PathBuf;
use tokenizers::{Tokenizer, PaddingParams, PaddingStrategy};
use tracing::{debug, info};

use crate::error::{Result, SearchError};
use super::provider::{EmbeddingProvider, EmbeddingProviderType, ProviderStatus};

pub const EMBEDDING_DIM: usize = 768;

/// Configuration for ONNX provider
#[derive(Debug, Clone)]
pub struct OnnxConfig {
    pub semantic_model_path: PathBuf,
    pub code_model_path: PathBuf,
    pub execution_provider: ExecutionProvider,
    pub device_id: u32,
    pub num_threads: Option<usize>,
}

/// Execution provider selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionProvider {
    Cpu,
    DirectML,
    OpenVino,
}

/// ONNX Runtime provider
#[derive(Clone)]
pub struct OnnxProvider {
    inner: Arc<OnnxProviderInner>,
}

struct OnnxProviderInner {
    semantic_model: OnceCell<SemanticModel>,
    code_model: OnceCell<CodeModel>,
    config: OnnxConfig,
}

struct SemanticModel {
    session: ort::Session,
    tokenizer: Tokenizer,
}

struct CodeModel {
    session: ort::Session,
    tokenizer: Tokenizer,
}

impl OnnxProvider {
    pub fn new(config: OnnxConfig) -> Result<Self> {
        // Validate execution provider availability
        validate_execution_provider(&config.execution_provider)?;

        Ok(Self {
            inner: Arc::new(OnnxProviderInner {
                semantic_model: OnceCell::new(),
                code_model: OnceCell::new(),
                config,
            }),
        })
    }

    fn ensure_semantic_model(&self) -> Result<&SemanticModel> {
        self.inner.semantic_model.get_or_try_init(|| {
            load_onnx_model(
                &self.inner.config.semantic_model_path,
                &self.inner.config.execution_provider,
                self.inner.config.device_id,
                self.inner.config.num_threads,
            )
        })
    }

    fn encode_semantic_sync(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let model = self.ensure_semantic_model()?;
        run_onnx_inference(&model.session, &model.tokenizer, texts)
    }
}

#[async_trait]
impl EmbeddingProvider for OnnxProvider {
    async fn encode_semantic(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        let provider = self.clone();
        tokio::task::spawn_blocking(move || provider.encode_semantic_sync(&texts))
            .await
            .map_err(|e| SearchError::Embedding(format!("Task panicked: {}", e)))?
    }

    async fn encode_code(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        let provider = self.clone();
        tokio::task::spawn_blocking(move || provider.encode_code_sync(&texts))
            .await
            .map_err(|e| SearchError::Embedding(format!("Task panicked: {}", e)))?
    }

    async fn check_status(&self) -> Result<ProviderStatus> {
        // Check model files exist, execution provider available
        let semantic_exists = self.inner.config.semantic_model_path.exists();
        let code_exists = self.inner.config.code_model_path.exists();

        if !semantic_exists || !code_exists {
            return Ok(ProviderStatus::unavailable(
                EmbeddingProviderType::Onnx,
                "Model files not found",
            ));
        }

        let device = execution_provider_name(&self.inner.config.execution_provider);
        Ok(ProviderStatus::healthy(EmbeddingProviderType::Onnx, device))
    }

    async fn warmup(&self) -> Result<()> {
        let provider = self.clone();
        tokio::task::spawn_blocking(move || {
            provider.ensure_semantic_model()?;
            provider.ensure_code_model()?;
            Ok::<_, SearchError>(())
        })
        .await
        .map_err(|e| SearchError::Embedding(format!("Warmup panicked: {}", e)))??;
        Ok(())
    }

    fn embedding_dim(&self) -> usize {
        EMBEDDING_DIM
    }

    fn provider_type(&self) -> EmbeddingProviderType {
        EmbeddingProviderType::Onnx
    }
}

// Helper functions for ONNX session creation, inference, etc.
```

Key implementation details:
- Use `ort` crate (ONNX Runtime bindings)
- Lazy model loading with `OnceCell` (same pattern as `LocalProvider`)
- Async methods use `spawn_blocking` for CPU-bound inference
- Support execution provider selection at runtime
- Reuse tokenizers from HuggingFace tokenizers crate

### Phase 4: Add ONNX Dependencies to Cargo.toml

**File:** `crates/codeprysm-search/Cargo.toml`

#### 4.1. Add features (after line 18):
```toml
[features]
default = []
metal = ["candle-core/metal", "candle-nn/metal", "candle-transformers/metal"]
cuda = ["candle-core/cuda", "candle-nn/cuda", "candle-transformers/cuda"]
rate-limit = ["dep:governor"]
# ONNX Runtime features
onnx = ["dep:ort"]
onnx-directml = ["onnx", "ort/directml"]
onnx-openvino = ["onnx", "ort/openvino"]
```

#### 4.2. Add dependency (after line 69):
```toml
# ONNX Runtime (optional)
# Note: Check https://crates.io/crates/ort for latest version
# As of Jan 2025, latest is 2.0.0-rc.x (release candidate)
ort = { version = "2.0.0-rc", optional = true, default-features = false }
```

### Phase 5: Propagate Features Through Crate Chain

#### 5.1. `crates/codeprysm-backend/Cargo.toml`
Add after line 48:
```toml
onnx = ["codeprysm-search/onnx"]
onnx-directml = ["codeprysm-search/onnx-directml"]
onnx-openvino = ["codeprysm-search/onnx-openvino"]
```

#### 5.2. `crates/codeprysm-mcp/Cargo.toml`
Add after line 17:
```toml
onnx = ["codeprysm-search/onnx"]
onnx-directml = ["codeprysm-search/onnx-directml"]
onnx-openvino = ["codeprysm-search/onnx-openvino"]
```

#### 5.3. `crates/codeprysm-cli/Cargo.toml`
Add after line 17:
```toml
onnx = ["codeprysm-search/onnx", "codeprysm-backend/onnx", "codeprysm-mcp/onnx"]
onnx-directml = ["codeprysm-search/onnx-directml", "codeprysm-backend/onnx-directml", "codeprysm-mcp/onnx-directml"]
onnx-openvino = ["codeprysm-search/onnx-openvino", "codeprysm-backend/onnx-openvino", "codeprysm-mcp/onnx-openvino"]
```

### Phase 6: Update Factory to Support ONNX

**File:** `crates/codeprysm-search/src/embeddings/factory.rs`

#### 6.1. Add import (line 13):
```rust
use super::onnx::{OnnxConfig as OnnxProviderConfig, OnnxProvider};
```

#### 6.2. Add field to `EmbeddingConfig` (line 31):
```rust
/// ONNX provider settings (used when provider = Onnx)
pub onnx: Option<OnnxProviderConfig>,
```

#### 6.3. Add factory method (after line 78):
```rust
/// Create config for ONNX provider with explicit config
pub fn onnx_with_config(config: OnnxProviderConfig) -> Self {
    Self {
        provider: EmbeddingProviderType::Onnx,
        azure_ml: None,
        openai: None,
        onnx: Some(config),
    }
}

/// Create config for ONNX provider (reads from environment)
pub fn onnx() -> Self {
    Self {
        provider: EmbeddingProviderType::Onnx,
        azure_ml: None,
        openai: None,
        onnx: None,
    }
}
```

#### 6.4. Add match arm in `create()` function (after line 148):
```rust
#[cfg(feature = "onnx")]
EmbeddingProviderType::Onnx => {
    let provider = if let Some(ref onnx_config) = config.onnx {
        OnnxProvider::new(onnx_config.clone())?
    } else {
        OnnxProvider::from_env()?
    };
    Arc::new(provider)
}
#[cfg(not(feature = "onnx"))]
EmbeddingProviderType::Onnx => {
    return Err(SearchError::Embedding(
        "ONNX provider not available. Rebuild with --features onnx".into(),
    ));
}
```

### Phase 7: Update Config Conversion in CLI

**File:** `crates/codeprysm-cli/src/commands/mod.rs`

Add match arm in `to_search_embedding_config()` function (after line 186):

```rust
EmbeddingProviderType::Onnx => {
    if let Some(ref onnx) = config.embedding.onnx {
        // Convert config types
        let execution_provider = match onnx.execution_provider {
            codeprysm_config::OnnxExecutionProvider::Cpu => {
                codeprysm_search::embeddings::onnx::ExecutionProvider::Cpu
            }
            codeprysm_config::OnnxExecutionProvider::DirectMl => {
                codeprysm_search::embeddings::onnx::ExecutionProvider::DirectML
            }
            codeprysm_config::OnnxExecutionProvider::OpenVino => {
                codeprysm_search::embeddings::onnx::ExecutionProvider::OpenVino
            }
        };

        let onnx_config = codeprysm_search::embeddings::onnx::OnnxConfig {
            semantic_model_path: onnx.semantic_model_path.clone(),
            code_model_path: onnx.code_model_path.clone(),
            execution_provider,
            device_id: onnx.device_id,
            num_threads: onnx.num_threads,
        };
        SearchEmbeddingConfig::onnx_with_config(onnx_config)
    } else {
        SearchEmbeddingConfig::onnx()
    }
}
```

### Phase 8: Update Module Exports

#### 8.1. `crates/codeprysm-search/src/embeddings/mod.rs`
Add after line 44:
```rust
#[cfg(feature = "onnx")]
pub mod onnx;
```

Add to re-exports (after line 60):
```rust
#[cfg(feature = "onnx")]
pub use onnx::{OnnxConfig, OnnxProvider, ExecutionProvider};
```

#### 8.2. `crates/codeprysm-config/src/lib.rs`
Update `EmbeddingConfig` struct (line 67):
```rust
pub struct EmbeddingConfig {
    pub provider: EmbeddingProviderType,
    pub azure_ml: Option<AzureMLSettings>,
    pub openai: Option<OpenAISettings>,
    pub onnx: Option<OnnxSettings>,
}
```

## Critical Files Summary

**Files to create:**
- `crates/codeprysm-search/src/embeddings/onnx.rs` (~600 lines)

**Files to modify:**
- `crates/codeprysm-search/src/embeddings/provider.rs` - Add enum variant
- `crates/codeprysm-search/src/embeddings/factory.rs` - Add factory support
- `crates/codeprysm-search/src/embeddings/mod.rs` - Add module export
- `crates/codeprysm-search/Cargo.toml` - Add dependencies and features
- `crates/codeprysm-config/src/lib.rs` - Add config structs and enum variants
- `crates/codeprysm-cli/src/commands/mod.rs` - Add config conversion
- `crates/codeprysm-cli/Cargo.toml` - Propagate features
- `crates/codeprysm-backend/Cargo.toml` - Propagate features
- `crates/codeprysm-mcp/Cargo.toml` - Propagate features

## Configuration Example

Users will configure ONNX provider via TOML:

```toml
# ~/.codeprysm/config.toml

[embedding]
provider = "onnx"

[embedding.onnx]
semantic_model_path = "~/.cache/codeprysm/models/jina-semantic.onnx"
code_model_path = "~/.cache/codeprysm/models/jina-code.onnx"
execution_provider = "directml"  # or "cpu" or "openvino"
device_id = 0
num_threads = 4
enable_optimizations = true
```

Or via CLI:
```bash
codeprysm init --embedding-provider onnx
```

Or via environment variables:
```bash
export CODEPRYSM_EMBEDDING_PROVIDER=onnx
export CODEPRYSM_ONNX_EXECUTION_PROVIDER=directml
```

## Build Instructions

```bash
# Base ONNX with CPU support
cargo build --features onnx --release

# ONNX with DirectML (Windows, Intel Arc/AMD/NVIDIA)
cargo build --features onnx-directml --release

# ONNX with OpenVINO (Intel CPUs/GPUs)
cargo build --features onnx-openvino --release

# Multiple execution providers
cargo build --features onnx-directml,onnx-openvino --release
```

## Verification Plan

### 1. Unit Tests
- Test ONNX provider creation with different configs
- Test execution provider selection logic
- Test model loading and lazy initialization
- Test encoding with sample inputs

### 2. Integration Tests
Create `crates/codeprysm-search/tests/onnx_integration.rs`:
```rust
#[cfg(feature = "onnx")]
#[tokio::test]
async fn test_onnx_provider_semantic_encoding() {
    let config = OnnxConfig {
        semantic_model_path: "tests/fixtures/jina-semantic.onnx".into(),
        code_model_path: "tests/fixtures/jina-code.onnx".into(),
        execution_provider: ExecutionProvider::Cpu,
        device_id: 0,
        num_threads: Some(1),
    };

    let provider = OnnxProvider::new(config).unwrap();
    let texts = vec!["hello world".to_string()];
    let embeddings = provider.encode_semantic(texts).await.unwrap();

    assert_eq!(embeddings.len(), 1);
    assert_eq!(embeddings[0].len(), 768);
}
```

### 3. End-to-End Test
```bash
# 1. Build with ONNX support
cargo build --features onnx --release

# 2. Configure ONNX provider
cat > ~/.codeprysm/config.toml <<EOF
[embedding]
provider = "onnx"

[embedding.onnx]
semantic_model_path = "models/jina-semantic.onnx"
code_model_path = "models/jina-code.onnx"
execution_provider = "cpu"
EOF

# 3. Initialize a test repo
./target/release/codeprysm init test-repo/

# 4. Verify embeddings were generated
./target/release/codeprysm status

# 5. Test search functionality
./target/release/codeprysm search "authentication"
```

### 4. Feature Flag Tests
```bash
# Verify build fails without feature
cargo build --release  # Should compile without onnx module

# Verify DirectML feature
cargo build --features onnx-directml --release

# Verify OpenVINO feature
cargo build --features onnx-openvino --release

# Verify error message when ONNX not compiled
# (try to use onnx provider without --features onnx)
```

### 5. Configuration Validation Tests
- Test TOML parsing with valid/invalid ONNX configs
- Test CLI flag `--embedding-provider onnx`
- Test environment variable `CODEPRYSM_EMBEDDING_PROVIDER=onnx`
- Test config validation errors (missing model files, invalid execution provider)

## Notes

### Experimental Status

**Important:** The ONNX provider should be marked as experimental because:

1. **Dependency on RC version**: `ort` crate 2.0.0 is still in release candidate phase
   - API may change before stable release
   - Potential breaking changes between RC versions
   - Not recommended for production without testing

2. **Documentation needed**:
   - Add "⚠️ Experimental" badge in user-facing docs
   - Clearly state in README and guides that ONNX support is experimental
   - Recommend pinning to specific RC version: `ort = "=2.0.0-rc.11"`

3. **Version upgrade path**:
   - Monitor `ort` releases: https://crates.io/crates/ort
   - Test against each new RC version
   - Update to stable 2.0.0 when available
   - Document any API changes needed

4. **User expectations**:
   - Warn users that ONNX provider may have bugs or API changes
   - Provide fallback instructions (use LocalProvider with Candle instead)
   - Collect feedback to stabilize before promoting to stable

**Recommendation:** Add feature flag documentation noting experimental status:
```toml
# Cargo.toml
[features]
# EXPERIMENTAL: ONNX Runtime support (requires ort 2.0.0-rc)
# API may change. Not recommended for production use.
onnx = ["dep:ort"]
```

### Model Export
Users will need to export Jina models to ONNX format. Consider providing a helper script:
```bash
python scripts/export_jina_to_onnx.py \
  --model jinaai/jina-embeddings-v2-base-en \
  --output models/jina-semantic.onnx
```

### Execution Provider Notes
- **DirectML**: Windows-only, supports Intel Arc, AMD, NVIDIA
- **OpenVINO**: Cross-platform, Intel-optimized, best for Intel Arc on Linux
- **CPU**: Always available, no special dependencies

### Performance Expectations
- CPU: Baseline performance, no GPU required
- DirectML: 5-10x faster than CPU on Intel Arc
- OpenVINO: 3-8x faster than CPU on Intel hardware

### Dependencies
- `ort` crate version 2.0.0-rc.x+ (ONNX Runtime Rust bindings - check latest at https://crates.io/crates/ort)
  - ⚠️ **Release candidate version** - API not yet stable
  - Recommend pinning to specific version: `ort = "=2.0.0-rc.11"`
- ONNX Runtime system libraries (may need separate installation)
- For DirectML: Windows 10+ with DirectX 12 capable GPU
- For OpenVINO: Intel OpenVINO toolkit installed

## Estimated Effort

- Phase 1-2 (Type system & config): 1 hour
- Phase 3 (ONNX provider implementation): 4-6 hours
- Phase 4-5 (Cargo features): 1 hour
- Phase 6-8 (Factory & integration): 2 hours
- Testing & documentation: 2-3 hours

**Total: 10-13 hours**
