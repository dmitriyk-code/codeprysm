# Tasks: Add ONNX Runtime Embedding Provider

> **Related Plan:** [add-onnx-provider.md](./add-onnx-provider.md)
> **Estimated Effort:** 10-13 hours
> **Status:** Not Started
> **Assignee:** TBD
>
> ⚠️ **EXPERIMENTAL FEATURE** - Depends on `ort` crate 2.0.0-rc (release candidate)
> - API may change before stable release
> - Not recommended for production without thorough testing
> - Monitor https://crates.io/crates/ort for stable 2.0.0 release

## Prerequisites

- [ ] Review the [implementation plan](./add-onnx-provider.md)
- [ ] Review `ort` crate documentation (check latest version at https://crates.io/crates/ort):
  - Session builder API and execution provider configuration
  - How to enable DirectML and OpenVINO features
  - Tensor input/output handling
  - Error types and handling patterns
  - Thread configuration and optimization options
  - **Note:** As of Jan 2025, latest is likely 2.0.0-rc.x (release candidate)
  - Docs link example: https://docs.rs/ort/2.0.0-rc.11/ort/
- [ ] Verify ONNX Runtime system libraries can be installed
- [ ] Obtain or export Jina models in ONNX format for testing

---

## Phase 1: Type System Updates (Est: 1 hour)

### Task 1.1: Update provider type in search crate
**File:** `crates/codeprysm-search/src/embeddings/provider.rs`

- [ ] Add `Onnx` variant to `EmbeddingProviderType` enum (line 16-24)
- [ ] Update `Display` impl to return `"onnx"` for Onnx variant (lines 26-34)
- [ ] Verify all match statements are exhaustive
- [ ] Run tests: `cargo test --package codeprysm-search`

### Task 1.2: Update provider type in config crate
**File:** `crates/codeprysm-config/src/lib.rs`

- [ ] Add `Onnx` variant to `EmbeddingProviderType` enum (lines 129-137)
- [ ] Update `Display` impl (lines 139-147)
- [ ] Update `FromStr` impl to parse `"onnx"`, `"onnxruntime"`, `"onnx_runtime"` (lines 149-163)
- [ ] Update error message in `FromStr` to include "onnx" as valid value
- [ ] Add validation case to `EmbeddingConfig::validate()` for Onnx (lines 80-123)
- [ ] Run tests: `cargo test --package codeprysm-config`

**Verification:**
```bash
# Test enum parsing
cargo test --package codeprysm-config test_embedding_provider_type_from_str
```

---

## Phase 2: Configuration Structs (Est: 1 hour)

### Task 2.1: Add OnnxSettings struct
**File:** `crates/codeprysm-config/src/lib.rs` (after line 247)

- [ ] Add `OnnxSettings` struct with fields:
  - `semantic_model_path: PathBuf`
  - `code_model_path: PathBuf`
  - `execution_provider: OnnxExecutionProvider`
  - `device_id: u32`
  - `num_threads: Option<usize>`
  - `enable_optimizations: bool`
- [ ] Implement `Default` trait for `OnnxSettings`
- [ ] Add derive macros: `Debug, Clone, Serialize, Deserialize`
- [ ] Add `#[serde(default)]` attribute

### Task 2.2: Add OnnxExecutionProvider enum
**File:** `crates/codeprysm-config/src/lib.rs` (after OnnxSettings)

- [ ] Add `OnnxExecutionProvider` enum with variants:
  - `Cpu`
  - `DirectMl`
  - `OpenVino`
- [ ] Add derive macros: `Debug, Clone, Serialize, Deserialize, PartialEq, Eq`
- [ ] Add `#[serde(rename_all = "lowercase")]` attribute

### Task 2.3: Update EmbeddingConfig struct
**File:** `crates/codeprysm-config/src/lib.rs`

- [ ] Add `pub onnx: Option<OnnxSettings>` field to `EmbeddingConfig` (line 75)
- [ ] Update `EmbeddingConfig::validate()` to validate ONNX settings when provider is Onnx
- [ ] Add validation for required fields (model paths exist, valid execution provider)

**Verification:**
```bash
# Test configuration serialization
cargo test --package codeprysm-config test_embedding_config_toml_roundtrip
```

---

## Phase 3: ONNX Provider Implementation (Est: 4-6 hours)

### Task 3.1: Create module file
**New file:** `crates/codeprysm-search/src/embeddings/onnx.rs`

- [ ] Create file with module documentation
- [ ] Add experimental status warning at top of file:
  ```rust
  //! ONNX Runtime embedding provider (EXPERIMENTAL)
  //!
  //! **Status:** Experimental - depends on `ort` crate 2.0.0-rc which is not yet stable.
  //! API may change in future releases.
  ```
- [ ] Add imports:
  - `std::sync::Arc`
  - `async_trait::async_trait`
  - `once_cell::sync::OnceCell`
  - `std::path::PathBuf`
  - `tokenizers::{Tokenizer, PaddingParams, PaddingStrategy}`
  - `tracing::{debug, info}`
- [ ] Add local imports:
  - `crate::error::{Result, SearchError}`
  - `super::provider::{EmbeddingProvider, EmbeddingProviderType, ProviderStatus}`

### Task 3.2: Define configuration types
**File:** `crates/codeprysm-search/src/embeddings/onnx.rs`

- [ ] Define `EMBEDDING_DIM` constant (768)
- [ ] Create `OnnxConfig` struct with fields:
  - `semantic_model_path: PathBuf`
  - `code_model_path: PathBuf`
  - `execution_provider: ExecutionProvider`
  - `device_id: u32`
  - `num_threads: Option<usize>`
- [ ] Create `ExecutionProvider` enum (Cpu, DirectML, OpenVino)
- [ ] Add derive macros: `Debug, Clone, Copy, PartialEq, Eq`

### Task 3.3: Implement OnnxProvider struct
**File:** `crates/codeprysm-search/src/embeddings/onnx.rs`

- [ ] Create `OnnxProvider` struct with `Arc<OnnxProviderInner>`
- [ ] Create `OnnxProviderInner` struct with:
  - `semantic_model: OnceCell<SemanticModel>`
  - `code_model: OnceCell<CodeModel>`
  - `config: OnnxConfig`
- [ ] Create `SemanticModel` struct (session + tokenizer)
- [ ] Create `CodeModel` struct (session + tokenizer)
- [ ] Implement `Clone` for `OnnxProvider`

### Task 3.4: Implement constructor and helpers
**File:** `crates/codeprysm-search/src/embeddings/onnx.rs`

- [ ] Implement `OnnxProvider::new(config: OnnxConfig)` method
- [ ] Implement `OnnxProvider::from_env()` method (reads from environment variables)
- [ ] Implement `ensure_semantic_model(&self)` helper
- [ ] Implement `ensure_code_model(&self)` helper
- [ ] Implement `encode_semantic_sync(&self, texts: &[String])` method
- [ ] Implement `encode_code_sync(&self, texts: &[String])` method

### Task 3.5: Implement EmbeddingProvider trait
**File:** `crates/codeprysm-search/src/embeddings/onnx.rs`

- [ ] Add `#[async_trait]` attribute
- [ ] Implement `encode_semantic()` - use `spawn_blocking`
- [ ] Implement `encode_code()` - use `spawn_blocking`
- [ ] Implement `check_status()` - validate model files exist
- [ ] Implement `warmup()` - preload both models
- [ ] Implement `embedding_dim()` - return 768
- [ ] Implement `provider_type()` - return `EmbeddingProviderType::Onnx`

### Task 3.6: Implement ONNX inference helpers
**File:** `crates/codeprysm-search/src/embeddings/onnx.rs`

- [ ] Implement `validate_execution_provider()` function
- [ ] Implement `execution_provider_name()` function
- [ ] Implement `load_onnx_model()` function:
  - Create ONNX session with execution provider
  - Load tokenizer from model directory
  - Configure session options (threads, optimizations)
- [ ] Implement `run_onnx_inference()` function:
  - Tokenize input texts
  - Run ONNX session forward pass
  - Extract embeddings from output
  - Normalize vectors (L2 norm)
  - Return Vec<Vec<f32>>

### Task 3.7: Add environment variable support
**File:** `crates/codeprysm-search/src/embeddings/onnx.rs`

- [ ] Implement `OnnxProvider::from_env()` to read:
  - `CODEPRYSM_ONNX_SEMANTIC_MODEL_PATH`
  - `CODEPRYSM_ONNX_CODE_MODEL_PATH`
  - `CODEPRYSM_ONNX_EXECUTION_PROVIDER`
  - `CODEPRYSM_ONNX_DEVICE_ID`
  - `CODEPRYSM_ONNX_NUM_THREADS`

**Verification:**
```bash
# Unit tests for OnnxProvider (add to bottom of file)
cargo test --package codeprysm-search onnx::tests --features onnx
```

---

## Phase 4: Cargo Dependencies (Est: 30 min)

### Task 4.1: Add ONNX features to search crate
**File:** `crates/codeprysm-search/Cargo.toml`

- [ ] Add features after line 18:
  ```toml
  # EXPERIMENTAL: ONNX Runtime support (requires ort 2.0.0-rc)
  # API may change. Not recommended for production use.
  onnx = ["dep:ort"]
  onnx-directml = ["onnx", "ort/directml"]
  onnx-openvino = ["onnx", "ort/openvino"]
  ```
- [ ] Add dependency after line 69:
  ```toml
  # ONNX Runtime (optional, EXPERIMENTAL - RC version)
  # Note: Pin to specific RC version to avoid breaking changes
  # Check https://crates.io/crates/ort for latest
  ort = { version = "=2.0.0-rc.11", optional = true, default-features = false }
  ```

**Verification:**
```bash
cargo check --package codeprysm-search --features onnx
cargo check --package codeprysm-search --features onnx-directml
cargo check --package codeprysm-search --features onnx-openvino
```

---

## Phase 5: Feature Propagation (Est: 30 min)

### Task 5.1: Propagate features to backend crate
**File:** `crates/codeprysm-backend/Cargo.toml`

- [ ] Add features after line 48:
  ```toml
  onnx = ["codeprysm-search/onnx"]
  onnx-directml = ["codeprysm-search/onnx-directml"]
  onnx-openvino = ["codeprysm-search/onnx-openvino"]
  ```

### Task 5.2: Propagate features to MCP crate
**File:** `crates/codeprysm-mcp/Cargo.toml`

- [ ] Add features after line 17:
  ```toml
  onnx = ["codeprysm-search/onnx"]
  onnx-directml = ["codeprysm-search/onnx-directml"]
  onnx-openvino = ["codeprysm-search/onnx-openvino"]
  ```

### Task 5.3: Propagate features to CLI crate
**File:** `crates/codeprysm-cli/Cargo.toml`

- [ ] Add features after line 17:
  ```toml
  onnx = ["codeprysm-search/onnx", "codeprysm-backend/onnx", "codeprysm-mcp/onnx"]
  onnx-directml = ["codeprysm-search/onnx-directml", "codeprysm-backend/onnx-directml", "codeprysm-mcp/onnx-directml"]
  onnx-openvino = ["codeprysm-search/onnx-openvino", "codeprysm-backend/onnx-openvino", "codeprysm-mcp/onnx-openvino"]
  ```

**Verification:**
```bash
# Test feature propagation
cargo check --features onnx
cargo check --features onnx-directml
cargo check --features onnx-openvino
```

---

## Phase 6: Factory Integration (Est: 1 hour)

### Task 6.1: Update factory imports
**File:** `crates/codeprysm-search/src/embeddings/factory.rs`

- [ ] Add conditional import at line 13:
  ```rust
  #[cfg(feature = "onnx")]
  use super::onnx::{OnnxConfig as OnnxProviderConfig, OnnxProvider};
  ```

### Task 6.2: Add ONNX field to EmbeddingConfig
**File:** `crates/codeprysm-search/src/embeddings/factory.rs`

- [ ] Add field to `EmbeddingConfig` struct (line 31):
  ```rust
  #[cfg(feature = "onnx")]
  pub onnx: Option<OnnxProviderConfig>,
  ```

### Task 6.3: Add factory methods
**File:** `crates/codeprysm-search/src/embeddings/factory.rs`

- [ ] Add `onnx_with_config()` method after line 78
- [ ] Add `onnx()` method after `onnx_with_config()`

### Task 6.4: Update create() function
**File:** `crates/codeprysm-search/src/embeddings/factory.rs`

- [ ] Add match arm in `create()` function after line 148:
  - Add `#[cfg(feature = "onnx")]` case that instantiates `OnnxProvider`
  - Add `#[cfg(not(feature = "onnx"))]` case that returns error

**Verification:**
```bash
cargo test --package codeprysm-search factory --features onnx
```

---

## Phase 7: CLI Integration (Est: 1 hour)

### Task 7.1: Add config conversion
**File:** `crates/codeprysm-cli/src/commands/mod.rs`

- [ ] Add match arm in `to_search_embedding_config()` after line 186
- [ ] Convert `codeprysm_config::OnnxExecutionProvider` to `codeprysm_search::embeddings::onnx::ExecutionProvider`
- [ ] Map all config fields from config crate to search crate types
- [ ] Handle None case (call `SearchEmbeddingConfig::onnx()`)

**Verification:**
```bash
cargo check --package codeprysm-cli --features onnx
```

---

## Phase 8: Module Exports (Est: 30 min)

### Task 8.1: Export ONNX module
**File:** `crates/codeprysm-search/src/embeddings/mod.rs`

- [ ] Add conditional module declaration after line 44:
  ```rust
  #[cfg(feature = "onnx")]
  pub mod onnx;
  ```
- [ ] Add conditional re-exports after line 60:
  ```rust
  #[cfg(feature = "onnx")]
  pub use onnx::{OnnxConfig, OnnxProvider, ExecutionProvider};
  ```

### Task 8.2: Update config struct
**File:** `crates/codeprysm-config/src/lib.rs`

- [ ] Verify `EmbeddingConfig` struct has `onnx: Option<OnnxSettings>` field (line 67-76)
- [ ] Update struct default implementation if needed

**Verification:**
```bash
cargo doc --package codeprysm-search --features onnx --no-deps --open
```

---

## Testing Phase (Est: 2-3 hours)

### Task 9.1: Write unit tests
**File:** `crates/codeprysm-search/src/embeddings/onnx.rs`

- [ ] Add `#[cfg(test)]` module at bottom of file
- [ ] Test `OnnxProvider::new()` with valid config
- [ ] Test `OnnxProvider::new()` with invalid config
- [ ] Test `validate_execution_provider()` for each provider
- [ ] Test `execution_provider_name()` returns correct strings
- [ ] Test `from_env()` with environment variables set
- [ ] Test `from_env()` with missing environment variables

### Task 9.2: Write integration tests
**New file:** `crates/codeprysm-search/tests/onnx_integration.rs`

- [ ] Create file with `#[cfg(feature = "onnx")]` guard
- [ ] Test semantic encoding with sample inputs
- [ ] Test code encoding with sample inputs
- [ ] Test batch encoding (multiple texts)
- [ ] Test empty input handling
- [ ] Test status check
- [ ] Test warmup preloads models
- [ ] Test embedding dimension is 768

### Task 9.3: Configuration validation tests
**File:** `crates/codeprysm-config/src/lib.rs` (tests section)

- [ ] Test TOML parsing with valid ONNX config
- [ ] Test TOML parsing with invalid ONNX config
- [ ] Test `EmbeddingProviderType::from_str("onnx")`
- [ ] Test config validation with missing model paths
- [ ] Test config validation with invalid execution provider

### Task 9.4: Feature flag tests

- [ ] Verify build succeeds without onnx feature
  ```bash
  cargo build --release
  ```
- [ ] Verify build succeeds with onnx feature
  ```bash
  cargo build --features onnx --release
  ```
- [ ] Verify build succeeds with onnx-directml
  ```bash
  cargo build --features onnx-directml --release
  ```
- [ ] Verify build succeeds with onnx-openvino
  ```bash
  cargo build --features onnx-openvino --release
  ```
- [ ] Test runtime error when ONNX provider used without feature
  ```bash
  # Build without onnx, configure ONNX, expect error
  cargo build --release
  CODEPRYSM_EMBEDDING_PROVIDER=onnx ./target/release/codeprysm init
  # Should show: "ONNX provider not available. Rebuild with --features onnx"
  ```

### Task 9.5: End-to-end test

- [ ] Export Jina models to ONNX format
  ```bash
  python scripts/export_jina_to_onnx.py \
    --model jinaai/jina-embeddings-v2-base-en \
    --output models/jina-semantic.onnx
  python scripts/export_jina_to_onnx.py \
    --model jinaai/jina-embeddings-v2-base-code \
    --output models/jina-code.onnx
  ```
- [ ] Build with ONNX support
  ```bash
  cargo build --features onnx --release
  ```
- [ ] Create test configuration
  ```bash
  cat > ~/.codeprysm/config.toml <<EOF
  [embedding]
  provider = "onnx"

  [embedding.onnx]
  semantic_model_path = "$(pwd)/models/jina-semantic.onnx"
  code_model_path = "$(pwd)/models/jina-code.onnx"
  execution_provider = "cpu"
  EOF
  ```
- [ ] Initialize test repository
  ```bash
  ./target/release/codeprysm init test-repo/
  ```
- [ ] Verify status shows ONNX provider
  ```bash
  ./target/release/codeprysm status
  ```
- [ ] Test search functionality
  ```bash
  ./target/release/codeprysm search "authentication"
  ```
- [ ] Verify results are returned

**Verification Checklist:**
```bash
# Run all tests
cargo test --features onnx
cargo test --features onnx-directml
cargo test --features onnx-openvino

# Check formatting
cargo fmt -- --check

# Check lints
cargo clippy --features onnx -- -D warnings
```

---

## Documentation Phase (Est: 1 hour)

### Task 10.1: Update README.md

- [ ] Add ONNX to list of supported providers with experimental badge
  ```markdown
  - **ONNX Runtime** - CPU, DirectML, OpenVINO backends ⚠️ *Experimental*
  ```
- [ ] Add build instructions for ONNX features
- [ ] Add configuration example for ONNX
- [ ] Add warning about experimental status:
  ```markdown
  > ⚠️ **Experimental:** ONNX provider depends on `ort` crate 2.0.0-rc (release candidate).
  > API may change. Not recommended for production use without testing.
  ```

### Task 10.2: Update CLAUDE.md

- [ ] Add ONNX to GPU acceleration section
- [ ] Document onnx, onnx-directml, onnx-openvino features
- [ ] Add example build commands

### Task 10.3: Create user guide
**New file:** `docs/guides/onnx-provider.md`

- [ ] Add experimental status banner at top of doc
  ```markdown
  # ONNX Runtime Provider (Experimental)

  > ⚠️ **Experimental Feature**
  >
  > The ONNX provider depends on the `ort` crate which is currently at version 2.0.0-rc (release candidate).
  > The API may change before stable release. This feature is not recommended for production use without
  > thorough testing. We recommend using the default LocalProvider (Candle) for production workloads.
  ```
- [ ] Document ONNX provider overview
- [ ] Document execution provider options
- [ ] Provide model export instructions
- [ ] Add troubleshooting section
- [ ] Add performance comparison table
- [ ] Document version pinning strategy for `ort` crate

### Task 10.4: Update CHANGELOG.md

- [ ] Add entry for ONNX provider feature with experimental status:
  ```markdown
  ### Added (Experimental)
  - ONNX Runtime embedding provider with CPU, DirectML, and OpenVINO backends
    - ⚠️ Experimental: Depends on `ort` 2.0.0-rc (release candidate)
    - New features: `onnx`, `onnx-directml`, `onnx-openvino`
    - New config section: `[embedding.onnx]`
    - Recommended for testing only, not production use
  ```
- [ ] Document breaking changes (if any)
- [ ] List new configuration options

---

## Cleanup & Final Review (Est: 1 hour)

### Task 11.1: Code review checklist

- [ ] All files follow existing code style
- [ ] All functions have documentation comments
- [ ] All public APIs have examples
- [ ] Error messages are helpful and actionable
- [ ] No unwrap() or expect() in production code (use proper error handling)
- [ ] All TODOs resolved or documented

### Task 11.2: Performance validation

- [ ] Benchmark CPU execution provider
- [ ] Benchmark DirectML execution provider (if available)
- [ ] Benchmark OpenVINO execution provider (if available)
- [ ] Compare with LocalProvider (Candle) performance
- [ ] Document results in plan or ADR

### Task 11.3: Security review

- [ ] Model paths are validated (no directory traversal)
- [ ] Environment variables are sanitized
- [ ] No secrets logged
- [ ] Input sizes are bounded (prevent DoS)

### Task 11.4: Final verification

- [ ] All tests pass: `cargo test --all-features`
- [ ] No clippy warnings: `cargo clippy --all-features`
- [ ] Code is formatted: `cargo fmt`
- [ ] Documentation builds: `cargo doc --all-features`
- [ ] No broken links in documentation

---

## Post-Implementation

### Task 12.1: Create ADR (Architecture Decision Record)

- [ ] Create `docs/adr/0004-add-onnx-runtime-provider.md`
- [ ] Document why ONNX was chosen
- [ ] List alternatives considered
- [ ] Document trade-offs and consequences
- [ ] Document experimental status and stabilization plan:
  - Current limitations (RC dependency)
  - Path to stability (wait for `ort` 2.0.0 stable)
  - Monitoring strategy for upstream releases
  - Decision criteria for promoting to stable

### Task 12.2: Update roadmap

- [ ] Mark "ONNX Runtime provider (experimental)" as completed in ROADMAP.md
- [ ] Add future enhancements:
  - Stabilize ONNX provider when `ort` 2.0.0 stable is released
  - Model quantization support
  - Additional backends (TensorRT, etc.)
  - Performance optimization

### Task 12.3: Archive plan

- [ ] Move plan from `docs/plans/proposed/` to `docs/plans/completed/`
- [ ] Update status to "Completed" in plan header

---

## Summary

**Total Tasks:** 73
**Estimated Effort:** 10-13 hours
**Critical Path:** Phase 3 (ONNX Provider Implementation) → Phase 6 (Factory) → Phase 9 (Testing)

**Dependencies:**
- ONNX Runtime system libraries
- Jina models exported to ONNX format
- Test fixtures for integration tests
- `ort` crate 2.0.0-rc.x (release candidate)

**Risks:**
- **HIGH:** `ort` crate API changes between RC versions (experimental dependency)
- **HIGH:** Breaking changes before stable 2.0.0 release
- MEDIUM: Platform-specific issues with DirectML/OpenVINO
- MEDIUM: Model export compatibility issues
- LOW: Performance may not match expectations

**Mitigation Strategies:**
- Pin to specific RC version: `ort = "=2.0.0-rc.11"`
- Mark feature as experimental in all documentation
- Monitor `ort` releases and test against new versions
- Provide clear upgrade path when 2.0.0 stable is released
- Recommend LocalProvider (Candle) for production use
