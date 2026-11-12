# Task 1.5 report: built-in functions and MT19937

## Implemented
- `src/mt19937.rs`: from-spec MT19937 (`init_genrand`, `genrand_int32`), `Mt19937::seeded/next_u32/rand`, `NSEEL_SEED = 0x4141f00d`. Held in `GlobalMem.rng` (shared by every Vm); borrowed only for one draw.
- `src/builtins.rs`: rand wired; sin/cos/tan x87 huge-argument handling; docs updated. Tests in `src/builtins_tests.rs` (20 tests).
- `mem.rs`, `lib.rs`: `rng` field, `pub mod mt19937`.

## Pinned against the C
- rand: `nseel_int_rand` = `genrand_int32() * (1.0/(double)0xFFFFFFFF) * x`, x = floor(x) clamped to >= 1 (NaN stays NaN). Range is CLOSED [0, x] (0xFFFFFFFF maps to exactly 1.0), not [0, x). Brief's `rand_is_below_max` became `rand_range_is_closed_and_floors_and_clamps`. First outputs for a fresh generator pinned (0.22772551216830628, 0.3407441909286995, then rand(10): 8.93651768540417, 9.49593038519284). The C seeds lazily on first call; seeding at GlobalMem construction is equivalent.
- MT: seed 5489 -> 3499211612, 581869302; seed 0x4141f00d -> 978073627, 1463485156, 3838205119, 4078471044, and output #1000 = 1961510904 (past first twist). Cross-checked with an independent Python implementation.
- sign/min/max/sqrt/abs/sqr/atan2 order/band/bor/sigmoid (strict `fabs > 0.00001`): verified against asm/C helpers; 1.4 implementations were correct, now tested incl. NaN, -0.
- invsqrt: x87 stub = 0x5f3759df fast estimate + one Newton step (not 1/sqrt); exact values pinned from an independent Python model (invsqrt(4)=0.49915357479239103).
- floor/ceil: libm floor/ceil (no int conversion). log/log10 use fyl2x on x87 (ln2*log2 x); we use libm ln/log10 (identical specials: log(0)=-inf, log(<0)=NaN; may differ in last ulp). pow: libm pow (pow(-8,1/3)=NaN, pow(0,0)=1).
- sin/cos: x87 fsin/fcos leave finite |x| >= 2^63 unchanged (mirrored); infinity gives NaN. tan: fptan out of range pushes nothing, so `fstp st(0); fstp [dst]` pops an empty stack and stores the indefinite NaN: tan(huge)=NaN (mirrored). Below 2^63 uses libm (x87's 66-bit pi reduction differs in low bits for large args; not reproduced; documented).
- memcpy/memset return dst; freembuf(n) is a no-op returning n (ledger 1.3 ruling).

## TDD evidence
Honest note: tests were written together with/after the implementation in one pass, so no separate RED run was recorded. Rand tests would fail against the former `Rand => 0.0` stub by inspection.
GREEN: `cargo test -p md-eval builtins` -> 20 passed; `cargo test --workspace` all ok (md-eval lib 102 passed); clippy -D warnings and fmt --check clean.

## Concerns
- x87-vs-libm last-ulp differences for sin/cos/tan/log/exp/pow are not bit-reproduced (cannot be without software x87).
- Test `sign` NaN assertion only checks the result is +-1 (the NaN sign bit depends on the producing op).
