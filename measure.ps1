# Measures the running BLOCKFALL games over a time window:
# CPU (total and kernel), I/O operations and how many of them each drawn frame costs.
#
#   .\measure.ps1              # 30-second window
#   .\measure.ps1 -Seconds 60
param([int]$Seconds = 30)

function Get-Snapshot {
    $stats = @{}
    foreach ($f in Get-ChildItem (Join-Path $env:TEMP 'blockfall\*.json') -ErrorAction SilentlyContinue) {
        try { $j = Get-Content $f.FullName -Raw | ConvertFrom-Json; $stats[[int]$j.pid] = $j } catch { }
    }
    [pscustomobject]@{
        Time  = Get-Date
        Procs = @(Get-CimInstance Win32_Process -Filter "Name LIKE 'blockfall-%'")
        Stats = $stats
    }
}

$a = Get-Snapshot
if ($a.Procs.Count -eq 0) { throw 'No blockfall processes running - start them with .\play-ai.ps1' }
Write-Host "Measuring $($a.Procs.Count) processes for $Seconds s..."
Start-Sleep -Seconds $Seconds
$b = Get-Snapshot
$dt = ($b.Time - $a.Time).TotalSeconds

$rows = foreach ($p in $b.Procs) {
    $q = $a.Procs | Where-Object ProcessId -eq $p.ProcessId
    if (-not $q) { continue }
    $cpu    = (($p.KernelModeTime + $p.UserModeTime) - ($q.KernelModeTime + $q.UserModeTime)) / 1e7
    $kernel = ($p.KernelModeTime - $q.KernelModeTime) / 1e7
    $ops    = ($p.WriteOperationCount - $q.WriteOperationCount) + ($p.OtherOperationCount - $q.OtherOperationCount)
    $bytes  = ($p.WriteTransferCount - $q.WriteTransferCount) + ($p.OtherTransferCount - $q.OtherTransferCount)
    $sa = $a.Stats[[int]$p.ProcessId]; $sb = $b.Stats[[int]$p.ProcessId]
    $frames = if ($sa -and $sb) { $sb.frames - $sa.frames } else { 0 }
    $mode = if (-not $sb) { '-' } elseif (-not $sb.ai) { 'human' } elseif ($sb.turbo) { 'turbo' } else { 'watch' }
    [pscustomobject]@{
        Process        = $p.Name -replace '\.exe$', ''
        Mode           = $mode
        'CPU %'        = [math]::Round($cpu / $dt * 100, 2)
        'Kernel %'     = [math]::Round($kernel / $dt * 100, 2)
        'Frames/s'     = [math]::Round($frames / $dt, 1)
        'I/O ops/s'    = [math]::Round($ops / $dt, 1)
        'Ops/frame'    = if ($frames -gt 0) { [math]::Round($ops / $frames, 2) } else { '-' }
        'Bytes/frame'  = if ($frames -gt 0) { [math]::Round($bytes / $frames) } else { '-' }
        'CPU ms/frame' = if ($frames -gt 0) { [math]::Round($cpu * 1000 / $frames, 3) } else { '-' }
    }
}
$rows | Sort-Object Process | Format-Table -AutoSize
Write-Host 'Ops include write + other I/O operations (console output goes through the console driver).'
Write-Host 'Each game also writes its stats file twice a second (counted here).'
