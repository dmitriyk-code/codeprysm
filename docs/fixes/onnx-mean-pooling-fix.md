# ONNX Provider Mean Pooling Fix

## Date
2026-02-12

## Issue
The ONNX embedding provider was producing different and lower quality search results compared to the Candle (LocalProvider), despite using the same Jina embedding models.

## Root Cause
The ONNX provider had **three critical bugs** in its embedding extraction logic:

### 1. Missing Mean Pooling
The ONNX provider was directly extracting embeddings from the model output without applying mean pooling. It assumed it could simply slice `[batch_size * EMBEDDING_DIM]` values from the output, which was incorrect.

**Original approach (incorrect)**:
```rust
// Assumed flat output [batch_size * EMBEDDING_DIM]
let start_idx = i * EMBEDDING_DIM;
let end_idx = start_idx + EMBEDDING_DIM;
let embedding = output_data[start_idx..end_idx].to_vec();
```

This accidentally extracted some data but **did not perform mean pooling** across the sequence.

### 2. Wrong Output Shape Assumption
The code assumed the model output was already pooled to `[batch_size, EMBEDDING_DIM]`, but BERT/Jina models actually output `[batch_size, seq_length, hidden_dim]` (last_hidden_state).

### 3. No Attention Mask Usage
The provider wasn't using the attention mask during pooling, so padding tokens polluted the embeddings.

## Solution

Updated `encode_with_onnx()` function in `crates/codeprysm-search/src/embeddings/onnx.rs` to:

1. **Correctly extract 3D output tensor**: `[batch_size, seq_length, hidden_dim]`
2. **Apply mean pooling with attention mask**: Replicates the `mean_pool()` logic from Candle's LocalProvider
3. **Apply L2 normalization**: Same as before, but now on properly pooled embeddings

### Key Changes

```rust
// Store attention mask as f32 for pooling calculations
let attention_mask_f32: Vec<f32> = attention_mask_flat.iter().map(|&x| x as f32).collect();

// Extract and validate 3D output shape
let (shape, output_data): (_, &[f32]) = output_tensor.try_extract_tensor()?;
let shape_dims = shape.as_ref();
let batch_size_out = shape_dims[0] as usize;
let seq_length_out = shape_dims[1] as usize;
let hidden_dim = shape_dims[2] as usize;

// Apply mean pooling with attention mask (matching Candle's approach)
for i in 0..batch_size {
    let seq_start = i * seq_length * hidden_dim;
    let mut sum = vec![0.0f32; hidden_dim];
    let mut mask_sum = 0.0f32;

    for j in 0..seq_length {
        let mask_val = attention_mask_f32[i * seq_length + j];
        if mask_val > 0.0 {
            let token_start = seq_start + j * hidden_dim;
            for k in 0..hidden_dim {
                sum[k] += output_data[token_start + k] * mask_val;
            }
            mask_sum += mask_val;
        }
    }

    // Average by dividing by sum of mask values
    if mask_sum > 0.0 {
        for val in &mut sum {
            *val /= mask_sum;
        }
    }

    // L2 normalization
    let norm: f32 = sum.iter().map(|x| x * x).sum::<f32>().sqrt();
    let normalized: Vec<f32> = if norm > 0.0 {
        sum.iter().map(|x| x / norm).collect()
    } else {
        sum
    };

    embeddings.push(normalized);
}
```

## Processing Pipeline Comparison

### Before (INCORRECT)
1. Tokenize input
2. Run ONNX inference
3. **Extract first EMBEDDING_DIM values** (no mean pooling)
4. L2 normalize
5. Return embeddings

### After (CORRECT - matches Candle)
1. Tokenize input
2. Run ONNX inference
3. **Extract 3D output tensor `[batch_size, seq_length, hidden_dim]`**
4. **Apply mean pooling with attention mask** (average non-padding tokens)
5. L2 normalize
6. Return embeddings

## Verification

The code compiles successfully:
```bash
cargo build --package codeprysm-search --features onnx --release
```

Base tests pass without ONNX:
```bash
cargo test --package codeprysm-search --lib
```

**Note**: ONNX tests have a Windows-specific linker issue with CRT (static vs dynamic) that is unrelated to these changes.

## Expected Impact

After this fix:
- ✅ ONNX provider will produce semantically equivalent embeddings to Candle
- ✅ Search quality will be comparable between providers
- ✅ Embeddings will respect attention masks (properly handle padding)
- ✅ The processing pipeline matches the reference implementation

## Testing Recommendations

1. **Re-index with ONNX provider**:
   ```bash
   codeprysm init --provider onnx
   ```

2. **Compare search results**:
   - Run identical queries with both providers
   - Results should be similar in quality and ranking

3. **Optional: Direct embedding comparison**:
   ```rust
   let text = "function to calculate fibonacci";
   let candle_emb = candle_provider.encode_semantic(vec![text]).await?;
   let onnx_emb = onnx_provider.encode_semantic(vec![text]).await?;
   let similarity = cosine_similarity(&candle_emb[0], &onnx_emb[0]);
   assert!(similarity > 0.95); // Should be very similar
   ```

## Files Modified

- `crates/codeprysm-search/src/embeddings/onnx.rs` - Fixed `encode_with_onnx()` function

## References

- Original implementation: `crates/codeprysm-search/src/embeddings/local.rs` (`mean_pool()` and `normalize_l2()`)
- Jina Embeddings v2 documentation: https://huggingface.co/jinaai/jina-embeddings-v2-base-en
