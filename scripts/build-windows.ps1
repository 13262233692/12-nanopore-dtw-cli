
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

Write-Host "==========================================" -ForegroundColor Cyan
Write-Host "  Nanopore DTW CLI - Windows Build Script" -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan
Write-Host ""

$ProjectDir = Split-Path -Parent $PSScriptRoot
Set-Location $ProjectDir

$Targets = @(
    "x86_64-pc-windows-msvc"
)

$BuildType = if ($args.Count -gt 0) { $args[0] } else { "release" }
$OutputDir = Join-Path $ProjectDir "dist"

New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

Write-Host "Build type: $BuildType"
Write-Host "Output directory: $OutputDir"
Write-Host ""

foreach ($target in $Targets) {
    Write-Host ""
    Write-Host "Building for target: $target" -ForegroundColor Yellow
    Write-Host "----------------------------------------"

    cargo build --target $target --profile $BuildType --features "static simd"

    $binName = "nanopore-dtw.exe"
    $binPath = Join-Path $ProjectDir "target\$target\$BuildType\$binName"

    if (Test-Path $binPath) {
        $distDir = Join-Path $OutputDir $target
        New-Item -ItemType Directory -Force -Path $distDir | Out-Null

        Copy-Item $binPath (Join-Path $distDir $binName) -Force

        $size = (Get-Item (Join-Path $distDir $binName)).Length
        $sizeMB = [math]::Round($size / 1MB, 2)
        Write-Host "Built successfully: $distDir\$binName ($sizeMB MB)" -ForegroundColor Green
    } else {
        Write-Host "Build failed for $target" -ForegroundColor Red
    }
}

Write-Host ""
Write-Host "==========================================" -ForegroundColor Cyan
Write-Host "  Build Complete" -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan
