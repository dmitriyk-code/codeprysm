# ONNX Provider Fix: Visual Comparison

## Before Fix (INCORRECT)

```
Input: ["hello world", "authentication logic"]
                ↓
         [Tokenization]
                ↓
    ┌───────────────────────┐
    │ ONNX Model Inference  │
    │                       │
    │ Output:               │
    │ [2, 5, 768]          │  ← [batch_size, seq_length, hidden_dim]
    │                       │
    │ 2 sequences           │
    │ 5 tokens each         │
    │ 768 dimensions        │
    └───────────────────────┘
                ↓
    ❌ WRONG: Direct slice extraction

    for i in 0..batch_size:
        embedding = output[i*768..(i+1)*768]

    This extracts the FIRST 768 values for batch 0,
    then the NEXT 768 values (likely first token of batch 1),
    completely ignoring sequence structure!
                ↓
         [L2 Normalize]
                ↓
    Result: Meaningless embeddings ❌
```

## After Fix (CORRECT)

```
Input: ["hello world", "authentication logic"]
                ↓
         [Tokenization]
         + Attention Mask
                ↓
    ┌───────────────────────┐
    │ ONNX Model Inference  │
    │                       │
    │ Output:               │
    │ [2, 5, 768]          │  ← [batch_size, seq_length, hidden_dim]
    │                       │
    │ 2 sequences           │
    │ 5 tokens each         │
    │ 768 dimensions        │
    └───────────────────────┘
                ↓
    ✅ CORRECT: Mean pooling with attention mask

    for i in 0..batch_size:
        sum = [0.0; 768]
        mask_sum = 0.0

        for j in 0..seq_length:
            if attention_mask[i][j] > 0:  # Skip padding
                for k in 0..hidden_dim:
                    sum[k] += output[i][j][k] * mask[i][j]
                mask_sum += mask[i][j]

        embedding = sum / mask_sum  # Average non-padding tokens
                ↓
         [L2 Normalize]
                ↓
    Result: Semantic embeddings ✅
```

## Example with Real Data

### Input Sequence
```
Text: "hello world"
Tokens: ["hello", "world", "<pad>", "<pad>", "<pad>"]
Attention Mask: [1, 1, 0, 0, 0]
```

### ONNX Model Output Shape
```
[batch_size=1, seq_length=5, hidden_dim=768]

Conceptually:
[
    [  # Batch 0
        [0.1, 0.2, ..., 0.9],  # Token 0: "hello" → 768 dims
        [0.3, 0.4, ..., 0.8],  # Token 1: "world" → 768 dims
        [0.0, 0.0, ..., 0.0],  # Token 2: <pad>   → 768 dims
        [0.0, 0.0, ..., 0.0],  # Token 3: <pad>   → 768 dims
        [0.0, 0.0, ..., 0.0],  # Token 4: <pad>   → 768 dims
    ]
]
```

### Before Fix (WRONG)
```rust
// Extract first 768 values
embedding = output_data[0..768]

// This gives you the "hello" token embedding ONLY
// Result: [0.1, 0.2, ..., 0.9]
// Then normalize it

❌ Problem: Only uses first token, ignores "world"!
```

### After Fix (CORRECT)
```rust
// Mean pool with attention mask
sum = [0.0; 768]
mask_sum = 0.0

// Token 0: "hello" (mask=1)
for k in 0..768:
    sum[k] += output[0][0][k] * 1.0  // Add "hello" embedding
mask_sum += 1.0

// Token 1: "world" (mask=1)
for k in 0..768:
    sum[k] += output[0][1][k] * 1.0  // Add "world" embedding
mask_sum += 1.0

// Tokens 2-4: <pad> (mask=0) - SKIPPED

// Average
embedding = sum / mask_sum  // Divide by 2
// Result: Average of "hello" + "world" embeddings

✅ Correct: Uses ALL non-padding tokens!
```

## Key Differences

| Aspect | Before (WRONG) | After (CORRECT) |
|--------|----------------|-----------------|
| **Output shape handling** | Assumes flat `[batch * 768]` | Properly handles 3D `[batch, seq, 768]` |
| **Pooling strategy** | None (direct slice) | Mean pooling with attention mask |
| **Padding handling** | Includes padding in embeddings | Masks out padding tokens |
| **Token coverage** | Only first 768 values (likely first token) | All non-padding tokens averaged |
| **Semantic quality** | ❌ Low - incomplete information | ✅ High - full sequence context |

## Why This Matters

### Search Example
```
Query: "authentication function"
Code: "def authenticate_user(username, password):"
```

**Before Fix:**
- ONNX might only encode "def" (first token)
- Missing crucial semantic information
- Poor search ranking

**After Fix:**
- ONNX encodes entire function signature
- Captures "authenticate", "user", "username", "password"
- Correct search ranking

## Mathematical Correctness

### Candle (Reference Implementation)
```python
# From local.rs mean_pool() function
attention_mask_expanded = attention_mask.unsqueeze(2)  # [batch, seq, 1]
sum_mask = attention_mask_expanded.sum(1)              # [batch, 1]
masked_embeddings = embeddings * attention_mask_expanded
summed = masked_embeddings.sum(1)                      # [batch, hidden]
pooled = summed / sum_mask                             # [batch, hidden]
```

### ONNX (After Fix)
```rust
// Equivalent implementation in Rust
for i in 0..batch_size {
    let mut sum = vec![0.0; hidden_dim];
    let mut mask_sum = 0.0;

    for j in 0..seq_length {
        let mask_val = attention_mask[i * seq_length + j];
        if mask_val > 0.0 {
            for k in 0..hidden_dim {
                sum[k] += output[i][j][k] * mask_val;  // Weighted by mask
            }
            mask_sum += mask_val;
        }
    }

    // Average: sum / mask_sum
    for val in &mut sum {
        *val /= mask_sum;
    }
}
```

Both implementations are now **mathematically equivalent**! ✅

## Performance Notes

The mean pooling adds minimal overhead:
- **Complexity**: O(batch_size × seq_length × hidden_dim)
- **Typical values**: 1 × 512 × 768 = ~393K operations
- **Time**: < 1ms on CPU, negligible on GPU
- **Benefit**: Correct embeddings → Accurate search results

This is a **quality fix**, not an optimization. The processing must be done correctly even if it takes slightly longer.
