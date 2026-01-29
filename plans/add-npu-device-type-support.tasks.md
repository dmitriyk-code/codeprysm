# Add NPU and Configurable Device Type Support - Task List

**Date:** 2026-01-28
**Plan:** `plans/add-npu-device-type-support.md`
**Status:** Ready for Implementation

## Task Breakdown

### Phase 1: Configuration Layer (codeprysm-config)

#### Task 1.1: Add device_type field to OnnxSettings struct
- **File:** `crates/codeprysm-config/src/lib.rs`
- **Line:** ~285 (OnnxSettings struct)
- **Changes:**
  - Add `pub device_type: Option<String>` field to `OnnxSettings`
  - Update `Default` implementation to set `device_type: Some("GPU_FP32".to_string())`
  - Update struct documentation with device type examples
- **Acceptance:**
  - [ ] Field added to struct
  - [ ] Default value set to `"GPU_FP32"` for backward compatibility
  - [ ] Documentation updated
  - [ ] Code compiles without errors

#### Task 1.2: Update OnnxSettings validation (if needed)
- **File:** `crates/codeprysm-config/src/lib.rs`
- **Line:** ~86-120 (EmbeddingConfig::validate)
- **Changes:**
  - Add validation for `device_type` values (optional, but good practice)
  - Valid values: CPU, GPU, GPU_FP32, GPU_FP16, NPU, AUTO, MULTI:*
- **Acceptance:**
  - [ ] Invalid device types produce clear error messages (optional)
  - [ ] Validation tests pass

#### Task 1.3: Add device_type to config examples in documentation
- **File:** `crates/codeprysm-config/src/lib.rs`
- **Line:** ~65-68 (Example TOML in doc comments)
- **Changes:**
  - Add `device_type = "NPU"` to example configuration
  - Add comment explaining device type options
- **Acceptance:**
  - [ ] Documentation example includes device_type
  - [ ] Comment explains available options

### Phase 2: Search Layer (codeprysm-search)

#### Task 2.1: Add device_type field to OnnxConfig struct
- **File:** `crates/codeprysm-search/src/embeddings/onnx.rs`
- **Line:** ~61 (OnnxConfig struct)
- **Changes:**
  - Add `pub device_type: Option<String>` field
  - Update `Default` implementation (line ~75) to set `device_type: Some("GPU_FP32".to_string())`
- **Acceptance:**
  - [ ] Field added to struct
  - [ ] Default value matches codeprysm-config
  - [ ] Code compiles

#### Task 2.2: Add CODEPRYSM_ONNX_DEVICE_TYPE environment variable support
- **File:** `crates/codeprysm-search/src/embeddings/onnx.rs`
- **Line:** ~145 (OnnxProvider::from_env method)
- **Changes:**
  - Add `let device_type = std::env::var("CODEPRYSM_ONNX_DEVICE_TYPE").ok().or_else(|| Some("GPU_FP32".to_string()));`
  - Pass `device_type` to `OnnxConfig` construction
- **Acceptance:**
  - [ ] Environment variable is read
  - [ ] Default value used when env var not set
  - [ ] Value passed to config struct

#### Task 2.3: Update create_session to use configurable device_type
- **File:** `crates/codeprysm-search/src/embeddings/onnx.rs`
- **Line:** ~403-424 (ExecutionProvider::OpenVino match arm)
- **Changes:**
  - Replace hardcoded `"GPU_FP32"` with `config.device_type.as_deref().unwrap_or("GPU_FP32")`
  - Update info! log message to include device type
  - Update error message to include device type
- **Acceptance:**
  - [ ] Hardcoded value removed
  - [ ] Config value used instead
  - [ ] Log messages show actual device type
  - [ ] Code compiles with `--features onnx-openvino`

#### Task 2.4: Pass device_type through OnnxProvider::new
- **File:** `crates/codeprysm-search/src/embeddings/onnx.rs`
- **Line:** ~120-140 (OnnxProvider::new method)
- **Changes:**
  - Ensure `config.device_type` is stored in provider
  - Pass to both semantic and code model session creation
- **Acceptance:**
  - [ ] device_type properly propagated through provider
  - [ ] Both models use same device_type

#### Task 2.5: Update module documentation
- **File:** `crates/codeprysm-search/src/embeddings/onnx.rs`
- **Line:** ~1-19 (module doc comment)
- **Changes:**
  - Add NPU to supported execution providers list
  - Add device type configuration example
  - Mention NPU requirements (Core Ultra, drivers, etc.)
- **Acceptance:**
  - [ ] Documentation mentions NPU support
  - [ ] Example shows device_type configuration

#### Task 2.6: Add tests for device_type configuration
- **File:** `crates/codeprysm-search/src/embeddings/onnx.rs`
- **Line:** ~600+ (tests module)
- **Changes:**
  - Add `test_device_type_configuration()` test
  - Add `test_device_type_defaults()` test
  - Add `test_device_type_from_env()` test (if possible without actual models)
- **Acceptance:**
  - [ ] Tests verify device_type field works
  - [ ] Tests verify default value
  - [ ] All tests pass

### Phase 3: Factory Layer (codeprysm-search)

#### Task 3.1: Update OnnxProviderConfig in factory
- **File:** `crates/codeprysm-search/src/embeddings/factory.rs`
- **Line:** Find OnnxProviderConfig struct (if it exists)
- **Changes:**
  - Add `pub device_type: Option<String>` field
  - Pass through to OnnxProvider::new
- **Acceptance:**
  - [ ] Factory config includes device_type
  - [ ] Value passed through to provider
  - [ ] Code compiles with `--features onnx`

### Phase 4: CLI Layer (codeprysm-cli)

#### Task 4.1: Update CLI config conversion for device_type
- **File:** `crates/codeprysm-cli/src/commands/mod.rs`
- **Line:** Find `to_search_embedding_config()` function, ONNX match arm
- **Changes:**
  - Add `device_type: onnx_settings.device_type.clone()` to OnnxConfig construction
  - Ensure device_type is passed when creating EmbeddingConfig
- **Acceptance:**
  - [ ] device_type from config file reaches ONNX provider
  - [ ] CLI compiles with `--features onnx`
  - [ ] Config file changes are respected

### Phase 5: Documentation

#### Task 5.1: Update CLAUDE.md with NPU configuration
- **File:** `CLAUDE.md`
- **Line:** GPU Acceleration section (if exists) or create new section
- **Changes:**
  - Add "Intel NPU (AI Boost)" subsection
  - Add OpenVINO Device Types reference table
  - Add configuration examples for NPU, AUTO, MULTI, FP16
  - Add requirements section (Core Ultra, drivers, etc.)
- **Acceptance:**
  - [ ] NPU configuration documented
  - [ ] Device types reference table added
  - [ ] Examples are complete and correct

#### Task 5.2: Update implementation_status.md
- **File:** `implementation_status.md`
- **Line:** Add new section or update ONNX configuration section
- **Changes:**
  - Document device_type field addition
  - List supported device types
  - Note NPU support status
- **Acceptance:**
  - [ ] implementation_status.md reflects new capability
  - [ ] Device types are listed

#### Task 5.3: Update README.md features list
- **File:** `README.md`
- **Line:** Features section
- **Changes:**
  - Mention Intel NPU support
  - Add to hardware acceleration section
- **Acceptance:**
  - [ ] README mentions NPU support
  - [ ] Hardware acceleration section updated

### Phase 6: Integration Testing

#### Task 6.1: Manual testing - Config file
- **Test:** Create config with device_type and verify it's used
- **Steps:**
  1. Create `~/.codeprysm/config.toml` with NPU device_type
  2. Build with `--features onnx-openvino --release`
  3. Run with log level info to see device selection
  4. Verify log shows correct device type
- **Acceptance:**
  - [ ] Config file device_type is read
  - [ ] Correct device type shown in logs
  - [ ] No errors during initialization

#### Task 6.2: Manual testing - Environment variables
- **Test:** Set CODEPRYSM_ONNX_DEVICE_TYPE env var and verify
- **Steps:**
  1. Set `CODEPRYSM_ONNX_DEVICE_TYPE=NPU`
  2. Run codeprysm-cli
  3. Check logs for device type
- **Acceptance:**
  - [ ] Environment variable is respected
  - [ ] Device type shown in logs
  - [ ] Overrides default value

#### Task 6.3: Manual testing - Default behavior
- **Test:** Verify default GPU_FP32 when no device_type specified
- **Steps:**
  1. Use config without device_type field
  2. Run and check logs
- **Acceptance:**
  - [ ] Defaults to GPU_FP32
  - [ ] Backward compatible with existing configs

#### Task 6.4: Manual testing - Invalid device type
- **Test:** Test with invalid device type value
- **Steps:**
  1. Set device_type to "INVALID_DEVICE"
  2. Run and observe error handling
- **Acceptance:**
  - [ ] Clear error message shown
  - [ ] Application doesn't crash
  - [ ] Error suggests valid values (bonus)

#### Task 6.5: Manual testing - NPU on compatible hardware
- **Test:** Run on Intel Core Ultra with NPU
- **Steps:**
  1. Configure device_type = "NPU"
  2. Run embedding generation
  3. Monitor Task Manager for NPU activity
  4. Compare performance vs CPU/GPU
- **Acceptance:**
  - [ ] NPU shows activity in Task Manager
  - [ ] Embeddings generated successfully
  - [ ] Performance metrics collected

#### Task 6.6: Manual testing - AUTO device selection
- **Test:** Test AUTO device type
- **Steps:**
  1. Configure device_type = "AUTO"
  2. Run and observe which device OpenVINO selects
  3. Check logs for selection
- **Acceptance:**
  - [ ] AUTO mode works
  - [ ] Best device is selected
  - [ ] Log shows which device was chosen

#### Task 6.7: Manual testing - GPU_FP16 precision
- **Test:** Test FP16 mode for faster inference
- **Steps:**
  1. Configure device_type = "GPU_FP16"
  2. Run embedding generation
  3. Compare speed vs GPU_FP32
- **Acceptance:**
  - [ ] FP16 mode works
  - [ ] Faster than FP32 (expected)
  - [ ] Embeddings still reasonable quality

### Phase 7: Cleanup and Polish

#### Task 7.1: Code review and cleanup
- **Review:**
  - Check all error messages are clear
  - Verify log messages are helpful
  - Ensure consistent naming (device_type vs deviceType)
  - Remove any debug code
- **Acceptance:**
  - [ ] Code follows project conventions
  - [ ] No debug logging left
  - [ ] Error messages are user-friendly

#### Task 7.2: Run full test suite
- **Test:** Ensure no regressions
- **Commands:**
  ```bash
  cargo test --workspace
  cargo test --package codeprysm-search --features onnx --release
  cargo test --package codeprysm-config
  ```
- **Acceptance:**
  - [ ] All existing tests pass
  - [ ] New tests pass
  - [ ] No warnings introduced

#### Task 7.3: Build verification across all feature combinations
- **Test:** Verify all feature combinations compile
- **Commands:**
  ```bash
  cargo check --package codeprysm-search
  cargo check --package codeprysm-search --features onnx
  cargo check --package codeprysm-search --features onnx-directml
  cargo check --package codeprysm-search --features onnx-openvino
  cargo check --package codeprysm-cli --features onnx --release
  cargo check --package codeprysm-cli --features onnx-openvino --release
  ```
- **Acceptance:**
  - [ ] All feature combinations compile
  - [ ] Release builds succeed
  - [ ] No warnings on any combination

#### Task 7.4: Update plan status
- **File:** `plans/add-npu-device-type-support.md`
- **Changes:**
  - Mark status as "Completed"
  - Add completion date
  - Document any deviations from plan
  - Add lessons learned section
- **Acceptance:**
  - [ ] Plan marked as complete
  - [ ] Status updated

## Summary

**Total Tasks:** 28
- Configuration Layer: 3 tasks
- Search Layer: 6 tasks
- Factory Layer: 1 task
- CLI Layer: 1 task
- Documentation: 3 tasks
- Integration Testing: 7 tasks
- Cleanup: 4 tasks
- Verification: 3 tasks

**Estimated Effort:** 4-6 hours
- Core implementation: 2-3 hours
- Testing: 1-2 hours
- Documentation: 1 hour

**Dependencies:**
- Must be done after ONNX provider implementation (already complete ✅)
- Requires system with OpenVINO support for testing
- NPU testing requires Intel Core Ultra hardware

**Risk Mitigation:**
- Backward compatibility maintained via default values
- Feature-gated code prevents compilation issues
- Comprehensive testing plan covers edge cases

## Quick Reference: Files to Modify

1. `crates/codeprysm-config/src/lib.rs` (3 tasks)
2. `crates/codeprysm-search/src/embeddings/onnx.rs` (6 tasks)
3. `crates/codeprysm-search/src/embeddings/factory.rs` (1 task)
4. `crates/codeprysm-cli/src/commands/mod.rs` (1 task)
5. `CLAUDE.md` (1 task)
6. `implementation_status.md` (1 task)
7. `README.md` (1 task)
8. Manual testing (7 tasks)
9. Cleanup and verification (4 tasks)

## Checklist Summary

Copy this checklist when starting implementation:

```markdown
### Configuration
- [ ] Add device_type to OnnxSettings (config)
- [ ] Update default values
- [ ] Add validation (optional)

### Search Provider
- [ ] Add device_type to OnnxConfig
- [ ] Add environment variable support
- [ ] Update create_session logic
- [ ] Update documentation
- [ ] Add tests

### Integration
- [ ] Update factory config
- [ ] Update CLI conversion
- [ ] Verify feature propagation

### Documentation
- [ ] Update CLAUDE.md
- [ ] Update implementation_status.md
- [ ] Update README.md

### Testing
- [ ] Test config file
- [ ] Test env variables
- [ ] Test defaults
- [ ] Test invalid values
- [ ] Test on NPU hardware (if available)
- [ ] Test AUTO mode
- [ ] Test FP16 mode

### Verification
- [ ] All tests pass
- [ ] All feature combinations build
- [ ] No regressions
- [ ] Documentation complete
```
