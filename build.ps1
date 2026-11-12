# Builds all three BLOCKFALL editions into .\bin
$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$bin = Join-Path $root 'bin'
New-Item -ItemType Directory -Force $bin | Out-Null

# Refresh PATH so freshly installed toolchains are visible in this shell
$env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User')

Write-Host '== Go' -ForegroundColor Cyan
Push-Location (Join-Path $root 'go')
go mod tidy
go build -o (Join-Path $bin 'blockfall-go.exe') .
Pop-Location

Write-Host '== C#' -ForegroundColor Cyan
Push-Location (Join-Path $root 'csharp')
dotnet publish -c Release -r win-x64 --self-contained false -p:PublishSingleFile=true -o (Join-Path $root 'csharp\out') --nologo -v q
Copy-Item (Join-Path $root 'csharp\out\blockfall.exe') (Join-Path $bin 'blockfall-cs.exe') -Force
Pop-Location

Write-Host '== Rust' -ForegroundColor Cyan
Push-Location (Join-Path $root 'rust')
cargo build --release
Copy-Item (Join-Path $root 'rust\target\release\blockfall.exe') (Join-Path $bin 'blockfall-rs.exe') -Force
Pop-Location

Write-Host '== Rocker' -ForegroundColor Cyan
Push-Location (Join-Path $root 'rocker')
cargo build --release
Copy-Item (Join-Path $root 'rocker\target\release\blockfall-rocker.exe') (Join-Path $bin 'blockfall-rocker.exe') -Force
Pop-Location

Write-Host '== Tower-3D' -ForegroundColor Cyan
Push-Location (Join-Path $root 'tower-3D')
cargo build --release
Copy-Item (Join-Path $root 'tower-3D\target\release\blockfall-tower3d.exe') (Join-Path $bin 'blockfall-tower3d.exe') -Force
Pop-Location

Write-Host '== Dashboard' -ForegroundColor Cyan
Push-Location (Join-Path $root 'dashboard')
go mod tidy
go build -o (Join-Path $bin 'blockfall-dash.exe') .
Pop-Location

Write-Host '== Same game check (seed 42, 10000 pieces)' -ForegroundColor Cyan
foreach ($e in 'go', 'cs', 'rs') { & (Join-Path $bin "blockfall-$e.exe") --bench 10000 --seed 42 }

Write-Host "Done. Binaries in $bin" -ForegroundColor Green
Get-ChildItem $bin | Format-Table Name, Length
