#!/usr/bin/env pwsh
# build.ps1 - Full build with quality checks
# Exit codes: 0=success, 1=test failure, 2=clippy failure,
#   3=coverage failure, 4=build failure

param(
    [Parameter(Position = 0)]
    [ValidateSet(
        "build", "build-only", "dev", "test", "clippy",
        "coverage", "validate",
        "deploy", "deploy-setup", "clean", "help"
    )]
    [string]$Command = "build",
    [switch]$Help
)

if ($Help -or $Command -eq "help") {
    Write-Host @"
Usage: .\build.ps1 [command]

Commands:
  build         Full build with all quality checks (default)
  build-only    Build release binaries only
  dev           Start backend with default dev settings
  test          Run all Rust tests
  clippy        Run clippy linter
  coverage      Generate HTML coverage report
  validate      Run cargo xtask validate
  deploy-setup  One-time RPi provisioning (user, dirs, service)
  deploy        Build and deploy to the RPi
  clean         Clean build artifacts
  help          Show this help
"@
    exit 0
}

function Invoke-Build {
    Invoke-Validate
    Invoke-BuildOnly
    Write-Host "Build OK"
}

function Invoke-BuildOnly {
    cargo build --release
    if ($LASTEXITCODE -ne 0) { exit 4 }
}

function Get-BackendPort {
    $portsFile = Join-Path $PSScriptRoot ".ports"
    if (-not (Test-Path $portsFile)) { return 3100 }
    foreach ($line in Get-Content $portsFile) {
        $trimmed = $line.Trim()
        if (-not $trimmed -or $trimmed.StartsWith("#")) { continue }
        if ($trimmed -match '^backend_port\s*=\s*(\d+)') {
            return [int]$Matches[1]
        }
    }
    return 3100
}

function Invoke-Dev {
    Write-Host "Building backend..."
    cargo build -p bellwether-web
    if ($LASTEXITCODE -ne 0) { exit 4 }

    $backendPort = Get-BackendPort
    $backendExe = "target/debug/bellwether-web.exe"
    Write-Host "Starting backend on :$backendPort..."
    Write-Host "Open http://localhost:$backendPort"
    & $backendExe --dev --port $backendPort
}

function Invoke-Test {
    cargo xtask test
    if ($LASTEXITCODE -ne 0) { exit 1 }
}

function Invoke-Clippy {
    cargo xtask clippy
    if ($LASTEXITCODE -ne 0) { exit 2 }
}

function Invoke-Coverage {
    cargo llvm-cov --workspace --html
    if ($LASTEXITCODE -ne 0) { exit 3 }
    Write-Host "Coverage: target/llvm-cov/html/index.html"
}

function Invoke-Validate {
    cargo xtask validate
    if ($LASTEXITCODE -ne 0) { exit 3 }
}

function Invoke-Deploy {
    cargo xtask deploy
    if ($LASTEXITCODE -ne 0) { exit 4 }
}

function Invoke-DeploySetup {
    cargo xtask deploy-setup
    if ($LASTEXITCODE -ne 0) { exit 4 }
}

function Invoke-Clean {
    cargo clean
    foreach ($f in @(
        "coverage.xml", "coverage.json"
    )) {
        if (Test-Path $f) { Remove-Item $f }
    }
    Write-Host "Clean OK"
}

switch ($Command) {
    "build"        { Invoke-Build }
    "build-only"   { Invoke-BuildOnly }
    "dev"          { Invoke-Dev }
    "test"         { Invoke-Test }
    "clippy"       { Invoke-Clippy }
    "coverage"     { Invoke-Coverage }
    "validate"     { Invoke-Validate }
    "deploy"       { Invoke-Deploy }
    "deploy-setup" { Invoke-DeploySetup }
    "clean"        { Invoke-Clean }
}
