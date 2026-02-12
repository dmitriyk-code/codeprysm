#!/bin/bash
# Git History Cleanup Script for CodePrysm
# Removes large model files from entire git history
#
# ⚠️  WARNING: This rewrites git history! Force push required!
# ⚠️  Coordinate with team before running if this is a shared repo!

set -e

echo "========================================"
echo "CodePrysm Git History Cleanup"
echo "========================================"
echo ""
echo "This script will remove the following from git history:"
echo "  - models/ directory (~900MB of ONNX models)"
echo "  - onnxruntime_extracted/ directory (~16MB)"
echo "  - *.whl files (ONNX Runtime wheels)"
echo ""
echo "Current .git size: $(du -sh .git | cut -f1)"
echo ""
read -p "Continue? (y/N): " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Aborted."
    exit 1
fi

echo ""
echo "Step 1: Checking for git-filter-repo..."

# Check if git-filter-repo is installed
if ! command -v git-filter-repo &> /dev/null; then
    echo "git-filter-repo not found. Installing..."

    # Try pip install
    if command -v pip3 &> /dev/null; then
        pip3 install --user git-filter-repo
    elif command -v pip &> /dev/null; then
        pip install --user git-filter-repo
    else
        echo ""
        echo "ERROR: pip not found. Please install git-filter-repo manually:"
        echo "  macOS: brew install git-filter-repo"
        echo "  Linux: pip3 install git-filter-repo"
        echo "  Windows: pip install git-filter-repo"
        exit 1
    fi

    # Add to PATH if needed
    export PATH="$HOME/.local/bin:$PATH"
fi

if ! command -v git-filter-repo &> /dev/null; then
    echo "ERROR: git-filter-repo still not found after installation."
    echo "Please install it manually and ensure it's in your PATH."
    exit 1
fi

echo "✓ git-filter-repo found"
echo ""

echo "Step 2: Creating backup..."
BACKUP_BRANCH="backup-before-cleanup-$(date +%Y%m%d-%H%M%S)"
git branch "$BACKUP_BRANCH"
echo "✓ Created backup branch: $BACKUP_BRANCH"
echo ""

echo "Step 3: Removing large files from history..."
git-filter-repo --path models/ --path onnxruntime_extracted/ --path-glob '*.whl' --invert-paths --force

echo ""
echo "✓ Git history cleaned!"
echo ""

echo "Step 4: Checking results..."
echo "New .git size: $(du -sh .git | cut -f1)"
echo ""

echo "========================================"
echo "Cleanup Complete!"
echo "========================================"
echo ""
echo "Next steps:"
echo ""
echo "1. Verify changes:"
echo "   git log --oneline | head -20"
echo ""
echo "2. Test the repository:"
echo "   cargo check"
echo "   just setup"
echo ""
echo "3. Force push to remote (REQUIRED - rewrites history):"
echo "   git push --force-with-lease origin main"
echo ""
echo "4. If you have a fork, update it too:"
echo "   git push --force-with-lease fork main"
echo ""
echo "⚠️  WARNING: All collaborators must re-clone or rebase their work!"
echo ""
echo "5. If something goes wrong, restore from backup:"
echo "   git reset --hard $BACKUP_BRANCH"
echo ""
