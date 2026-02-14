# Setting Up CodePrysm MCP with Claude Code (CLI)

This guide shows you how to configure CodePrysm as an MCP server for Claude Code (the command-line interface), enabling AI-powered code exploration and research.

## Prerequisites

1. **Claude Code CLI** installed and configured
2. **CodePrysm** installed with ONNX DirectML (or your preferred feature):
   ```bash
   # Windows with GPU acceleration
   cargo install codeprysm-cli --features onnx-directml --force

   # macOS with Metal
   cargo install codeprysm-cli --features metal --force

   # Linux with CUDA
   cargo install codeprysm-cli --features cuda --force

   # CPU only
   cargo install codeprysm-cli --force
   ```
3. **Docker** for running Qdrant
4. **Your repository** with code graph already generated

## Step 1: Start Qdrant

CodePrysm requires Qdrant for semantic search. Start it with Docker:

```bash
docker run -d --name qdrant \
  -p 6333:6333 -p 6334:6334 \
  -v qdrant_storage:/qdrant/storage \
  qdrant/qdrant:latest
```

Verify it's running:
```bash
curl http://localhost:6333/health
# Should return: {"title":"qdrant - vector search engine","version":"..."}
```

## Step 2: Initialize Your Repository

Navigate to your codebase and generate the code graph:

```bash
cd /path/to/your/repository
codeprysm init
```

This will:
- Parse all supported source files using Tree-sitter
- Build a graph of code entities and relationships
- Generate semantic embeddings
- Index everything in Qdrant
- Create `.codeprysm/` directory (automatically added to `.gitignore`)

**First-time notes:**
- Initial indexing may take 5-20 minutes for large repos
- Embedding models (~400-500MB) will be downloaded once to `~/.cache/huggingface/hub/`
- With ONNX DirectML, you should see: "ONNX DirectML feature detected"

## Step 3: Configure Claude Code MCP Settings

### Find Your Claude Code Configuration Directory

Claude Code stores its configuration in:
- **Linux/macOS**: `~/.claude/` or `~/.config/claude/`
- **Windows**: `%APPDATA%\.claude\` or `%USERPROFILE%\.claude\`

Create the directory if it doesn't exist:
```bash
mkdir -p ~/.claude
```

### Create MCP Server Configuration

Create or edit `~/.claude/mcp_settings.json`:

```json
{
  "mcpServers": {
    "codeprysm": {
      "command": "codeprysm",
      "args": [
        "mcp",
        "--root",
        "/absolute/path/to/your/repository",
        "--qdrant-url",
        "http://localhost:6334"
      ],
      "env": {
        "RUST_LOG": "info"
      }
    }
  }
}
```

**Important:**
- Replace `/absolute/path/to/your/repository` with the actual path to your repo
- Use **absolute paths**, not relative paths like `./` or `~/`
- On Windows, use forward slashes: `C:/Users/yourname/projects/myrepo`

### Multiple Repositories

To work with multiple repositories, add more entries:

```json
{
  "mcpServers": {
    "codeprysm-project-a": {
      "command": "codeprysm",
      "args": ["mcp", "--root", "/path/to/project-a"]
    },
    "codeprysm-project-b": {
      "command": "codeprysm",
      "args": ["mcp", "--root", "/path/to/project-b"]
    }
  }
}
```

Claude Code will connect to all configured servers simultaneously.

## Step 4: Verify the Configuration

### Test MCP Server Manually

Before using it with Claude Code, verify the MCP server works:

```bash
cd /path/to/your/repository
codeprysm mcp --root . --qdrant-url http://localhost:6334
```

You should see output like:
```
INFO Initializing CodePrysm MCP server
INFO   Repository: /path/to/your/repository
INFO   CodePrysm dir: /path/to/your/repository/.codeprysm
INFO ONNX DirectML feature detected, using ONNX provider with DirectML
INFO Using embedding provider: Onnx
INFO Connected to Qdrant for hybrid search
INFO Preloading embedding models...
INFO Embedding models preloaded successfully
```

**Key indicators:**
- ✅ Should show "ONNX DirectML feature detected" (if built with `--features onnx-directml`)
- ✅ Should show "Connected to Qdrant for hybrid search"
- ✅ Should show "Embedding models preloaded successfully"
- ❌ Should NOT show "GPU not available, falling back to CPU" (old bug)

Press `Ctrl+C` to stop the test server.

### Test with Claude Code

Start a new Claude Code session:

```bash
claude
```

At the prompt, ask a question about your codebase:

```
> Tell me about the authentication logic in this codebase
```

Or be more specific with tool usage:

```
> Use the codeprysm MCP server to search for "authentication" and explain the results
```

## Step 5: Using CodePrysm for Code Research

### Available MCP Tools

Once configured, Claude Code has access to these CodePrysm tools:

| Tool | Purpose | Example Query |
|------|---------|---------------|
| `search_graph_nodes` | Find code by name or concept | "Find all authentication functions" |
| `get_node_info` | Get metadata about a code entity | "Show info for UserService class" |
| `read_code` | Read source code with context | "Read the login method implementation" |
| `find_references` | Who calls this? (incoming edges) | "What calls the parseConfig function?" |
| `find_outgoing_references` | What does this call? (dependencies) | "What does AuthMiddleware depend on?" |
| `find_definitions` | What does this contain? | "What methods does User class have?" |
| `find_call_chain` | Trace execution paths | "Trace the login flow downstream" |
| `find_module_structure` | Explore directory organization | "Show structure of src/api directory" |
| `sync_repository` | Refresh index after code changes | "Re-index the repository" |
| `get_index_status` | Check indexing status | "What's the index status?" |

### Search Modes

CodePrysm supports three search modes:

1. **`mode=code`** - For identifiers and function names
   ```
   Search for "UserService" class
   Search for "parseConfig" function
   ```

2. **`mode=info`** - For concepts and descriptions
   ```
   Find authentication logic
   Find error handling code
   Where is the database configured?
   ```

3. **`mode=hybrid`** (default) - Combines both
   ```
   Find login-related code
   Show me user management features
   ```

### Example Queries for Code Research

**Understanding a new codebase:**
```
1. "What's the structure of the src/ directory?"
2. "Find all API endpoints"
3. "Show me the main entry point"
4. "What authentication mechanisms are used?"
```

**Investigating a feature:**
```
1. "Find the user login function"
2. "What calls the login function?"
3. "What does the login function call?"
4. "Trace the full login flow from request to response"
```

**Analyzing dependencies:**
```
1. "What does the AuthMiddleware class depend on?"
2. "Find all references to the database connection"
3. "What files use the Logger class?"
```

**Exploring architecture:**
```
1. "Show the structure of the authentication module"
2. "List all classes in the services directory"
3. "Find all error handler functions"
```

## Step 6: Keeping the Index Updated

After making code changes, update the index:

```bash
cd /path/to/your/repository
codeprysm update
```

This is much faster than `codeprysm init` because it only processes changed files.

**Or** use the MCP tool from Claude Code:
```
> Sync the codeprysm repository index
```

## Advanced Configuration

### Environment Variables

You can customize behavior with environment variables:

```json
{
  "mcpServers": {
    "codeprysm": {
      "command": "codeprysm",
      "args": ["mcp", "--root", "/path/to/repo"],
      "env": {
        "RUST_LOG": "debug",
        "CODEPRYSM_ONNX_EXECUTION_PROVIDER": "directml",
        "CODEPRYSM_ONNX_DEVICE_ID": "0"
      }
    }
  }
}
```

**Available variables:**
- `RUST_LOG` - Log level (`error`, `warn`, `info`, `debug`, `trace`)
- `CODEPRYSM_ONNX_EXECUTION_PROVIDER` - Force provider (`directml`, `openvino`, `cpu`)
- `CODEPRYSM_ONNX_DEVICE_ID` - GPU device ID (default: 0)
- `CODEPRYSM_ONNX_NUM_THREADS` - CPU threads for inference

### Custom Qdrant Instance

If running Qdrant on a different host/port:

```json
{
  "mcpServers": {
    "codeprysm": {
      "command": "codeprysm",
      "args": [
        "mcp",
        "--root", "/path/to/repo",
        "--qdrant-url", "http://192.168.1.100:6334"
      ]
    }
  }
}
```

### Custom CodePrysm Directory

By default, the graph is stored in `.codeprysm/`. To use a different location:

```json
{
  "mcpServers": {
    "codeprysm": {
      "command": "codeprysm",
      "args": [
        "mcp",
        "--root", "/path/to/repo",
        "--codeprysm-dir", "/custom/path/.codeprysm"
      ]
    }
  }
}
```

## Troubleshooting

### Issue: "MCP server not found"

**Solution:** Verify `codeprysm` is in your PATH:
```bash
which codeprysm  # Linux/macOS
where codeprysm  # Windows
```

If not found, add Cargo's bin directory to PATH:
```bash
export PATH="$HOME/.cargo/bin:$PATH"  # Add to ~/.bashrc or ~/.zshrc
```

### Issue: "Connection to Qdrant failed"

**Solution:** Check if Qdrant is running:
```bash
docker ps | grep qdrant
curl http://localhost:6334/health
```

Restart if needed:
```bash
docker restart qdrant
```

### Issue: "No code graph found"

**Solution:** Initialize the repository:
```bash
cd /path/to/repo
codeprysm init
```

### Issue: "Still showing CPU fallback with DirectML"

**Solution:** Rebuild with the fix:
```bash
cargo install codeprysm-cli --features onnx-directml --force
```

Verify the binary location:
```bash
which codeprysm
# Should show: ~/.cargo/bin/codeprysm (or similar)
```

### Issue: "Search returns no results"

**Possible causes:**
1. Index is empty or outdated
   ```bash
   codeprysm update
   ```

2. Qdrant collections don't exist
   ```bash
   codeprysm init --force  # Rebuild index
   ```

3. Query is too specific - try broader terms

### Issue: "Models downloading every time"

Models should be cached in `~/.cache/huggingface/hub/`. If they re-download:

1. Check cache directory exists and is writable
2. Verify disk space (models are ~500MB each)
3. Check network connectivity to huggingface.co

## Performance Tips

### GPU Acceleration

- **Windows**: Use `--features onnx-directml` for Intel Arc, AMD, or NVIDIA GPUs
- **macOS**: Use `--features metal` for Apple Silicon
- **Linux**: Use `--features cuda` for NVIDIA GPUs

### Large Repositories

For repos with 100K+ files:
- Initial indexing: 10-30 minutes
- Incremental updates: 1-5 minutes
- Use `codeprysm update` instead of `codeprysm init` after the first run

### Memory Usage

- Small repos (<1K files): ~500MB RAM
- Medium repos (1K-10K files): ~2GB RAM
- Large repos (10K-100K files): ~8GB RAM

## Example Workflow

Here's a complete workflow for researching a new codebase:

```bash
# 1. Start Qdrant
docker start qdrant || docker run -d --name qdrant -p 6334:6334 qdrant/qdrant

# 2. Initialize the repository
cd /path/to/new-codebase
codeprysm init

# 3. Configure Claude Code (edit ~/.claude/mcp_settings.json)
# Add the codeprysm server configuration

# 4. Start Claude Code
claude
```

Then in Claude Code:
```
> I'm exploring a new codebase. Can you help me understand its structure?

> Start by showing me the directory structure of src/

> Find the main entry point

> What authentication mechanisms are implemented?

> Trace the user login flow from start to finish

> What external APIs does this application call?

> Show me all database models
```

## Next Steps

- [MCP Server Architecture](./features/mcp-server-architecture.md)
- [Semantic Search Best Practices](./features/semantic-search-guide.md)
- [Contributing to CodePrysm](../CONTRIBUTING.md)
