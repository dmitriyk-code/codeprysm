# Git History Cleanup - Model Files

This directory contains scripts to remove large model files from git history.

## Problem

The `models/` directory (~900MB of ONNX models) was previously committed to git, bloating the repository size to 1.4GB. These models are now auto-downloaded from HuggingFace Hub at runtime.

## What Gets Removed

- `models/` directory (ONNX model files: jina-semantic.onnx, jina-code.onnx, tokenizers)
- `onnxruntime_extracted/` directory (ONNX Runtime binaries)
- `*.whl` files (Python wheel files)

## Prerequisites

- **Backup your work**: Commit and push all local changes
- **git-filter-repo**: Will be auto-installed by the script (requires Python pip)
- **Coordination**: If working with a team, ensure everyone is aware of the force push

## Usage

### Windows (PowerShell)

```powershell
.\cleanup-git-history.ps1
```

### macOS/Linux (Bash)

```bash
chmod +x cleanup-git-history.sh
./cleanup-git-history.sh
```

## What Happens

1. **Backup**: Creates a backup branch with timestamp
2. **Cleanup**: Removes specified paths from entire git history
3. **Verification**: Shows before/after .git directory size
4. **Instructions**: Provides next steps for force-pushing

## Expected Results

- `.git` directory reduces from ~1.4GB to ~100-200MB
- All commits remain, but model files are removed from history
- Working directory unchanged (models/ already in .gitignore)

## After Running

### 1. Verify the Cleanup

```bash
# Check repo size
du -sh .git

# Verify models are gone from history
git log --all --full-history -- models/

# Test the repository works
cargo check
```

### 2. Force Push (REQUIRED)

⚠️ **This rewrites history - coordinate with team first!**

```bash
# Push to origin
git push --force-with-lease origin main

# Push to fork (if you have one)
git push --force-with-lease fork main
```

### 3. Notify Collaborators

All collaborators must either:
- **Fresh clone** (recommended): `git clone <repo>`
- **Rebase their work**: See "For Collaborators" section below

## For Collaborators

After the force push, collaborators need to update their local repos:

### Option 1: Fresh Clone (Easiest)

```bash
cd ..
mv codeprysm codeprysm-old
git clone https://github.com/codeprysm/codeprysm.git
cd codeprysm
# Copy over any uncommitted work from codeprysm-old
```

### Option 2: Reset Local Branches

```bash
# Save your work first!
git stash

# Update from remote
git fetch origin
git reset --hard origin/main

# Restore your work
git stash pop
```

### Option 3: Rebase (Advanced)

```bash
git fetch origin
git rebase origin/main
# Resolve any conflicts
```

## Troubleshooting

### Script Fails

If the cleanup script fails:

```bash
# Restore from backup
git reset --hard backup-before-cleanup-YYYYMMDD-HHMMSS

# Check what went wrong
git status
```

### Need to Undo After Push

If you need to revert after force-pushing:

```bash
# Find the commit before cleanup (on backup branch)
git log backup-before-cleanup-YYYYMMDD-HHMMSS

# Force push the backup
git push --force origin backup-before-cleanup-YYYYMMDD-HHMMSS:main
```

## Alternative: Using BFG Repo-Cleaner

If git-filter-repo doesn't work, you can use BFG instead:

```bash
# Download BFG
wget https://repo1.maven.org/maven2/com/madgag/bfg/1.14.0/bfg-1.14.0.jar

# Run cleanup
java -jar bfg-1.14.0.jar --delete-folders models --delete-folders onnxruntime_extracted .
java -jar bfg-1.14.0.jar --delete-files "*.whl" .

# Clean up
git reflog expire --expire=now --all
git gc --prune=now --aggressive
```

## Why This Matters

- **Faster clones**: 1.2GB saved per clone
- **Better workflows**: CI/CD pipelines run faster
- **Best practices**: Binary files don't belong in git
- **Auto-download**: Models download from HuggingFace on first use

## References

- [git-filter-repo documentation](https://github.com/newren/git-filter-repo)
- [BFG Repo-Cleaner](https://rtyley.github.io/bfg-repo-cleaner/)
- [HuggingFace Hub cache](https://huggingface.co/docs/huggingface_hub/guides/manage-cache)
