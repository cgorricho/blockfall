# Go edition: the assembly, explained

[blockfall-go.asm](blockfall-go.asm) is every function of package `main` (the game's own code) as it sits in `blockfall-go.exe`, in Intel syntax. Each group of instructions has a `; main.go:NNN` comment above it, with the Go line it came from. Start with [../../../ASSEMBLY.md](../../../ASSEMBLY.md) if assembly is new to you.

How it's made ([disasm.ps1](../../disasm.ps1)): GNU `objdump -d -M intel` gives Intel syntax, and `go tool objdump` gives the source line of each address. The script merges the two by address. Addresses below come from one build, and yours will differ.

The `.exe` is 4.6 MB, but the game's own code is only ~5,700 instructions. The rest is the **Go runtime** that every Go program carries: garbage collector, goroutine scheduler, maps, fmt, and so on.

## Things you will see in every Go function

```asm
main.(*game).draw7:
    140140180  cmp    rsp,QWORD PTR [r14+0x10]   ; is the stack nearly full?
    140140184  jbe    1401402fc                  ; yes → jump to the end, call runtime.morestack, retry
    14014018a  push   rbp                        ; normal function entry (save frame pointer,
    14014018b  mov    rbp,rsp                    ;  set up a new one,
    14014018e  sub    rsp,0x20                   ;  reserve 32 bytes of stack)
```
- **Stack check.** Goroutines start with a tiny stack (a few KB) that grows on demand. `r14` always points to the current goroutine, and `[r14+0x10]` is its stack limit. If `rsp` (the stack pointer) is at or below it, the runtime grows the stack and runs the function again. C and Rust have fixed stacks and no such check.
- **Register arguments.** The first argument (here the receiver `g`) arrives in `rax`, the second in `rbx`, then `rcx, rdi, rsi, …`. This is Go's own convention, not the Windows one.
- **`xmm15` is always zero.** Go uses it to clear memory quickly (see `evaluate` below).

## 1. `nextRand`: the random number generator (inlined)

There is no `main.(*game).nextRand` in the listing. It is small, so the compiler **inlined** it: the body was copied into its caller `draw7`. The comments still point at its source lines (97–101):

```asm
; main.go:97   x := g.rng
    140140270  mov    ebx,DWORD PTR [rsi+0x690]  ; ebx = g.rng   (g is in rsi; rng is at offset 0x690 inside the game struct)
; main.go:98   x ^= x << 13
    140140276  mov    edi,ebx                    ; edi = x       (keep a copy)
    140140278  shl    ebx,0xd                    ; ebx = x << 13
    14014027b  xor    ebx,edi                    ; ebx = (x << 13) ^ x
; main.go:99   x ^= x >> 17
    14014027d  mov    edi,ebx
    14014027f  shr    ebx,0x11                   ; 0x11 = 17. shr = unsigned shift (uint32)
    140140282  xor    ebx,edi
; main.go:100  x ^= x << 5
    140140284  mov    edi,ebx
    140140286  shl    ebx,0x5
    140140289  xor    ebx,edi
  ...
; main.go:101  g.rng = x
    140140299  mov    DWORD PTR [rsi+0x690],ebx  ; store the new state back
```
`ebx` and `edi` are the 32-bit halves of `rbx` and `rdi`, because `rng` is a `uint32`. The 32-bit operations wrap around on their own, exactly as the spec requires.

## 2. The 7-bag shuffle: `j := nextRand() % (i+1)`

```asm
; main.go:107  a := []int{0, 1, 2, 3, 4, 5, 6}
    1401401a1  mov    eax,0x38                   ; 0x38 = 56 bytes = 7 ints × 8 bytes
    1401401ad  ...
    1401401b2  call   runtime.mallocgcSmallNoScanSC6   ; allocate on the GC heap (it outlives the function, in g.bag)
    1401401b7  mov    QWORD PTR [rax+0x8],0x1    ; a[1] = 1
    ...                                          ; a[2..6]
    1401401df  mov    QWORD PTR [rax+0x30],0x6   ; a[6] = 6
                                                 ; no store for a[0] = 0: Go's allocator returns zeroed memory
; main.go:109  j := int(g.nextRand() % uint32(i+1))
    14014026c  lea    rcx,[rdx+0x1]              ; rcx = i + 1   (the divisor)
    ...         (inlined nextRand, above)
    14014028e  mov    eax,ebx                    ; eax = random number
    140140293  xor    edx,edx                    ; edx = 0 (div divides the 64-bit pair edx:eax)
    140140295  div    ecx                        ; eax = edx:eax / ecx, edx = remainder  ← the %
    140140297  mov    ecx,edx                    ; j = remainder
; main.go:110  a[i], a[j] = a[j], a[i]
    14014029f  mov    rbx,QWORD PTR [rdi+rcx*8]  ; rbx = a[j]   (rdi = &a[0], 8 bytes per int)
    1401402a3  mov    r9,QWORD PTR [rdi+r8*8]    ; r9  = a[i]
    1401402a7  mov    QWORD PTR [rdi+r8*8],rbx   ; a[i] = old a[j]
    1401402ab  mov    QWORD PTR [rdi+rcx*8],r9   ; a[j] = old a[i]
; main.go:108  for i := 6; i >= 1; i-- {
    1401402af  lea    rdx,[r8-0x1]               ; i--
    1401402b6  test   rdx,rdx
    1401402b9  jg     14014026c                  ; loop while i > 0
```
- `div` is the CPU's hardware divide. It is much slower than a multiply. Go keeps the loop as a loop, so the divisor is a variable and it has to use `div`. The Rust compiler avoids it (see `rust/asm/README.md`).
- There is **no bounds check** on `a[j]` and `a[i]`. The compiler proved that `j < i+1 ≤ 7` and `i ≤ 6` always hold.

The end of `draw7` shows the **garbage collector's write barrier**:
```asm
; main.go:115  g.bag = g.bag[1:]
    14014023b  cmp    DWORD PTR [rip+0x1d79ee],0x0   ; runtime.writeBarrier: is the GC marking right now?
    140140242  je     140140257                      ; no → plain store
    14014024b  call   runtime.gcWriteBarrier2        ; yes → tell the GC about the pointer we're about to overwrite
    140140257  mov    QWORD PTR [rax+0x678],rcx      ; g.bag.ptr = new pointer
```
Go's garbage collector runs while your code runs. Each time a pointer is stored in the heap, the compiler adds this check so the collector never misses a live object.

## 3. `evaluate`: the AI's scoring formula

The start clears the local array `hts [10]int64` using the always-zero register:
```asm
; main.go:260  var hts [W]int64
    140140da8  lea    rcx,[rsp]                  ; rcx = &hts
    140140dac  movups XMMWORD PTR [rcx],xmm15    ; write 16 zero bytes (2 int64s) at a time
    ...                                          ; 5 stores × 16 bytes = 80 bytes = 10 int64s
```
The bumpiness loop has no `if` jump. It uses a **conditional move**:
```asm
; main.go:278  d := hts[x] - hts[x+1]
    140140e4e  mov    r8,QWORD PTR [rsp+rdi*8]       ; r8 = hts[x]
    140140e52  mov    r9,QWORD PTR [rsp+rdi*8+0x8]   ; r9 = hts[x+1]
    140140e57  mov    r10,r8
    140140e5a  sub    r8,r9                          ; r8 = d
; main.go:280  d = -d
    140140e5d  sub    r9,r10                         ; r9 = -d  (computed in advance)
; main.go:279  if d < 0 {
    140140e60  test   r8,r8
; main.go:282  bump += d
    140140e63  cmovl  r8,r9                          ; if d < 0, r8 = -d   (no branch to mispredict)
    140140e67  add    rdx,r8                         ; bump += |d|
```
Then the formula itself, one `imul` per weight:
```asm
; main.go:284  return -510066*agg + 760666*int64(cleared) - 356630*holes - 184483*bump
    140140e73  imul   rsi,rsi,0xfffffffffff8378e ; agg     × -510066  (negative numbers are stored as two's complement)
    140140e7a  imul   rbx,rbx,0xb9b5a            ; cleared ×  760666  (cleared arrived in rbx, the 2nd argument)
    140140e81  lea    rax,[rbx+rsi*1]            ; rax = the sum of both (lea used as an add)
    140140e85  imul   rcx,rcx,0x57116            ; holes   ×  356630
    140140e8c  sub    rax,rcx
    140140e8f  imul   rcx,rdx,0x2d0a3            ; bump    ×  184483
    140140e96  sub    rax,rcx                    ; result in rax = the return value
    140140e99  add    rsp,0x50                   ; free the stack frame
    140140e9d  pop    rbp
    140140e9e  ret
```
After the `ret` comes `call runtime.panicBounds`. That's where the bounds checks on `b[y][x]` jump if an index is ever out of range. It never runs, but Go guarantees a clean panic and never a memory overwrite.

## Try it yourself
- Search the listing for `main.(*game).findPlan` and follow the calls to `main.kickRotate` and `main.evaluate`.
- `go build -gcflags=-m` (in `go/`) prints every inlining decision, such as "inlining call to (*game).nextRand".
- `go build -gcflags=-S` prints the compiler's own assembly (Go's Plan 9 syntax, `MOVQ src, dst`, the reverse of Intel order).
