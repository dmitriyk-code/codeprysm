# Claude Code + CodePrysm MCP - Quick Reference

## Configuration File

**Location:** `~/.claude/mcp_settings.json`

```json
{
  "mcpServers": {
    "codeprysm": {
      "command": "codeprysm",
      "args": ["mcp", "--root", "/absolute/path/to/your/repo"],
      "env": {
        "RUST_LOG": "info"
      }
    }
  }
}
```

## Quick Start

```bash
# 1. Start Qdrant
docker run -d --name qdrant -p 6334:6334 qdrant/qdrant

# 2. Initialize repository
cd /path/to/repo && codeprysm init

# 3. Start Claude Code
claude
```

## Example Questions for Claude Code

### Understanding Structure
```
- "What's the structure of the src/ directory?"
- "Show me the main entry point"
- "List all API endpoints"
```

### Finding Code
```
- "Find authentication logic"
- "Search for 'UserService' class"
- "Where is the database configured?"
```

### Analyzing Relationships
```
- "What calls the login function?"
- "What does AuthMiddleware depend on?"
- "Trace the user registration flow"
```

### Exploring Features
```
- "Show me all error handlers"
- "What methods does the User class have?"
- "Find all database models"
```

## Available MCP Tools

| Tool | What it does |
|------|-------------|
| `search_graph_nodes` | Find code by name or concept |
| `find_references` | Who calls this? |
| `find_outgoing_references` | What does this call? |
| `find_definitions` | What's inside this? |
| `find_call_chain` | Trace execution paths |
| `read_code` | View source code |
| `get_node_info` | Get entity metadata |
| `find_module_structure` | Explore directories |
| `sync_repository` | Update after code changes |

## Search Modes

- **`mode=code`** - For function/class names (exact matching)
- **`mode=info`** - For concepts (semantic matching)
- **`mode=hybrid`** - Default, combines both

## Keeping Index Updated

```bash
# After making code changes
codeprysm update
```

Or from Claude Code:
```
> Sync the codeprysm repository
```

## Troubleshooting

### Check Qdrant
```bash
docker ps | grep qdrant
curl http://localhost:6334/health
```

### Check CodePrysm
```bash
which codeprysm
codeprysm --version
```

### Rebuild Index
```bash
cd /path/to/repo
codeprysm init --force
```

## Installation (with fix)

```bash
# Windows with GPU
cargo install codeprysm-cli --features onnx-directml --force

# macOS with GPU
cargo install codeprysm-cli --features metal --force

# Linux with GPU
cargo install codeprysm-cli --features cuda --force
```

## Verification

When MCP server starts, you should see:
```
✅ "ONNX DirectML feature detected" (with --features onnx-directml)
✅ "Connected to Qdrant for hybrid search"
✅ "Embedding models preloaded successfully"
```

NOT:
```
❌ "GPU not available, falling back to CPU" (old bug, now fixed)
```
