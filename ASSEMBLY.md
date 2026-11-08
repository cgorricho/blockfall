# Reading the assembly of BLOCKFALL

Each language folder has an `asm/` folder holding the x86-64 machine code of that edition, written as assembly text, plus a README that walks through the interesting parts:

| file | where the code comes from | walkthrough |
|------|---------------------------|-------------|
| [go/asm/blockfall-go.asm](go/asm/blockfall-go.asm) | disassembled from `blockfall-go.exe` | [go/asm/README.md](go/asm/README.md) |
| [rust/asm/blockfall-rs.asm](rust/asm/blockfall-rs.asm) | disassembled from `blockfall-rs.exe` | [rust/asm/README.md](rust/asm/README.md) |
| [csharp/asm/blockfall-cs.asm](csharp/asm/blockfall-cs.asm) | printed by the .NET JIT at run time | [csharp/asm/README.md](csharp/asm/README.md) |

Regenerate all three after changing the code:
```powershell
.\disasm.ps1
```
Addresses (such as `140140180`) change with every build. The instructions only change when the code or the compiler changes.

## Source → executable, per language

```
Go     main.go    ──go build──▶  x86-64 machine code + Go runtime ──▶ blockfall-go.exe (4.6 MB)
Rust   main.rs    ──cargo (LLVM)─▶ x86-64 machine code + std       ──▶ blockfall-rs.exe (1.5 MB)
C#     Program.cs ──dotnet (Roslyn)─▶ IL bytecode                  ──▶ blockfall-cs.exe (0.2 MB)
                                       └── at run time, the .NET JIT turns each method into x86-64
```
Go and Rust compile **ahead of time**: the CPU instructions are already in the `.exe`, and a disassembler turns them back into text. C# compiles to **IL** (Intermediate Language), which is CPU-independent. The machine code only exists in memory, once the JIT ("just in time" compiler) has compiled a method the first time it runs. That is why the C# listing comes from the runtime and not from the file.

## A five-minute primer on x86-64 assembly (Intel syntax)

All three listings use **Intel syntax**: `instruction destination, source`.

```asm
mov    eax, DWORD PTR [rcx+0x6c]   ; eax = the 4-byte value in memory at address rcx+0x6c
shl    edx, 13                     ; edx = edx << 13
xor    eax, edx                    ; eax = eax ^ edx
imul   rcx, rdx, 0x2d0a3           ; rcx = rdx * 184483
cmp    rdi, 0x9                    ; compare rdi with 9 (sets flags, stores nothing)
jl     140140e4e                   ; jump if the comparison said "less than"
call   140085d60                   ; call a function
ret                                ; return to the caller
```

**Registers** are the CPU's own variables. x86-64 has 16 general-purpose ones: `rax rbx rcx rdx rsi rdi rbp rsp r8 … r15`. One register has several names by size: `rax` is 64-bit, `eax` is its low 32 bits, `ax` 16 bits, `al` 8 bits. A `uint32` (such as the random number generator's state) therefore shows up as `eax`, `ecx` and so on. `rsp` is the stack pointer. `xmm0…xmm15` are 128-bit registers used for floating point and SIMD (several numbers at once).

**Memory** is written `[base + index*scale + offset]`, e.g. `QWORD PTR [rdi+rcx*8]` is the 8-byte element `rcx` of an array that starts at `rdi`. `BYTE/WORD/DWORD/QWORD` = 1/2/4/8 bytes.

**Flags and jumps**: `cmp`/`test` set flags. The conditional jumps that follow read them: `je`/`jne` (equal / not equal), `jl`/`jg` (signed less/greater), `jb`/`ja` (unsigned below/above), `js` (negative). `cmovl` is a "conditional move": it copies only if the flag condition holds, which avoids a jump.

**Function calls** pass arguments in registers. Which registers is the **calling convention**:
- Windows x64 (Rust, C#, and every Windows DLL): `rcx, rdx, r8, r9`, result in `rax`.
- Go's own convention (ABIInternal): `rax, rbx, rcx, rdi, rsi, r8 …`, result in `rax`. Go also keeps two fixed registers: `r14` = the current goroutine, `xmm15` = always zero.

Common tricks you'll see:
- `xor ecx, ecx`: set a register to 0 (shorter than `mov ecx, 0`).
- `lea rax, [rdx+rdx*2]`: "load effective address", used as cheap arithmetic (here `rax = rdx*3`).
- multiply by a weird constant then shift right: a **division by a constant**, see the Rust walkthrough.

## The same three functions in all three walkthroughs

Each README annotates the same pieces of the game, so you can compare compilers:

1. **`nextRand`**, the xorshift random number generator (SPEC.md): `x ^= x<<13; x ^= x>>17; x ^= x<<5`. All three compile it to the same 9 instructions.
2. **The 7-bag shuffle** in `draw7`: `j = next() % (i+1)`. Go and C# use the `div` instruction. Rust unrolls the loop and replaces each division with a multiply.
3. **The AI's scoring formula** at the end of `evaluate`: `-510066*agg + 760666*cleared - 356630*holes - 184483*bump`, four `imul` instructions in every language.

| | Go | Rust | C# |
|---|---|---|---|
| where the code lives | in the `.exe` | in the `.exe` | made at run time by the JIT |
| `nextRand` | inlined into `draw7` | inlined into `draw7` | separate function, called |
| `% (i+1)` | `div` (hardware divide) | multiply by magic constant | `div` |
| `evaluate` | own function | inlined into `find_plan`, inside `spawn` | own function |
| `bump` loop | scalar, branch-free `cmovl` | SIMD (2 values per instruction) | scalar, with an overflow check |
| extra checks it adds | stack growth, GC write barrier, bounds | bounds | bounds, `Math.Abs` overflow |
