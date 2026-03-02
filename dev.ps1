#!/usr/bin/env pwsh
# Hemulator development shell
# Usage: .\dev.ps1 [command] [args...]
#
# Commands:
#   build      Build (release-quick, default)
#   run        Build and run the emulator (pass ROM path as extra arg)
#   test       Run all workspace tests
#   clippy     Run clippy with -D warnings
#   fmt        Format all code
#   check      fmt check + clippy + build + test (full CI)
#   trace      Run with instruction tracing (pass ROM and optional --breakpoint)
#   dump       Run headless debug dump (pass ROM and --debug-dump-pc or --debug-dump-cycles)
#   clean      cargo clean
#   help       Show this help

param(
    [Parameter(Position=0)]
    [string]$Command = "help",
    [Parameter(Position=1, ValueFromRemainingArguments=$true)]
    [string[]]$Args
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $Root

function Write-Header([string]$msg) {
    Write-Host ""
    Write-Host "=== $msg ===" -ForegroundColor Cyan
}

function Invoke-Check([string]$label, [scriptblock]$block) {
    Write-Header $label
    & $block
    if ($LASTEXITCODE -ne 0) {
        Write-Host "FAILED: $label" -ForegroundColor Red
        exit $LASTEXITCODE
    }
    Write-Host "OK" -ForegroundColor Green
}

function Show-Help {
    Write-Host ""
    Write-Host "Hemulator Dev Shell" -ForegroundColor Yellow
    Write-Host "===================" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "  .\dev.ps1 build                    Build workspace (release-quick)"
    Write-Host "  .\dev.ps1 run game.gba             Run emulator with a ROM"
    Write-Host "  .\dev.ps1 test                     Run all tests"
    Write-Host "  .\dev.ps1 clippy                   Clippy -D warnings"
    Write-Host "  .\dev.ps1 fmt                      Format code"
    Write-Host "  .\dev.ps1 check                    Full CI: fmt+clippy+build+test"
    Write-Host "  .\dev.ps1 trace game.gba           Run with instruction tracing"
    Write-Host "  .\dev.ps1 trace game.gba --breakpoint 0x8000"
    Write-Host "  .\dev.ps1 dump game.gba --debug-dump-pc 0x8000"
    Write-Host "  .\dev.ps1 dump game.gba --debug-dump-cycles 50000"
    Write-Host "  .\dev.ps1 cpu game.gba              Run with CPU trace logging"
    Write-Host "  .\dev.ps1 cpu game.gba debug        Run with CPU debug logging"
    Write-Host "  .\dev.ps1 gpu game.gba              Run with PPU trace logging"
    Write-Host "  .\dev.ps1 gpu game.gba debug        Run with PPU debug logging"
    Write-Host "  .\dev.ps1 clean                    Clean build artifacts"
    Write-Host ""
}

switch ($Command.ToLower()) {

    "build" {
        Write-Header "Building (release-quick)"
        cargo build --profile release-quick @Args
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
        Write-Host "Build OK" -ForegroundColor Green
    }

    "run" {
        Write-Header "Building and running"
        cargo build --profile release-quick
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
        $exe = "$Root\target\release-quick\hemu.exe"
        if (-not (Test-Path $exe)) { $exe = "$Root\target\release-quick\hemu" }
        Write-Host "Running: $exe $Args" -ForegroundColor Gray
        & $exe @Args
    }

    "test" {
        Write-Header "Running tests"
        cargo test --workspace @Args
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
        Write-Host "All tests passed" -ForegroundColor Green
    }

    "clippy" {
        Write-Header "Clippy"
        cargo clippy --workspace --all-targets -- -D warnings @Args
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
        Write-Host "Clippy clean" -ForegroundColor Green
    }

    "fmt" {
        Write-Header "Formatting"
        cargo fmt --all @Args
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
        Write-Host "Formatted" -ForegroundColor Green
    }

    "check" {
        Invoke-Check "Format check" { cargo fmt --all -- --check }
        Invoke-Check "Clippy" { cargo clippy --workspace --all-targets -- -D warnings }
        Invoke-Check "Build" { cargo build --profile release-quick }
        Invoke-Check "Tests" { cargo test --workspace }
        Write-Host ""
        Write-Host "All checks passed!" -ForegroundColor Green
    }

    "trace" {
        Write-Header "Building for trace run"
        cargo build --profile release-quick
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
        $exe = "$Root\target\release-quick\hemu.exe"
        if (-not (Test-Path $exe)) { $exe = "$Root\target\release-quick\hemu" }
        # Default trace limit 10000; override by passing --trace-limit N in $Args
        $traceArgs = @("--trace-instructions")
        if ($Args -notcontains "--trace-limit") { $traceArgs += "--trace-limit", "10000" }
        Write-Host "Running: $exe $traceArgs $Args" -ForegroundColor Gray
        & $exe @traceArgs @Args
    }

    "dump" {
        Write-Header "Building for headless dump"
        cargo build --profile release-quick
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
        $exe = "$Root\target\release-quick\hemu.exe"
        if (-not (Test-Path $exe)) { $exe = "$Root\target\release-quick\hemu" }
        Write-Host "Running: $exe $Args" -ForegroundColor Gray
        & $exe @Args
    }

    "cpu" {
        Write-Header "Building for CPU debug run"
        cargo build --profile release-quick
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
        $exe = "$Root\target\release-quick\hemu.exe"
        if (-not (Test-Path $exe)) { $exe = "$Root\target\release-quick\hemu" }
        # First positional arg after 'cpu' is the ROM, rest are extra flags
        # Default log level is 'trace'; pass a level as second arg to override
        $rom = $null
        $level = "trace"
        $extra = @()
        $gotRom = $false
        $gotLevel = $false
        foreach ($a in $Args) {
            if (-not $gotRom -and $a -notlike "--*") {
                $rom = $a; $gotRom = $true
            } elseif ($gotRom -and -not $gotLevel -and $a -notlike "--*") {
                $level = $a; $gotLevel = $true
            } else {
                $extra += $a
            }
        }
        if (-not $rom) {
            Write-Host "Usage: .\dev.ps1 cpu <rom> [level] [extra flags...]" -ForegroundColor Red
            Write-Host "  level: trace (default), debug, info, warn, error" -ForegroundColor Gray
            exit 1
        }
        $cpuArgs = @("--log-cpu", $level, $rom) + $extra
        Write-Host "Running: $exe $cpuArgs" -ForegroundColor Gray
        & $exe @cpuArgs
    }

    "gpu" {
        Write-Header "Building for GPU/PPU debug run"
        cargo build --profile release-quick
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
        $exe = "$Root\target\release-quick\hemu.exe"
        if (-not (Test-Path $exe)) { $exe = "$Root\target\release-quick\hemu" }
        $rom = $null
        $level = "trace"
        $extra = @()
        $gotRom = $false
        $gotLevel = $false
        foreach ($a in $Args) {
            if (-not $gotRom -and $a -notlike "--*") {
                $rom = $a; $gotRom = $true
            } elseif ($gotRom -and -not $gotLevel -and $a -notlike "--*") {
                $level = $a; $gotLevel = $true
            } else {
                $extra += $a
            }
        }
        if (-not $rom) {
            Write-Host "Usage: .\dev.ps1 gpu <rom> [level] [extra flags...]" -ForegroundColor Red
            Write-Host "  level: trace (default), debug, info, warn, error" -ForegroundColor Gray
            exit 1
        }
        $gpuArgs = @("--log-ppu", $level, $rom) + $extra
        Write-Host "Running: $exe $gpuArgs" -ForegroundColor Gray
        & $exe @gpuArgs
    }

    "clean" {
        Write-Header "Cleaning"
        cargo clean @Args
        Write-Host "Clean done" -ForegroundColor Green
    }

    { $_ -in "help", "--help", "-h", "" } {
        Show-Help
    }

    default {
        Write-Host "Unknown command: $Command" -ForegroundColor Red
        Show-Help
        exit 1
    }
}
