param([string]$NodePath)
$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$oldNode = $env:UBI_BENCHMARK_NODE
$oldLocation = Get-Location
try {
    if ($NodePath) { $env:UBI_BENCHMARK_NODE = $NodePath }
    Set-Location $repo
    & cargo test --lib benchmark_grade -- --nocapture
    if ($LASTEXITCODE -ne 0) { throw "Benchmark evaluation failed (cargo exit $LASTEXITCODE)." }
} finally {
    $env:UBI_BENCHMARK_NODE = $oldNode
    Set-Location $oldLocation
}
