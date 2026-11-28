param([switch]$Run)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$architecture = [System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString().ToLowerInvariant()
$triple = @{
    x64 = 'x86_64-pc-windows-msvc'
    arm64 = 'aarch64-pc-windows-msvc'
    x86 = 'i686-pc-windows-msvc'
}[$architecture]
if (!$triple) { throw "Unsupported Windows architecture $architecture" }
$platform = if ($architecture -eq 'arm64') { 'ARM64' } else { $architecture }
$previousDll = $env:M0CoreDll

Push-Location $root
try {
    cargo build --locked --release -p notes-ffi --features c-editor --target $triple --target-dir target
    if ($LASTEXITCODE) { throw 'Rust build failed' }
    $env:M0CoreDll = Join-Path $root "target/$triple/release/notes_ffi.dll"
    Push-Location (Join-Path $root 'clients/windows/Notes')
    try {
        if ($Run) { dotnet run -p:Platform=$platform -p:WinAppRunDevTools=off }
        else { dotnet build -p:Platform=$platform }
        if ($LASTEXITCODE) { throw 'WinUI build or launch failed' }
    }
    finally { Pop-Location }
}
finally {
    $env:M0CoreDll = $previousDll
    Pop-Location
}
