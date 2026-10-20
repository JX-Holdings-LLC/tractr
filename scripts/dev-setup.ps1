<#
.SYNOPSIS
Sets up a Windows machine to build, test and run everything in this repository, then checks the
result. Safe to run again: it installs only what is missing.

.DESCRIPTION
Installs with winget: the Visual Studio Build Tools with the C++ workload (Rust's MSVC linker),
rustup, the .NET 10 SDK and Node 22. Then installs the Rust toolchain rust-toolchain.toml pins and
restores the dotnet tools (Fable). global.json pins the .NET SDK version and rolls forward to
newer feature bands, so winget's .NET 10 SDK is accepted.

  scripts\dev-setup.ps1           install what is missing
  scripts\dev-setup.ps1 -Check    install nothing; report what is present and what is missing
  scripts\dev-setup.ps1 -Full     also wasm-bindgen-cli (version from Cargo.lock) and cargo-about

Linux and macOS: scripts/dev-setup.sh.
#>
param(
    [switch]$Check,
    [switch]$Full
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$missing = [System.Collections.Generic.List[string]]::new()

function Say($text) { Write-Host "==> $text" -ForegroundColor White }
function Ok($text) { Write-Host "  ok      $text" -ForegroundColor Green }
function Miss($text) { Write-Host "  missing $text" -ForegroundColor Red; $missing.Add($text) }
function Has($command) { [bool](Get-Command $command -ErrorAction SilentlyContinue) }
function Refresh-Path {
    $env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' +
        [Environment]::GetEnvironmentVariable('Path', 'User') + ";$env:USERPROFILE\.cargo\bin"
}

function Winget($id, $what, [string[]]$extra = @()) {
    if ($Check) { Miss $what; return }
    if (-not (Has winget)) { Miss "$what (install it yourself; winget is not available)"; return }
    winget install --id $id --exact --silent --accept-package-agreements --accept-source-agreements @extra
    if ($LASTEXITCODE -ne 0) { Miss $what; return }
    Refresh-Path
}

$dotnetSdk = (Get-Content (Join-Path $root 'global.json') -Raw | ConvertFrom-Json).sdk.version
$rustChannel = (Select-String -Path (Join-Path $root 'rust-toolchain.toml') -Pattern '^channel\s*=\s*"(.+)"').Matches[0].Groups[1].Value
$lock = Get-Content (Join-Path $root 'Cargo.lock')
$i = [Array]::IndexOf($lock, 'name = "wasm-bindgen"')
$wasmBindgen = ($lock[$i + 1] -replace '^version = "(.+)"$', '$1')
$cargoAbout = '0.9.2'
$env:DOTNET_CLI_TELEMETRY_OPTOUT = '1'
Refresh-Path

Say 'Visual Studio Build Tools (C++ workload, for Rust''s MSVC linker)'
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$vc = if (Test-Path $vswhere) {
    & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
}
if ($vc) { Ok $vc } else {
    Winget 'Microsoft.VisualStudio.2022.BuildTools' 'Visual Studio Build Tools (C++)' @(
        '--override', '--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended')
}

Say "Rust $rustChannel"
if (-not (Has rustup)) { Winget 'Rustlang.Rustup' 'rustup' }
if (Has rustup) {
    # rustup installs the toolchain, components and targets rust-toolchain.toml names.
    if (-not $Check) { Push-Location $root; rustup toolchain install | Out-Null; Pop-Location }
    Push-Location $root
    $have = rustc --version 2>$null
    Pop-Location
    if ($have -like "rustc $rustChannel *") { Ok $have } else { Miss "rustc $rustChannel (have: $have)" }
}

Say ".NET SDK $dotnetSdk (or a newer 10.0 feature band)"
$sdkOk = { (Has dotnet) -and ((dotnet --list-sdks 2>$null) -match '^10\.0\.') }
if (-not (& $sdkOk)) { Winget 'Microsoft.DotNet.SDK.10' '.NET 10 SDK' }
if (& $sdkOk) {
    Push-Location $root
    Ok "dotnet $(dotnet --version)"
    if (-not $Check) { dotnet tool restore | Out-Null }
    if ((dotnet tool list --local) -match '^fable ') { Ok 'fable (dotnet tool)' } else { Miss 'fable (dotnet tool restore)' }
    Pop-Location
}

Say 'Node 22 (the WebAssembly and Fable smoke tests, markdownlint; optional)'
if ((Has node) -and ((node --version) -match '^v2[2-9]')) { Ok "node $(node --version)" }
elseif (-not $Check) { Winget 'OpenJS.NodeJS.LTS' 'Node 22' }
else { Write-Host '  optional: Node 22 is not installed' }

if ($Full) {
    Say 'Extra CI tools'
    foreach ($tool in @(@('wasm-bindgen', 'wasm-bindgen-cli', $wasmBindgen), @('cargo-about', 'cargo-about', $cargoAbout))) {
        $bin, $crate, $version = $tool
        if ((Has $bin) -and ((& $bin --version) -match " $([regex]::Escape($version))")) { Ok "$bin $version" }
        elseif ($Check) { Miss "$bin $version" }
        else { cargo install --locked $crate --version $version; Ok "$bin $version" }
    }
}

Write-Host ''
if ($missing.Count -gt 0) {
    Say "Missing: $($missing -join ', ')"
    exit 1
}
Say 'Ready. Open a new terminal (so PATH is current), then:'
Write-Host '  dotnet build FarmingRpgMaker.sln; dotnet test FarmingRpgMaker.sln --no-build'
Write-Host '  cargo test --workspace'
Write-Host '  dotnet run --project src/FarmingRpgMaker.App'
