# C# edition: the assembly, explained

[blockfall-cs.asm](blockfall-cs.asm) is x86-64 code that the **.NET JIT** (RyuJIT) produced for the game's methods while running `--bench 10000 --seed 42`. Start with [../../ASSEMBLY.md](../../ASSEMBLY.md) if assembly is new to you.

## Why this listing doesn't come from the `.exe`

`blockfall-cs.exe` (0.2 MB) holds no x86 code for the game. It holds **IL** (Intermediate Language), a CPU-independent bytecode, plus a small launcher. When the program runs:

1. the .NET runtime loads the IL,
2. the first time a method is called, the **JIT** compiles its IL to machine code for *this* CPU,
3. that code lives in memory only and is gone when the program exits.

So the only way to see it is to ask the runtime to print it while it compiles. [disasm.ps1](../../disasm.ps1) does that with environment variables that the normal .NET 8 runtime supports:
```powershell
$env:DOTNET_JitDisasm = 'Game:* Screen:* Program:* ...'   # print code for these classes' methods
$env:DOTNET_JitStdOutFile = '...\cs-jit.txt'              # into this file
$env:DOTNET_TieredCompilation = '0'                       # compile once, fully optimised (see below)
.\blockfall.exe --bench 10000 --seed 42
```
Only methods that actually ran are compiled, and therefore listed. The benchmark runs the rules and the AI (19 methods). Drawing (`Render`, `Screen`) only runs in the interactive game, so it isn't in the listing.

**Tiered compilation.** Normally .NET first compiles a method quickly with no optimisation ("Tier0"), so the program starts fast. It counts calls, and recompiles the busy methods with full optimisation ("Tier1"), sometimes using what it saw at run time (PGO). With tiering off, you get the fully optimised version (`FullOpts`) straight away, which compares fairly with Go and Rust.

## How to read a JIT listing

```asm
; Assembly listing for method Game:NextRand():uint:this (FullOpts)   ← class:method(args):return type, "this" = instance method
; Emitting BLENDED_CODE for X64 with AVX - Windows                   ← tuned for any x64 CPU, may use AVX
; rsp based frame                                                    ← no frame pointer (rbp) set up
; No PGO data                                                        ← no run-time profile was used

G_M000_IG01:                ;; offset=0x0000                        ← "instruction group" = a label; offset from the method start
```
`gword ptr` means memory that holds a reference to a GC object, so the garbage collector must track it. `bword ptr` is an interior pointer, pointing inside an object. Helper calls such as `CORINFO_HELP_NEWARR_1_VC` (allocate an array) or `CORINFO_HELP_RNGCHKFAIL` (index out of range) are calls into the runtime. C# uses the **Windows x64 calling convention**: `this` in `rcx`, then arguments in `rdx, r8, r9`, result in `eax`/`rax`.

The JIT doesn't print source line numbers, so match methods by name with `Program.cs`.

## 1. `NextRand`: the random number generator

```csharp
uint NextRand() { uint x = rng; x ^= x << 13; x ^= x >> 17; x ^= x << 5; rng = x; return x; }
```
```asm
; Assembly listing for method Game:NextRand():uint:this (FullOpts)
G_M000_IG02:
       mov      eax, dword ptr [rcx+0x6C]   ; eax = this.rng   (this in rcx; rng is at offset 0x6C)
       mov      edx, eax
       shl      edx, 13                     ; the JIT prints decimal: 13, not 0xd
       xor      eax, edx                    ; x ^= x << 13
       mov      edx, eax
       shr      edx, 17
       xor      eax, edx                    ; x ^= x >> 17
       mov      edx, eax
       shl      edx, 5
       xor      eax, edx                    ; x ^= x << 5
       mov      dword ptr [rcx+0x6C], eax   ; this.rng = x
G_M000_IG03:
       ret                                  ; return value already in eax
; Total bytes of code 28
```
These are the same instructions as Go and Rust. Unlike them, here `NextRand` stays a real function. In `Draw7` the JIT chose to *call* it rather than inline it.

## 2. `Draw7`: the 7-bag shuffle

```csharp
int[] a = { 0, 1, 2, 3, 4, 5, 6 };
for (int i = 6; i >= 1; i--) { int j = (int)(NextRand() % (uint)(i + 1)); (a[i], a[j]) = (a[j], a[i]); }
```
```asm
G_M000_IG03:
       mov      rcx, 0x7FF8D7668318         ; the type handle for int[]
       mov      edx, 7                      ; length 7
       call     CORINFO_HELP_NEWARR_1_VC    ; new int[7] on the GC heap → rax
       mov      rsi, rax                    ; rsi = a
       mov      rcx, 0x16B0226BBD8          ; address of the constant data {0,1,2,3,4,5,6}
       vmovdqu  xmm0, xmmword ptr [rcx]     ; copy 16 bytes = a[0..3]   (AVX register, 4 ints at once)
       vmovdqu  xmmword ptr [rsi+0x10], xmm0;   array data starts at +0x10, after the object header and length
       vmovdqu  xmm0, xmmword ptr [rcx+0x0C]; copy 16 bytes = a[3..6]   (overlaps a[3], that's fine)
       vmovdqu  xmmword ptr [rsi+0x1C], xmm0
       mov      edi, 6                      ; i = 6
G_M000_IG04:                                ; ── loop body ──
       mov      rcx, rbx                    ; this
       call     [Game:NextRand():uint:this] ; eax = random number
       lea      ecx, [rdi+0x01]             ; ecx = i + 1
       xor      edx, edx
       div      edx:eax, ecx                ; hardware divide: edx = remainder = j   (same as Go)
       mov      ecx, edi
       lea      rax, bword ptr [rsi+4*rcx+0x10] ; rax = &a[i]   (4 bytes per int)
       cmp      edx, 7                      ; bounds check: is j < a.Length (7)?
       jae      G_M000_IG11                 ;   no → throw IndexOutOfRangeException
       lea      r8, bword ptr [rsi+4*rdx+0x10]  ; r8 = &a[j]
       mov      edx, dword ptr [rsi+4*rdx+0x10] ; swap a[i] and a[j]
       mov      ecx, dword ptr [rsi+4*rcx+0x10]
       mov      dword ptr [rax], edx
       mov      dword ptr [r8], ecx
       dec      edi                         ; i--
       test     edi, edi
       jg       SHORT G_M000_IG04           ; while i > 0
  ...
G_M000_IG11:
       call     CORINFO_HELP_RNGCHKFAIL     ; throws IndexOutOfRangeException
```
- The JIT removed the bounds check on `a[i]` (it knows `1 ≤ i ≤ 6`) but kept the one on `a[j]`. Go and Rust proved that `j % (i+1)` is always below 7. The JIT has less time to analyse, because it compiles while your program waits.
- The array is 4-byte `int`s (`4*rcx`), against 8-byte `int`/`usize` in Go and Rust.

## 3. `Evaluate`: the scoring formula

The bumpiness loop, `bump += Math.Abs(hts[x] - hts[x + 1])`:
```asm
G_M000_IG13:
       mov      edx, ecx
       mov      rdx, qword ptr [rax+8*rdx+0x10]   ; rdx = hts[x]   (hts is a long[] on the heap)
       inc      ecx
       mov      r8d, ecx
       sub      rdx, qword ptr [rax+8*r8+0x10]    ; rdx = hts[x] − hts[x+1]
       js       SHORT G_M000_IG18                 ; negative? → go negate it
G_M000_IG14:
       add      r14, rdx                          ; bump += |d|
       cmp      ecx, 9
       jl       SHORT G_M000_IG13                 ; next x
  ...
G_M000_IG18:
       neg      rdx                               ; d = −d
       js       SHORT G_M000_IG19                 ; still negative?? only possible for long.MinValue
       jmp      SHORT G_M000_IG14
G_M000_IG19:
       call     [System.Math:ThrowNegateTwosCompOverflow()]   ; Math.Abs(long.MinValue) throws OverflowException
```
`Math.Abs` in .NET is defined to throw for `long.MinValue`, whose positive value doesn't fit in a `long`. So the JIT keeps a check that never fires here. Go and Rust's `abs` just wrap, and they compile it branch-free.

Then the formula. The JIT prints negative constants as signed numbers:
```asm
G_M000_IG15:
       imul     rax, rdi, -0x7C872          ; agg     × −510066
       movsxd   rdx, esi                    ; widen `cleared` from int (32-bit) to long (64-bit), keeping the sign
       imul     rcx, rdx, 0xB9B5A           ; cleared × 760666
       add      rax, rcx
       imul     rcx, rbp, 0x57116           ; holes   × 356630
       sub      rax, rcx
       imul     rcx, r14, 0x2D0A3           ; bump    × 184483
       sub      rax, rcx                    ; result in rax
G_M000_IG16:
       add      rsp, 32                     ; free the stack and restore the saved registers
       pop      rbx
       ...
       ret
```
The same four multiplies as Go, and the same constants as all three.

## Try it yourself
- Remove `DOTNET_TieredCompilation = '0'` from `disasm.ps1` and run it again. Busy methods then appear twice: first `(Tier0)`, long and naive, then `(Tier1)` optimised, sometimes with `PGO data`.
- `DOTNET_JitDisasmSummary=1` lists every method the JIT compiled, including the .NET library ones.
- To see the IL itself, install a decompiler such as ILSpy (`dotnet tool install -g ilspycmd`, then `ilspycmd -il csharp\bin\Release\net8.0\win-x64\blockfall.dll`). That `.dll` is the IL that the single-file `.exe` wraps.
