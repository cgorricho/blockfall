# Rust edition: the assembly, explained

[blockfall-rs.asm](blockfall-rs.asm) is every function of the `blockfall` crate (the game's own code) as it sits in the release `.exe`, in Intel syntax. Above each group of instructions:
- `; main.rs:NNN  <code>` is the Rust line it came from.
- `; in fn X()` means the following code belongs to function `X`, which the compiler **inlined** here (copied into the caller instead of calling it).
- `; [library] …` is code from Rust's standard library that was inlined, such as `Vec::len` or `i64::abs`.

Start with [../../../ASSEMBLY.md](../../../ASSEMBLY.md) if assembly is new to you.

How it's made ([disasm.ps1](../../disasm.ps1)): the script builds the same release profile again, adding only line tables (`CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`), then runs GNU `objdump -d -l -C -M intel`. Debug info is designed not to change the generated code. Here the code size differs by 16 bytes out of 1 MB, and the benchmark gives the same result. Addresses below come from one build, and yours will differ.

Rust compiles through **LLVM**, the same optimiser behind clang (C/C++) and Swift. It optimises much harder than Go's compiler, which is why the Rust edition is the fastest in `--bench`. That also makes its assembly the hardest of the three to follow.

## Inlining: where did my functions go?

These functions from `main.rs` have **no entry** of their own in the listing: `next_rand`, `rotate`, `fits_on`, `clear_lines`, `evaluate`, `find_plan`, `level`, `try_move` and others. LLVM inlined them into their callers. For example, the AI's whole planning loop (`find_plan`, which calls `evaluate`, `fits_on`, `rotate`, `clear_lines`) was inlined into `<blockfall::Game>::spawn`. Search for `; in fn find_plan()` to see it.

Rust functions use the **Windows x64 calling convention**: first argument in `rcx`, then `rdx, r8, r9`, result in `rax`. So `&mut self` arrives in `rcx`.

## 1. `next_rand`: the random number generator

Inlined into `draw7`. These are the same 9 instructions as Go and C#, apart from register names:
```asm
<blockfall::Game>::draw7:
    140005300  push   rsi                        ; save registers this function will use
    ...
    140005307  mov    rsi,rcx                    ; rsi = self   (1st argument, Windows convention)
  ...
; in fn next_rand()
; main.rs:222  let mut x = self.rng;
    14000536e  mov    eax,DWORD PTR [rsi+0x184]  ; eax = self.rng  (Rust reorders struct fields: rng sits at 0x184)
; main.rs:223  x ^= x << 13;
    140005374  mov    ecx,eax
    140005376  shl    ecx,0xd                    ; 0xd = 13
    140005379  xor    ecx,eax
; main.rs:224  x ^= x >> 17;
    14000537b  mov    eax,ecx
    14000537d  shr    eax,0x11                   ; 0x11 = 17
    140005380  xor    eax,ecx
; main.rs:225  x ^= x << 5;
    140005382  mov    ecx,eax
    140005384  shl    ecx,0x5
    140005387  xor    ecx,eax
```

## 2. The 7-bag shuffle: no division at all

```rust
for i in (1..=6usize).rev() {
    let j = (self.next_rand() % (i as u32 + 1)) as usize;
    a.swap(i, j);
}
```
LLVM sees that the loop always runs exactly 6 times with `i = 6, 5, 4, 3, 2, 1`, so it **unrolls** it: it writes out six copies of the body, one per value of `i`. Each copy then divides by a known constant (7, 6, 5, …). Dividing by a constant can be replaced by a multiply and a shift, which is much faster than the `div` instruction that Go and C# use.

**`x % 7`** (first iteration, `i = 6`):
```asm
; main.rs:234  let j = (self.next_rand() % (i as u32 + 1)) as usize;
    140005389  movabs rdx,0x24924924a0000000     ; ≈ 2^64 / 7, pre-shifted (the "magic number")
    140005393  mov    rax,rcx                    ; rax = x
    140005396  mul    rdx                        ; rdx:rax = x × magic. The high half rdx = x / 7
    140005399  lea    eax,[rdx*8+0x0]            ; eax = 8q
    1400053a0  sub    edx,eax                    ; edx = q − 8q = −7q
    1400053a2  add    edx,ecx                    ; edx = x − 7q = x % 7          ← j
; in fn copy<usize>()                            ; a.swap(6, j), inlined down to plain loads and stores
    1400053a4  mov    rax,QWORD PTR [rdi+rdx*8]  ; rax = a[j]
    1400053a8  mov    QWORD PTR [rdi+0x30],rax   ; a[6] = a[j]    (0x30 = 6 × 8: the index is a constant now)
    1400053ac  mov    QWORD PTR [rdi+rdx*8],0x6  ; a[j] = 6       (LLVM knows a[6] still holds 6!)
```
**`x % 6`** (second iteration) uses a different magic number. The next `next_rand` is mixed in between, because the CPU can run independent instructions in parallel, and the compiler **interleaves** the two calculations on purpose:
```asm
    1400053c9  mov    eax,0xaaaaaaab             ; ≈ 2^34 / 6
    1400053ce  mov    ecx,edx                    ; ecx = x
    1400053d0  mov    r8d,edx                    ; ┐ already starting the NEXT random number
    1400053d3  shl    r8d,0xd                    ; │  (x << 13 for iteration 3)
    1400053d7  xor    r8d,edx                    ; ┘
    1400053da  imul   rdx,rax                    ; rdx = x × 0xaaaaaaab
    1400053de  shr    rdx,0x22                   ; >> 34 → q = x / 6
    1400053e2  add    edx,edx                    ; 2q
    1400053e4  lea    edx,[rdx+rdx*2]            ; 6q    (lea as a cheap ×3)
    1400053e7  sub    ecx,edx                    ; x − 6q = x % 6
```
`% 5` follows with `0xcccccccd` (≈ 2^34 / 5), and so on. There are no bounds checks on `a[j]` either: LLVM proved `j < 7`.

## 3. `evaluate`: the scoring formula, and SIMD

`evaluate` is inlined into `find_plan`, which is inlined into `spawn`. The bumpiness loop
```rust
for x in 0..W - 1 { bump += (hts[x] - hts[x + 1]).abs(); }
```
is fully unrolled (9 differences) and **vectorised**. `xmm` registers hold two `i64`s each, so one `psubq` does two subtractions:
```asm
    140005a4a  psubq  xmm3,xmm4                  ; two differences hts[x] − hts[x+1] at once
    ...
; in fn abs()                                    ; branch-free |d| for both lanes:
    140005a75  pshufd xmm4,xmm3,0xf5             ;   copy each lane's top 32 bits into both halves
    140005a7a  psrad  xmm4,0x1f                  ;   shift right 31 → all 1s if negative, else 0  (= mask)
    140005a7f  pxor   xmm3,xmm4                  ;   d ^ mask
    140005a83  psubq  xmm3,xmm4                  ;   (d ^ mask) − mask  =  |d|
; main.rs:173  bump += (hts[x] - hts[x + 1]).abs();
    140005a99  paddq  xmm1,xmm3                  ; add two results at once
    ...
    140005adb  pshufd xmm1,xmm0,0xee             ; finally fold the two lanes together
    140005ae0  paddq  xmm1,xmm0
    140005ae4  movq   rdx,xmm1                   ; back to a normal register
```
Four `xmm` registers × 2 lanes = 8 differences. The 9th is done the normal way (`neg` + `cmovs`).

Then the formula:
```asm
; main.rs:175  -510066 * agg + 760666 * cleared - 356630 * holes - 184483 * bump
    140005aec  imul   rcx,rcx,0xfffffffffff8378e         ; agg   × −510066
    140005af3  imul   r8,QWORD PTR [rsp+0x68],0xb9b5a    ; cleared × 760666 (read from the stack)
    140005afc  add    r8,rcx
    140005aff  imul   rcx,rax,0xfffffffffffa8eea         ; holes × −356630   ← Go and C# multiply by +356630 and subtract;
    140005b06  add    rcx,r8                             ;                      LLVM folds the minus into the constant
    140005b09  imul   rax,rdx,0xfffffffffffd2f5d         ; bump  × −184483
    140005b10  add    rax,rcx                            ; rax = score s
; in fn find_plan()
; main.rs:362  if !best.ok || s > best_score {           ; no return: the next line of the caller follows straight on
    140005b18  cmp    rax,rdx
```
Because `evaluate` is inlined, there's no `ret`. The score goes straight into `find_plan`'s comparison.

## Try it yourself
- Search for `panic_bounds_check`. Those are the bounds checks LLVM could *not* remove, with the panic call they lead to.
- Change `opt-level = 3` to `1` in `Cargo.toml`, run `.\disasm.ps1` again from the project folder, and compare how much less gets inlined and unrolled.
- [godbolt.org](https://godbolt.org) with `-C opt-level=3`: paste `next_rand` and `draw7` to see the same thing live.
