# BLOCKFALL ×3

An original falling-block puzzle game for the terminal, built three times with the same behaviour:

| folder | language | terminal lib | binary |
|--------|----------|--------------|--------|
| `go/` | Go | [tcell](https://github.com/gdamore/tcell) | `bin\blockfall-go.exe` |
| `csharp/` | C# / .NET 8 | none (System.Console + ANSI) | `bin\blockfall-cs.exe` |
| `rust/` | Rust | [crossterm](https://github.com/crossterm-rs/crossterm) | `bin\blockfall-rs.exe` |

The rules, layout and controls are in [SPEC.md](SPEC.md). All three versions follow it.

## Toolchains (installed with winget)
```powershell
winget install -e --id GoLang.Go
winget install -e --id Microsoft.DotNet.SDK.8
winget install -e --id Rustlang.Rustup
rustup default stable-x86_64-pc-windows-gnu   # self-contained linker, no Visual Studio needed
```

## Build & play
```powershell
.\build.ps1
.\bin\blockfall-go.exe
.\bin\blockfall-cs.exe
.\bin\blockfall-rs.exe
```
Use **Windows Terminal** for correct colours and box characters. The window needs to be at least 60×28.

Controls: ←/→ move · ↑/X rotate · Z rotate ccw · ↓ soft drop · Space hard drop · P pause · R restart · Q/Esc quit
