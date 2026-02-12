# Git History Cleanup Script for CodePrysm (PowerShell)
# Removes large model files from entire git history
#
# ⚠️  WARNING: This rewrites git history! Force push required!
# ⚠️  Coordinate with team before running if this is a shared repo!

Write-Host "========================================"
Write-Host "CodePrysm Git History Cleanup"
Write-Host "========================================"
Write-Host ""
Write-Host "This script will remove the following from git history:"
Write-Host "  - models/ directory (~900MB of ONNX models)"
Write-Host "  - onnxruntime_extracted/ directory (~16MB)"
Write-Host "  - *.whl files (ONNX Runtime wheels)"
Write-Host ""

# Get current .git size
$gitSize = (Get-ChildItem .git -Recurse | Measure-Object -Property Length -Sum).Sum / 1GB
Write-Host "Current .git size: $($gitSize.ToString('0.00')) GB"
Write-Host ""

$response = Read-Host "Continue? (y/N)"
if ($response -ne "y" -and $response -ne "Y") {
    Write-Host "Aborted."
    exit 1
}

Write-Host ""
Write-Host "Step 1: Checking for git-filter-repo..."

# Check if git-filter-repo is installed
$filterRepo = Get-Command git-filter-repo -ErrorAction SilentlyContinue

if (-not $filterRepo) {
    Write-Host "git-filter-repo not found. Installing with pip..."

    try {
        pip install --user git-filter-repo
        $env:PATH = "$env:USERPROFILE\.local\bin;$env:PATH"
    }
    catch {
        Write-Host ""
        Write-Host "ERROR: Failed to install git-filter-repo." -ForegroundColor Red
        Write-Host "Please install it manually:"
        Write-Host "  pip install git-filter-repo"
        Write-Host ""
        Write-Host "Or download from: https://github.com/newren/git-filter-repo/releases"
        exit 1
    }
}

# Verify it's now available
$filterRepo = Get-Command git-filter-repo -ErrorAction SilentlyContinue
if (-not $filterRepo) {
    Write-Host "ERROR: git-filter-repo still not found after installation." -ForegroundColor Red
    Write-Host "Please ensure it's in your PATH and try again."
    exit 1
}

Write-Host "✓ git-filter-repo found" -ForegroundColor Green
Write-Host ""

Write-Host "Step 2: Creating backup..."
$backupBranch = "backup-before-cleanup-$(Get-Date -Format 'yyyyMMdd-HHmmss')"
git branch $backupBranch
Write-Host "✓ Created backup branch: $backupBranch" -ForegroundColor Green
Write-Host ""

Write-Host "Step 3: Removing large files from history..."
Write-Host "(This may take a few minutes...)"
git-filter-repo --path models/ --path onnxruntime_extracted/ --path-glob '*.whl' --invert-paths --force

if ($LASTEXITCODE -ne 0) {
    Write-Host ""
    Write-Host "ERROR: git-filter-repo failed!" -ForegroundColor Red
    Write-Host "Restoring from backup: git reset --hard $backupBranch"
    git reset --hard $backupBranch
    exit 1
}

Write-Host ""
Write-Host "✓ Git history cleaned!" -ForegroundColor Green
Write-Host ""

Write-Host "Step 4: Checking results..."
$newGitSize = (Get-ChildItem .git -Recurse | Measure-Object -Property Length -Sum).Sum / 1GB
Write-Host "New .git size: $($newGitSize.ToString('0.00')) GB" -ForegroundColor Cyan
$savedSize = $gitSize - $newGitSize
Write-Host "Saved: $($savedSize.ToString('0.00')) GB" -ForegroundColor Green
Write-Host ""

Write-Host "========================================"
Write-Host "Cleanup Complete!" -ForegroundColor Green
Write-Host "========================================"
Write-Host ""
Write-Host "Next steps:"
Write-Host ""
Write-Host "1. Verify changes:"
Write-Host "   git log --oneline | head -20"
Write-Host ""
Write-Host "2. Test the repository:"
Write-Host "   cargo check"
Write-Host "   just setup"
Write-Host ""
Write-Host "3. Force push to remote (REQUIRED - rewrites history):"
Write-Host "   git push --force-with-lease origin main" -ForegroundColor Yellow
Write-Host ""
Write-Host "4. If you have a fork, update it too:"
Write-Host "   git push --force-with-lease fork main" -ForegroundColor Yellow
Write-Host ""
Write-Host "⚠️  WARNING: All collaborators must re-clone or rebase their work!" -ForegroundColor Red
Write-Host ""
Write-Host "5. If something goes wrong, restore from backup:"
Write-Host "   git reset --hard $backupBranch"
Write-Host ""
