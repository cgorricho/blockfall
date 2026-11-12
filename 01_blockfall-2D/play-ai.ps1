# Launches the three AI games side by side in one Windows Terminal window,
# plus the resource dashboard in a second window.
#
#   .\play-ai.ps1               # random seed, watchable pace
#   .\play-ai.ps1 -Seed 42      # same game every time
#   .\play-ai.ps1 -Turbo        # start flat-out
param(
    [uint32]$Seed = 0,
    [switch]$Turbo
)
$ErrorActionPreference = 'Stop'
# The binaries live in the repo-wide bin\ folder, one level up.
$bin = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\bin'))
foreach ($exe in 'blockfall-go.exe', 'blockfall-cs.exe', 'blockfall-rs.exe', 'blockfall-dash.exe') {
    if (-not (Test-Path (Join-Path $bin $exe))) { throw "Missing bin\$exe - run ..\build.ps1 first" }
}
if (-not (Get-Command wt -ErrorAction SilentlyContinue)) { throw 'Windows Terminal (wt.exe) is required' }

if ($Seed -eq 0) { $Seed = [uint32](Get-Random -Minimum 1 -Maximum 2147483647) }
$gameArgs = @('--ai', '--seed', "$Seed")
if ($Turbo) { $gameArgs += '--turbo' }

# One maximized window, three panes of equal width (each game needs 44 columns).
#   pane 0 (Go) | split off 2/3 for pane 1 (C#) | focus pane 1, split it in half for pane 2 (Rust)
# focus-pane makes the second split land on the C# pane; without it wt may split the Go pane.
# (--title is only valid on new-tab / split-pane, not on the window itself)
wt -w new --maximized `
    new-tab --title 'Go' -d $bin "$bin\blockfall-go.exe" @gameArgs `; `
    split-pane -V -s 0.667 --title 'C#' -d $bin "$bin\blockfall-cs.exe" @gameArgs `; `
    focus-pane -t 1 `; `
    split-pane -V -s 0.5 --title 'Rust' -d $bin "$bin\blockfall-rs.exe" @gameArgs

# Second window: the dashboard (opens on top of the games window).
wt -w new --size 80,37 `
    new-tab --title 'Dashboard' -d $bin "$bin\blockfall-dash.exe"

Write-Host "Started seed $Seed. In each game: T = turbo, F = retro look, P = pause, Q = quit."
