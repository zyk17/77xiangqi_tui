# Capture TUI snapshot and print board preview
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot\..

cargo test capture_startpos_writes_board_snapshot -- --nocapture

$path = Join-Path (Get-Location) "logs\board_capture.txt"
$lines = Get-Content $path -Encoding UTF8
Write-Host ("Wrote {0} ({1} lines)" -f $path, $lines.Count)

$start = -1
for ($i = 0; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -like "*A *") {
        $start = $i
        break
    }
}

if ($start -ge 0) {
    $end = [Math]::Min($start + 27, $lines.Count - 1)
    Write-Host ("Board preview lines {0}-{1}" -f ($start + 1), ($end + 1))
    for ($k = $start; $k -le $end; $k++) {
        Write-Host $lines[$k]
    }
}

Write-Host "See logs/runtime.log when XIANGQI_TUI_DEBUG=1"
