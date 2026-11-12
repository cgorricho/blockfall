# BLOCKFALL

An original falling-block puzzle game, built several ways. Each project has its own folder and README:

| folder | what | tech | binary |
|--------|------|------|--------|
| [blockfall-2D/](blockfall-2D/README.md) | The original terminal game, built three times (Go, C#, Rust) with the same behaviour, plus a live resource dashboard | Go · C# / .NET 8 · Rust (crossterm) · Go (tcell) | `bin\blockfall-{go,cs,rs,dash}.exe` |
| [rocker/](rocker/README.md) | **BLOCKFALL ROCKER**: the same pieces stacked on a rocking base with real rolling physics; you lose if the stack slides off | Rust + [macroquad](https://macroquad.rs) | `bin\blockfall-rocker.exe` |
| [tower-3D/](tower-3D/README.md) | **BLOCKFALL TOWER-3D**: the board wrapped around a 3D cylinder | Rust + macroquad | `bin\blockfall-tower3d.exe` |

## Docs in this folder

- [SPEC.md](SPEC.md): the shared rules, layout, controls, PRNG and 7-bag. Every project follows it, so seed 42 deals the same pieces everywhere.
- [LANGUAGE-COMPARISON.md](LANGUAGE-COMPARISON.md): which language came out best, with benchmark numbers.
- [ASSEMBLY.md](ASSEMBLY.md): reading the x86-64 machine code of the three 2D editions.
- [HANDOFF.md](HANDOFF.md): current state and open threads, for picking the work up again.

## Toolchains (installed with winget)
```powershell
winget install -e --id GoLang.Go
winget install -e --id Microsoft.DotNet.SDK.8
winget install -e --id Rustlang.Rustup
rustup default stable-x86_64-pc-windows-gnu   # self-contained linker, no Visual Studio needed
```

## Build
```powershell
& 'C:\Users\carlos\Documents\Projects\blockfall\build.ps1'
```
This builds every project into `bin\` at the repo root and checks that the three 2D editions play the same game. Close running games first: Windows locks the `.exe` files and the copy fails.

How to play each one: see the README in its folder.
