# Parallel File Processing Optimization

## Overview

Parallelize the file processing loop in `GraphBuilder::build_from_directory()` using rayon to achieve 4-8x speedup on multi-core systems.

## Current Bottleneck

The main file processing loop is sequential (lines 288-325 in `crates/codeprysm-core/src/builder.rs`):

```rust
for file_path in files {
    match self.process_file(...) {
        Ok(_) => file_count += 1,
        Err(e) => warn!("Error processing {}: {}", rel_path, e),
    }
}
```

Each file undergoes expensive operations:
- File I/O (read with encoding detection)
- SHA256 hashing
- Tree-sitter parsing (AST construction + query execution)
- Tag extraction and deduplication
- Graph mutations (node/edge creation)

**Impact**: For 1000 files @ 50ms each = 50 seconds total
**With 8-core parallelism**: Could reduce to ~6-7 seconds

## Implementation Strategy

### Step 1: Create Result Container

Add a struct to hold per-file processing results:

```rust
/// Results from processing a single file in parallel.
struct FileProcessingResult {
    /// File path (relative to repository)
    rel_path: String,
    /// Nodes extracted from this file
    nodes: Vec<Node>,
    /// Edges from this file (CONTAINS and DEFINES only)
    edges: Vec<Edge>,
    /// Definitions found in this file (name -> node_id)
    definitions: HashMap<String, String>,
    /// References found in this file (name -> reference info)
    references: HashMap<String, Vec<ReferenceInfo>>,
    /// Number of Data nodes skipped (if skip_data_nodes enabled)
    skipped_data_nodes: usize,
    /// Number of nodes skipped due to depth limit
    skipped_depth_nodes: usize,
}
```

### Step 2: Refactor process_file()

Create a new method that returns results instead of mutating shared state:

```rust
fn process_file_parallel(
    &self,
    file_path: &Path,
    repo_name: &str,
) -> Result<FileProcessingResult, BuilderError> {
    // Detect language
    let language = match SupportedLanguage::from_path(file_path) {
        Some(lang) => lang,
        None => return Err(BuilderError::UnsupportedLanguage),
    };

    // Get relative path
    let rel_path = file_path
        .strip_prefix(repo_name)
        .unwrap_or(file_path)
        .to_string_lossy()
        .to_string();

    // Read file content (with encoding detection)
    let source = read_file_with_encoding(file_path)?;

    // Compute file hash
    let file_hash = compute_file_hash(file_path)?;

    // Count lines
    let line_count = source.lines().count();

    // Create local collections for this file
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut definitions = HashMap::new();
    let mut references = HashMap::new();
    let mut skipped_data_nodes = 0;
    let mut skipped_depth_nodes = 0;

    // Add file container node
    nodes.push(Node::source_file(
        rel_path.clone(),
        rel_path.clone(),
        file_hash,
        line_count,
    ));

    // Add CONTAINS edge from Repository to File
    if !repo_name.is_empty() {
        edges.push(Edge::contains(repo_name.to_string(), rel_path.clone()));
    }

    // Get or create tag extractor for this language
    let mut extractor = match &self.queries_dir {
        Some(dir) => TagExtractor::from_queries_dir(language, dir)?,
        None => TagExtractor::from_embedded(language)?,
    };
    let metadata_extractor = MetadataExtractor::new(language);

    // Extract tags
    let tags = extractor.extract(&source)?;

    // Process tags (same logic as current process_file)
    // ... (extract definitions and references into local collections)

    Ok(FileProcessingResult {
        rel_path,
        nodes,
        edges,
        definitions,
        references,
        skipped_data_nodes,
        skipped_depth_nodes,
    })
}
```

### Step 3: Parallelize File Processing Loop

Replace the sequential loop with rayon's `par_iter()`:

```rust
use rayon::prelude::*;

info!("Found {} files to process", files.len());

// Process files in parallel
let results: Vec<FileProcessingResult> = files
    .par_iter()
    .filter_map(|file_path| {
        // Check max files limit (needs atomic counter for parallel)
        // For now, filter before parallel processing if max_files is set

        match self.process_file_parallel(file_path, &repo_name) {
            Ok(result) => Some(result),
            Err(e) => {
                // Get relative path for error message
                let rel_path = file_path
                    .strip_prefix(directory)
                    .unwrap_or(file_path)
                    .to_string_lossy();
                warn!("Error processing {}: {}", rel_path, e);
                None
            }
        }
    })
    .collect();
```

### Step 4: Merge Results into Main Graph

After parallel processing, merge results sequentially (fast operation):

```rust
// Merge results into main graph
let mut file_count = 0;
let mut skipped_data_nodes = 0;
let mut skipped_depth_nodes = 0;
let mut defines = HashMap::new();
let mut references = HashMap::new();

for result in results {
    // Add all nodes from this file
    for node in result.nodes {
        graph.add_node(node);
    }

    // Add all edges from this file
    for edge in result.edges {
        graph.add_edge_from_struct(&edge);
    }

    // Merge definitions
    defines.extend(result.definitions);

    // Merge references
    for (name, refs) in result.references {
        references.entry(name).or_default().extend(refs);
    }

    // Accumulate statistics
    skipped_data_nodes += result.skipped_data_nodes;
    skipped_depth_nodes += result.skipped_depth_nodes;
    file_count += 1;

    // Progress logging
    if file_count % 100 == 0 {
        debug!("Merged {} files", file_count);
    }
}

info!("Processed {} files", file_count);

// Continue with existing reference resolution
self.resolve_references(&mut graph, &defines, &references);
```

### Step 5: Handle max_files Configuration

For the `max_files` limit, filter before parallel processing:

```rust
let files_to_process: Vec<PathBuf> = if let Some(max) = self.config.max_files {
    files.into_iter().take(max).collect()
} else {
    files
};

info!("Found {} files to process", files_to_process.len());

let results: Vec<FileProcessingResult> = files_to_process
    .par_iter()
    // ... rest of parallel processing
```

## Expected Performance

### Speedup by Core Count

| Cores | Expected Speedup | Example (1000 files @ 50ms) |
|-------|------------------|------------------------------|
| 4     | 2.5-3.5x        | 50s → 14-20s                 |
| 8     | 4-7x            | 50s → 7-12s                  |
| 16    | 6-12x           | 50s → 4-8s                   |

### Memory Impact

- **Current**: Low memory (sequential processing)
- **Parallel**: Higher memory (~N files * avg file result size)
- **Estimate**: For 1000 files, expect 10-50MB additional memory usage
- **Mitigation**: Can batch process if needed (process in chunks of 100-500 files)

## Testing & Verification

The testing strategy follows a phased approach, starting with low-risk unit tests and progressing to full integration and performance validation.

### Phase 1: Unit Tests (START HERE)

**Purpose**: Validate core logic in isolation before touching main processing loop.

**Location**: `crates/codeprysm-core/src/builder.rs` (tests module at line 1702)

**Test Cases**:

1. **FileProcessingResult Construction**
   ```rust
   #[test]
   fn test_file_processing_result_creation() {
       // Create a result with nodes, edges, definitions
       // Verify all fields are properly populated
   }
   ```

2. **Merge Logic - Basic**
   ```rust
   #[test]
   fn test_merge_single_file_result() {
       // Create empty graph
       // Merge one FileProcessingResult
       // Verify nodes and edges are added correctly
   }

   #[test]
   fn test_merge_multiple_file_results() {
       // Merge results from 3 files
       // Verify node count = sum of all result nodes
       // Verify edge count = sum of all result edges
   }
   ```

3. **Merge Logic - Definitions and References**
   ```rust
   #[test]
   fn test_merge_definitions_no_conflicts() {
       // Two files with different definitions
       // Verify both definitions are in merged map
   }

   #[test]
   fn test_merge_definitions_with_conflicts() {
       // Two files defining same name (last one wins)
       // Verify expected definition is kept
   }

   #[test]
   fn test_merge_references_aggregation() {
       // Multiple files referencing same symbol
       // Verify references are aggregated into vector
   }
   ```

4. **Result Ordering and Determinism**
   ```rust
   #[test]
   fn test_merge_order_independence() {
       // Create same results in different orders
       // Merge both orderings
       // Verify final graphs are identical (node IDs, edge count)
   }
   ```

5. **Statistics Accumulation**
   ```rust
   #[test]
   fn test_statistics_accumulation() {
       // Create results with skipped_data_nodes and skipped_depth_nodes
       // Verify statistics are summed correctly
   }
   ```

6. **Error Handling**
   ```rust
   #[test]
   fn test_parallel_error_handling() {
       // Simulate file processing errors
       // Verify errors are logged but don't stop processing
       // Verify successful files are still merged
   }
   ```

**Run Unit Tests**:
```bash
# Run all builder unit tests
cargo test --package codeprysm-core --lib builder

# Run specific test
cargo test --package codeprysm-core --lib builder::tests::test_merge_single_file_result
```

**Success Criteria**:
- All new unit tests pass
- No existing unit tests broken
- Code coverage for merge logic >80%

### Phase 2: Integration Tests

**Purpose**: Verify the parallel implementation produces identical results to sequential version.

1. **Existing Test Suite**
   ```bash
   # Run all tests
   just rust-test

   # Run integration tests
   just rust-test-integration

   # Run language-specific integration tests
   just rust-test-integration-lang python
   just rust-test-integration-lang rust
   ```

2. **Graph Determinism**: Verify parallel output matches sequential output
   - Compare node counts, edge counts
   - Check specific node IDs and relationships
   - Run `just stats` before and after on same repository

3. **Repository Tests**: Test on various codebases
   - Small repo (10-100 files)
   - Medium repo (1000-5000 files)
   - Large repo (10000+ files)

**Success Criteria**:
- All existing integration tests pass
- Graph statistics match sequential version exactly
- No new warnings or errors in logs

### Phase 3: Regression Testing

**Purpose**: Ensure edge cases and error conditions still work correctly.

Scenarios to verify (many covered by existing tests):
- Empty directories
- Single file
- Unsupported file types mixed with supported
- Files with parse errors
- Very large files (>1MB)
- Deeply nested directory structures
- Special characters in filenames
- Symlinks (should be skipped by WalkBuilder)
- max_files configuration limit

**Test Files**:
- `tests/edge_cases.rs` - Edge case scenarios
- `tests/equivalence.rs` - Sequential vs parallel equivalence
- `tests/graph_completeness.rs` - Graph structure validation

**Success Criteria**:
- All edge case tests pass
- Error messages are clear and helpful
- No panics or crashes

### Phase 4: Performance Testing

1. **Baseline Measurement**:
   ```bash
   time just init /path/to/repo
   # Record: total time, files/sec
   ```

2. **After Optimization**:
   ```bash
   time just init /path/to/repo
   # Compare: total time, speedup ratio, CPU utilization
   ```

3. **Metrics to Track**:
   - Total processing time
   - Files processed per second
   - CPU utilization (should be 400-800% with 4-8 cores)
   - Memory usage (monitor with `time -v` or Activity Monitor)
   - Graph statistics (should be identical)

### Regression Testing

Ensure these scenarios still work correctly:
- Empty directories
- Single file
- Unsupported file types mixed with supported
- Files with parse errors
- Very large files (>1MB)
- Deeply nested directory structures
- Special characters in filenames
- Symlinks (should be skipped by WalkBuilder)

## Configuration Options (Optional)

Consider adding parallel processing controls to `BuilderConfig`:

```rust
pub struct BuilderConfig {
    // ... existing fields ...

    /// Number of threads for parallel file processing
    /// None = use rayon default (num_cpus)
    pub num_threads: Option<usize>,

    /// Process files in batches to limit memory usage
    /// None = process all files at once
    pub batch_size: Option<usize>,
}
```

Usage:
```rust
// Limit to 4 threads
let config = BuilderConfig {
    num_threads: Some(4),
    ..Default::default()
};

// Process in batches of 500 files
let config = BuilderConfig {
    batch_size: Some(500),
    ..Default::default()
};
```

## Risks & Mitigations

### Risk 1: Non-Deterministic Output
**Issue**: Parallel processing might change node/edge ordering
**Mitigation**:
- Files are pre-sorted before processing
- Merge results in deterministic order
- Test output matches sequential version exactly

### Risk 2: Memory Usage Spike
**Issue**: Buffering all results before merge increases memory
**Mitigation**:
- Monitor memory in tests
- Add batch processing if needed
- Document memory requirements

### Risk 3: Error Handling Complexity
**Issue**: Parallel errors are harder to debug
**Mitigation**:
- Log each file error with full path
- Continue processing remaining files
- Collect all errors for final report

### Risk 4: Parser Thread Safety
**Issue**: Tree-sitter parsers may not be thread-safe
**Mitigation**:
- Create new parser per file (current approach)
- Each parallel task gets its own TagExtractor
- Rayon handles thread safety automatically

## Implementation Checklist

### Code Implementation
- [ ] Add `FileProcessingResult` struct to builder.rs
- [ ] Implement `process_file_parallel()` method
- [ ] Replace sequential loop with `par_iter()`
- [ ] Implement result merging logic
- [ ] Handle `max_files` configuration

### Testing (Phased Approach)
- [ ] **Phase 1: Unit Tests** (START HERE)
  - [ ] Test FileProcessingResult creation
  - [ ] Test merge logic for single result
  - [ ] Test merge logic for multiple results
  - [ ] Test definitions merge (no conflicts)
  - [ ] Test definitions merge (with conflicts)
  - [ ] Test references aggregation
  - [ ] Test result ordering independence
  - [ ] Test statistics accumulation
  - [ ] Test error handling in parallel context
  - [ ] Verify all unit tests pass: `cargo test --package codeprysm-core --lib builder`
- [ ] **Phase 2: Integration Tests**
  - [ ] Run full test suite: `just rust-test`
  - [ ] Run integration tests: `just rust-test-integration`
  - [ ] Test on small repository (10-100 files)
  - [ ] Test on medium repository (1000-5000 files)
  - [ ] Verify graph statistics match sequential version
- [ ] **Phase 3: Regression Tests**
  - [ ] Verify edge cases pass: `cargo test --package codeprysm-core --test edge_cases`
  - [ ] Test empty directories, single files
  - [ ] Test files with parse errors
  - [ ] Test max_files configuration
- [ ] **Phase 4: Performance Tests**
  - [ ] Baseline measurement on real repository
  - [ ] Benchmark parallel implementation
  - [ ] Verify 4-8x speedup on multi-core systems
  - [ ] Monitor memory usage (expect 10-50MB increase)

### Documentation
- [ ] Document performance gains in README
- [ ] Update CLAUDE.md with performance notes
- [ ] Add comments explaining parallel processing strategy

## Files Modified

- **`crates/codeprysm-core/src/builder.rs`** (PRIMARY)
  - Lines 242-355: `build_from_directory()` method
  - Add `FileProcessingResult` struct
  - Add `process_file_parallel()` method
  - Replace sequential loop with rayon parallel processing
  - Add merge logic

- **`crates/codeprysm-core/Cargo.toml`** (NO CHANGE)
  - rayon is already a dependency

- **Tests** (NEW)
  - Add parallel processing tests
  - Add merge logic tests
  - Verify deterministic output

## Future Enhancements

After parallel processing is working:

1. **Parser Pooling**: Reuse TagExtractor instances per language per thread
2. **I/O Optimization**: Share file bytes between hash and parse operations
3. **Batch Processing**: Add automatic batching for very large repositories
4. **Progress Reporting**: Add parallel-safe progress updates every N files
5. **Incremental Parallelization**: Extend to incremental updates as well

## References

- rayon documentation: https://docs.rs/rayon
- Tree-sitter thread safety: https://tree-sitter.github.io/tree-sitter/
- Current implementation: `crates/codeprysm-core/src/builder.rs:242-355`
