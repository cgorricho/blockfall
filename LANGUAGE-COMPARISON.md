# Which language is best for BLOCKFALL?

**Conclusion: Rust is the best language for BLOCKFALL. Go is a close second, and C# comes third.**

BLOCKFALL was built three times with the same rules ([SPEC.md](SPEC.md)): in Go, C# and Rust. This report compares the three builds on speed, size, code and dependencies.

## How it was measured

- **Benchmark:** the headless mode, which runs the game logic and the AI with no screen.
  ```powershell
  .\bin\blockfall-<go|cs|rs>.exe --bench 50000 --seed 42
  ```
- **Runs:** 3 per build, on the same Windows 11 PC (2026-11-11).
- **Same game in all three:** every build printed `lines=19829 score=764094 games=11`, so they did exactly the same work. (Measured on the old 10×20 board. The board is now 12×22, so a rerun prints different totals, though still the same in all three.)

## Results

| | Rust | Go | C# |
|---|---|---|---|
| Benchmark time (3 runs) | **343 / 365 / 353 ms** | 646 / 687 / 705 ms | 1302 / 1244 / 1235 ms |
| Average | **~350 ms** | ~680 ms | ~1,260 ms |
| Compared with Rust | 1× | ~2× slower | ~3.6× slower |
| Size of the `.exe` | 1.4 MB | 4.4 MB | 0.17 MB, plus the .NET 8 runtime |
| Lines of code | 789 | ~870 (4 files, tests not counted) | **586** |
| Runs on its own | yes | yes | no, needs .NET 8 |
| Libraries used | crossterm | `golang.org/x/sys` only | none |

## Why Rust is first

- **It's the fastest.** It's about twice as fast as Go and 3.6× as fast as C#. [ASSEMBLY.md](ASSEMBLY.md) shows why, in the generated machine code:
  - LLVM inlines the AI's `evaluate` function into `find_plan`.
  - It replaces the `% (i+1)` in the 7-bag shuffle with a multiply by a "magic" constant, which is cheaper than a hardware `div`.
  - It does the `bump` loop with SIMD: two values per instruction.
- **It's small and self-contained.** One 1.4 MB `.exe`, with no runtime and no garbage collector.
- **It fits the project.** BLOCKFALL is largely about measuring CPU, memory and machine code, and Rust wins every one of those measurements.

## Where Go is the better choice

- It was the quickest to write, with the fewest dependencies.
- Its build is simple.
- tcell made it the natural choice for the resource dashboard (`01_blockfall-2D/dashboard/`).
- Its runtime adds overhead (stack-growth checks, GC write barriers, bounds checks), but you can't notice it at a playable pace.
- For a small terminal tool that needs to ship quickly, Go is the practical choice.

## C#

- **For:** the shortest source (586 lines).
- **Against:**
  - The JIT compiles the code every time the game starts.
  - It's the slowest in the benchmark.
  - It needs the .NET 8 runtime installed.
- Native AOT would fix the startup and the runtime dependency, but this build doesn't use it.

## Caveats

- **Players won't feel any difference.** At human playing speed all three use only about 1–2% of one CPU core. The ranking only matters for the AI at turbo speed, or when efficiency is the point.
- **The timings are approximate.** They come from one machine and three runs per build.
- **The C# build hasn't been tuned.** With Native AOT or tiered-compilation settings it could get closer to the others.

## Reproduce

```powershell
.\build.ps1
foreach ($e in 'go','cs','rs') { 1..3 | % { & ".\bin\blockfall-$e.exe" --bench 50000 --seed 42 } }
```
