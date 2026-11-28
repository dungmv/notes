param(
    [Parameter(Mandatory)][string]$App,
    [Parameter(Mandatory)][string]$FixtureTool,
    [Parameter(Mandatory)][string]$Results,
    [switch]$RequireOffline
)

$ErrorActionPreference = 'Stop'
$appPath = (Resolve-Path -LiteralPath $App).Path
$fixturePath = (Resolve-Path -LiteralPath $FixtureTool).Path
$resultsPath = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Results)
if (Test-Path -LiteralPath $resultsPath) { throw 'Results must be a new directory' }
New-Item -ItemType Directory -Path $resultsPath | Out-Null
if ($RequireOffline) {
    $routes = @(Get-NetRoute | Where-Object DestinationPrefix -in @('0.0.0.0/0', '::/0'))
    $adapters = @(Get-NetAdapter | Where-Object Status -eq 'Up')
    if ($routes.Count -ne 0 -or $adapters.Count -ne 0) { throw 'The verification machine must be disconnected' }
    Write-Output "Offline: $($routes.Count) default routes, $($adapters.Count) active adapters"
}
Write-Output "Windows $([Environment]::OSVersion.Version), $env:PROCESSOR_ARCHITECTURE"

function Invoke-Fixture($command, $data) {
    & $fixturePath $command $data
    if ($LASTEXITCODE) { throw "Fixture command $command failed" }
}

function Start-Notes($data, $mode) {
    $arguments = @('--data-dir', ('"{0}"' -f $data), "--m2-$mode")
    $previous = $env:NOTES_M2_LAUNCH_TICKS
    try {
        $env:NOTES_M2_LAUNCH_TICKS = [string][DateTime]::UtcNow.Ticks
        Start-Process -FilePath $appPath -WorkingDirectory (Split-Path -Parent $appPath) -ArgumentList $arguments -PassThru
    }
    finally { $env:NOTES_M2_LAUNCH_TICKS = $previous }
}

function Wait-Marker($process, $data, $name) {
    $limit = [Diagnostics.Stopwatch]::StartNew()
    while (!(Test-Path -LiteralPath (Join-Path $data $name))) {
        if ($process.HasExited -or $limit.Elapsed.TotalSeconds -gt 60) {
            $report = Join-Path $data 'm2-result.json'
            if (Test-Path -LiteralPath $report) { throw (Get-Content -Raw -LiteralPath $report) }
            throw "WinUI did not write $name"
        }
        Start-Sleep -Milliseconds 25
    }
}

function Read-Report($process, $data) {
    if (!$process.WaitForExit(60000)) { throw 'WinUI did not exit within 60 seconds' }
    $report = Get-Content -Raw -LiteralPath (Join-Path $data 'm2-result.json') | ConvertFrom-Json
    if ($process.ExitCode -or $report.error) { throw "WinUI exit $($process.ExitCode): $($report.error)" }
    $report.passed | ForEach-Object { Write-Output "PASS: $_" }
}

$failures = @()
foreach ($case in @('verify', 'retry', 'text', 'image') + @(1..8 | ForEach-Object { "startup-$_" })) {
    $data = Join-Path $resultsPath $case
    $mode = if ($case.StartsWith('startup-')) { 'startup' } else { $case }
    $schema = if ($mode -eq 'verify') { 'prepare-v1' } else { 'prepare' }
    Invoke-Fixture $schema $data
    if ($mode -eq 'retry') { Invoke-Fixture 'hold-failures' $data }
    $process = Start-Notes $data $mode
    try {
        if ($mode -eq 'retry') {
            Wait-Marker $process $data 'rejected'
            Invoke-Fixture 'release-text' $data
            New-Item -ItemType File -Path (Join-Path $data 'retry-text') | Out-Null
            Wait-Marker $process $data 'rejected-image'
            Invoke-Fixture 'release-image' $data
            New-Item -ItemType File -Path (Join-Path $data 'retry-image') | Out-Null
        }
        if ($mode -in @('text', 'image')) {
            Wait-Marker $process $data 'ack.json'
            $process.Kill()
            if (!$process.WaitForExit(10000)) { throw 'WinUI did not terminate' }
            $process.Dispose()
            $process = Start-Notes $data 'reopen'
        }
        Read-Report $process $data
    }
    catch {
        $failures += "$($case): $_"
        Write-Output "FAIL: $($failures[-1])"
    }
    finally {
        if (!$process.HasExited) { $process.Kill(); $process.WaitForExit() }
        $process.Dispose()
    }
}
if ($failures.Count) { throw "$($failures.Count) M2 case(s) failed. See the printed results and $resultsPath" }
