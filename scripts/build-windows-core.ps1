param([switch]$Run, [string]$DataDirectory, [switch]$ReplayM0, [switch]$VerifyM1, [string]$StandaloneOutput)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
# SQLCipher pulls openssl-src which needs a full Perl (MSYS perl bundled with Git
# misses core modules). Prepend the portable Strawberry Perl if present; see
# docs/technical-debt.md TD-015.
$portablePerl = Join-Path $env:LOCALAPPDATA 'NotesBuild/perl/perl/bin'
if (Test-Path $portablePerl) { $env:PATH = "$portablePerl;$env:PATH" }
$architecture = [System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString().ToLowerInvariant()
$triple = @{
    x64 = 'x86_64-pc-windows-msvc'
    arm64 = 'aarch64-pc-windows-msvc'
    x86 = 'i686-pc-windows-msvc'
}[$architecture]
if (!$triple) { throw "Unsupported Windows architecture $architecture" }
$platform = if ($architecture -eq 'arm64') { 'ARM64' } else { $architecture }
$previousDll = $env:M0CoreDll
$appArguments = @()
if ($StandaloneOutput) {
    if ($Run -or $VerifyM1 -or $ReplayM0) { throw 'StandaloneOutput builds without launching the app' }
    $StandaloneOutput = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($StandaloneOutput)
}
if ($DataDirectory) {
    $absoluteDataDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($DataDirectory)
    $appArguments += '--data-dir', $absoluteDataDirectory
}
if ($ReplayM0) { $appArguments += '--m0-replay' }
if ($VerifyM1) {
    if (!$Run -or !$DataDirectory -or $ReplayM0 -or (Test-Path -LiteralPath $absoluteDataDirectory)) {
        throw 'VerifyM1 requires -Run and -DataDirectory with a new directory'
    }
    $appArguments += '--verify-m1'
}

Push-Location $root
try {
    # The DLL must load on a clean Windows install before any VC runtime is installed.
    $staticCrt = "target.$triple.rustflags = ['-C', 'target-feature=+crt-static']"
    cargo build --locked --release -p notes-ffi --features c-editor --target $triple --target-dir target --config $staticCrt
    if ($LASTEXITCODE) { throw 'Rust build failed' }
    $env:M0CoreDll = Join-Path $root "target/$triple/release/notes_ffi.dll"
    Push-Location (Join-Path $root 'clients/windows/Notes')
    try {
        if ($Run) { dotnet run -c Debug -p:Platform=$platform -p:WinAppRunDevTools=off -- @appArguments }
        elseif ($StandaloneOutput) {
            dotnet build -c Debug -p:Platform=$platform -p:WindowsPackageType=None -p:WindowsAppSDKSelfContained=true --self-contained -o $StandaloneOutput
        }
        else { dotnet build -p:Platform=$platform }
        if ($LASTEXITCODE) { throw 'WinUI build or launch failed' }
        if ($VerifyM1) {
            $report = Get-Content -Raw -LiteralPath (Join-Path $absoluteDataDirectory 'winui-acceptance.json') |
                ConvertFrom-Json
            $report.passed | ForEach-Object { Write-Output "PASS: $_" }
            if ($report.error) { throw $report.error }
        }
    }
    finally { Pop-Location }
}
finally {
    $env:M0CoreDll = $previousDll
    Pop-Location
}
