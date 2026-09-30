# WASI shared memory grow — Design & Principles

## Summary

The threaded WASI binding (`wasm32-wasip1-threads`) traps with "memory access out
of bounds" under concurrent load because of a V8 bug: a thread that did not run
`memory.grow` keeps a stale memory size in its running wasm code, and `memory.fill`,
`memory.copy`, Liftoff atomics and atomic wait/notify are bounds-checked against
it; on hosts without V8's wasm trap handler, plain loads and stores are too.
Rolldown works around it in its own allocator layer: one lock around every call
into wasi-libc's dlmalloc, a `memory.grow(0)` refresh right after the lock is
taken when another thread has seen a larger memory, and an `sbrk` hook that
publishes every growth before the lock is released. The hook also hands dlmalloc
the pages the loader already created before it grows, so most loads never grow
the memory at all. The same refresh runs at every scheduler handoff (task poll,
blocking closure start), for work that moves between threads. The workaround is
what lets the threaded build default to the MultiThread flavor (2 workers). It
lives only in the threaded build and goes away once the Node versions we support
ship the V8 fix. For the machinery, see [implementation.md](./implementation.md).
Addresses #10697.

## The bug

```
thread A (out of heap)            thread B (long wasm activation)
  dlmalloc -> sbrk
  memory.grow(n)  ── V8 refreshes A's cached size only
  hands out / frees blocks          malloc() -> block in the new pages
                                    memset / memcpy on it
                                      memory.fill / memory.copy
                                      bounds check vs B's OLD size -> trap
```

- wasi-libc's dlmalloc grows the one shared memory from whichever thread runs out
  of heap (`sbrk` is the only `memory.grow` site in the module).
- The stale size stays until the thread handles V8's grow interrupt, which only
  some code paths do (see "What refreshes a stale thread" below). Returning to JS
  and entering wasm again does not do it by itself. So a thread that stays in
  wasm (rayon workers, emnapi async work threads, dlmalloc's spin lock) keeps the
  old size.
- Plain loads and stores do not trap, and neither do TurboFan atomics: with the
  wasm trap handler (default Node on x64 and on most arm64 hosts, see Remaining
  gaps) they are checked by guard pages against the real size.
- It is not MultiThread-specific: the published CurrentThread artifact
  (`@rolldown/binding-wasm32-wasi@1.2.8`) traps under 16 concurrent
  `parse()` / `transform()` calls x3 (20/20), because emnapi async work threads
  allocate too.

### Evidence (node 24.21 arm64, 2026-09-30)

- Innermost instruction of every trap: `memory.copy` or `memory.fill` (133/133 in
  the repro lane, 39/39 in the judge lane). Never a plain load or store.
- Pure V8 repro (two workers, one wasm module, shared memory): after worker A grows,
  worker B's `memory.fill` / `memory.copy` / `i32.atomic.store` on the new page trap
  at iteration 0-1; a plain store passes 3000/3000; `memory.size` before the fill
  still traps; `memory.grow(0)` before the fill passes.
- Two-worker handoff probe (`scripts/wasi/check-v8-shared-memory-grow.mjs`, see
  Mechanism below; worker B is already inside one long activation that never
  allocates or grows, A grows one page per round and hands B a pointer into it),
  20 runs x 200 rounds per cell, cells are node 24.12.0 / 24.21.0:

  | B's activation               | no refresh         | `memory.grow(0)` | `memory.size`      |
  | ---------------------------- | ------------------ | ---------------- | ------------------ |
  | cold (first call, Liftoff)   | 20/20 / 20/20 trap | 0/20 / 0/20      | 20/20 / 20/20 trap |
  | warm (tiered up to TurboFan) | 1/20 / 3/20 trap   | 0/20 / 0/20      | 0/20 / 6/20 trap   |
  | cold, `--liftoff-only`       | 0/20 / 0/20        | 0/20 / 0/20      | 1/20 / 0/20 trap   |

  Cold traps land in round 1 or 2; each of `memory.copy`, `memory.fill` and
  `i32.atomic.rmw.add` traps on its own. `memory.size` returns the STALE size:
  in every `memory.size` trap it read one page less than the memory had. A later
  run of the committed script (2026-09-30) got warm 13/20 (24.12.0) and 8/20
  (24.21.0), and cold `--liftoff-only` 1/20 with no refresh (24.12.0).

- `--liftoff-only` is a near-pass control, not a clean one: the first pure-V8
  runs had 0/900 traps and rolldown MultiThread passed 10/10, but the handoff
  probe trapped 1/20 (above) and the earlier grow-race atomic shape trapped 1/5
  (at iteration 2182 of 3000).
- V8 fixed exactly this upstream: "[wasm] Atomic memory.size and dynamic bounds
  checks for shared memory", v8/v8@34241014663390c72e08c123faef6fedf395be8e
  (crrev 8466625, V8 bugs 529880019 / 533026477, 2026-09-29). Not in any Node
  release as of 2026-09-30.
- #10697 first read this as heap metadata corruption. Its V8 control used a plain
  store, which passes here too; `fill` / `copy` / atomics trap, a pre-grown heap
  cures it, and an allocator race would not be cured by pre-growing.

### Mechanism (V8 source, node v24.12.0 `deps/v8`)

```
worker A: memory.grow(n)
  WasmMemoryObject::Grow -> BroadcastSharedWasmMemoryGrow   (src/wasm/wasm-objects.cc:1223)
    every other isolate: stack_guard()->RequestGrowSharedMemory()
    own isolate only: UpdateSharedWasmMemoryObjects       (src/objects/backing-store.cc:855-869)

worker B: size stays stale until B handles that interrupt
  GROW_SHARED_MEMORY -> UpdateSharedWasmMemoryObjects       (src/execution/stack-guard.cc:337-339)
  meanwhile memory.copy / memory.fill check against B's per-isolate instance size
    memory_copy_wrapper / memory_fill_wrapper -> trusted_data->memory_size()
                                                            (src/wasm/wasm-external-refs.cc:786-790, 806-807)
```

- The deterministic case is a thread whose activation is still Liftoff code: its
  first call under Node's default dynamic tiering (wasm has no OSR, so a rayon /
  emnapi worker loop entered once stays in that frame). With dynamic tiering
  Liftoff emits no loop stack check (src/wasm/baseline/liftoff-compiler.cc:1415-1420);
  it handles interrupts only when the tier-up budget runs out
  (`Runtime_WasmTriggerTierUp`, src/runtime/runtime-wasm.cc:786-794) or at a
  function-entry stack check. So a call-free loop keeps the old size for a whole
  round: cold, 20/20 trap.
- A warm TurboFan activation checks the stack on every loop pass, so it only
  misses a grow requested in the same pass: a race, 1-6/20 in the probe.
- `--liftoff-only` turns dynamic tiering off, so Liftoff loops stack-check again:
  the same one-pass window as TurboFan, hit less often (1/20). Flag probes on
  24.12.0 (cold, 5 runs each): `--no-wasm-dynamic-tiering` 0/5,
  `--no-wasm-tier-up` and `--wasm-tiering-budget=2000000000` 5/5 trap,
  `--no-liftoff` 3/5 trap.
- `memory.grow(0)` on B goes through the same `Grow`, which ends in
  `UpdateSharedWasmMemoryObjects` for B's own isolate (backing-store.cc:869), so
  the size is current before the touch: 0/240 across all probe rows.
  `memory.size` reads the stale size and handles no interrupt.

Which operations check the cached size:

| operation                                  | trap handler on (default) | trap handler off / `--wasm-enforce-bounds-checks` |
| ------------------------------------------ | ------------------------- | ------------------------------------------------- |
| `memory.fill` / `memory.copy` (both tiers) | cached size, can trap     | cached size, can trap                             |
| Liftoff atomics, wait/notify (both tiers)  | cached size, can trap     | cached size, can trap                             |
| TurboFan atomic load / store / rmw         | guard pages, safe         | cached size, can trap                             |
| plain load / store                         | guard pages, safe         | cached size, can trap                             |

Liftoff forces the check on atomics (src/wasm/baseline/liftoff-compiler.cc:5649-5650);
TurboFan omits it under the trap handler
(src/wasm/turboshaft-graph-interface.cc:4094-4096, 7162-7165) and keeps it for
wait/notify (:3921, :3943). Measured: a TurboFan `i32.atomic.rmw.add` on a new
page passed 20/20 while the thread's size was stale in 196-199 of 200 rounds.

#### What refreshes a stale thread

Every "yes" below is one code path that runs `StackGuard::HandleInterrupts` for
B's pending grow interrupt. Two-worker probes, node 24.12.0 and 24.21.0, 20 runs x
200 rounds per cell (the event-loop row: 40 runs of a runtime-shaped probe),
"traps" = runs with "memory access out of bounds":

| B does this between "A grew" and "B touches"                         | refreshes? | traps           |
| -------------------------------------------------------------------- | ---------- | --------------- |
| `memory.grow(0)` (in wasm or in JS)                                  | yes        | 0/20, all tiers |
| `wait32` on a private word, expected = its value, timeout 0          | yes        | 0/160           |
| a futex wait (wasm `wait32` or JS `Atomics.wait`) that sleeps        | yes        | 0/20            |
| call a Liftoff function (its entry stack check)                      | yes        | 0/20            |
| call a TurboFan non-leaf function that is not inlined                | yes        | 0/20            |
| call any JS function or JS builtin (emnapi imports, `Atomics.store`) | yes        | 0/20            |
| return to the Worker event loop (postMessage, `waitAsync`)           | yes        | 0/40            |
| nothing (stay in one activation)                                     | no         | 20/20           |
| `memory.size`                                                        | no, stale  | 20/20 cold      |
| a futex wait that returns not-equal                                  | no         | 20/20           |
| `memory.atomic.notify`                                               | no         | 20/20           |
| call a TurboFan leaf function, or an inlined one                     | no         | 20/20 cold      |
| return to JS and enter wasm again, nothing else                      | **no**     | 20/20           |

- A futex wait handles the interrupt only after its value check, so a wait that
  returns not-equal leaves the size stale; one that sleeps is woken by the grow
  and refreshes (src/execution/futex-emulation.cc:395, 405-428). So an idle
  worker parked in a futex is current when it wakes.
- The JS -> wasm wrapper checks only the real stack limit, not interrupts
  (src/builtins/arm64/builtins-arm64.cc:4229-4470). The refresh people see after a JS call comes
  from the called function's entry stack check or from a JS builtin on the way.

### Workaround results (release-wasi, interleaved A/B, same loads)

| run (5 loads x 10 interleaved rounds unless noted)            | base        | heap-sync    |
| ------------------------------------------------------------- | ----------- | ------------ |
| landing tree, per load: MultiThread w4 16 builds              | 8/10 trap   | 10/10 pass   |
| — MultiThread w4 16 builds + JS plugin x2 waves               | 10/10 trap  | 10/10 pass   |
| — MultiThread w4 parse 16x3                                   | 10/10 trap  | 10/10 pass   |
| — CurrentThread parse 16x3                                    | 10/10 trap  | 10/10 pass   |
| — CurrentThread transform 16x3                                | 1/10 trap   | 10/10 pass   |
| landing tree, stress: 32 builds x 3 waves + JS plugin (MT w4) | -           | 20/20 pass   |
| judge lane total                                              | 39/50 trap  | 50/50 pass   |
| code lane, debug                                              | 83/105 fail | 105/105 pass |
| code lane, release-wasi                                       | 49/70 fail  | 70/70 pass   |

The MultiThread rows were measured before the threaded WASI binding accepted
MultiThread (they lifted the guard in a local build). That binding now defaults to
MultiThread with 2 workers; `ROLLDOWN_RUNTIME=single` selects CurrentThread, which
the CurrentThread rows cover. The "landing tree" rows ran on the commit that adds this workaround.

The CI stress script `packages/rolldown/tests/wasi/threaded-memory-stress.mjs`
runs these loads in one pass (builds with a JS plugin, parse and transform, each
under CurrentThread and MultiThread w4, plus builds on the default MultiThread w2).
The base vs heap-sync rounds below ran its first six-case version. Against the release wasm files of
the table (base vs heap-sync), 3 interleaved rounds: base failed every round
(4 or 5 of 6 cases trapped with "memory access out of bounds"), heap-sync passed
every round.

Timing is at noise level (CurrentThread release median 729 vs 723.5 ms;
MultiThread vs a pre-grown base 479.5 vs 480 ms).

Keeping the grow-ahead alive (the earlier `black_box` fix, before the lock and the
break; principle 4), release-wasi, same five loads, 5 interleaved rounds, all 50
runs pass. Medians (ms), without -> with:

| load                                 | without | with | speedup |
| ------------------------------------ | ------- | ---- | ------- |
| MultiThread w4 16 builds             | 700     | 419  | 1.7x    |
| MultiThread w4 16 builds + JS plugin | 2654    | 1702 | 1.6x    |
| MultiThread w4 parse 16x3            | 556     | 251  | 2.2x    |
| CurrentThread parse 16x3             | 796     | 248  | 3.2x    |
| CurrentThread transform 16x3         | 748     | 792  | noise   |
| MultiThread w4 three10x x4 (3 runs)  | 17108   | 1495 | 11x     |

The "without" runs are noisy (16 builds: 615-1259 ms), the "with" runs much
less so (319-506 ms). three10x x4 grows the heap to about 2.2 GiB of memory.

The scheduler handoff refresh (principle 2), release-wasi, 10 interleaved rounds
of the five judge loads, without vs with the hook: 100/100 pass. Medians (ms):

| load                                 | without | with  |
| ------------------------------------ | ------- | ----- |
| MultiThread w4 16 builds             | 567     | 563.5 |
| MultiThread w4 16 builds + JS plugin | 1680    | 1671  |
| MultiThread w4 parse 16x3            | 437     | 444.5 |
| CurrentThread parse 16x3             | 450     | 440.5 |
| CurrentThread transform 16x3         | 604.5   | 601   |

#### The allocator lock and the break (2026-10-01)

Three release-wasi artifacts through the CI stress script's options
(`threaded-memory-stress.mjs`, MultiThread w4, node 24.21 arm64): **base**
db90bbbcf (heap-sync allocator, dead grow-ahead), **grow-ahead** 64409a594 (the
`black_box` fix; both built before the branch's main merge), **lock** this change.
30 s per run.

| load (runs)                                                   | base                     | grow-ahead           | lock                      |
| ------------------------------------------------------------- | ------------------------ | -------------------- | ------------------------- |
| builds + JS plugin, `--wasm-enforce-bounds-checks` (20)       | 0 pass (16 hang, 4 trap) | 18 pass (2 hang)     | 20 pass, 0 grows          |
| the same, `--disable-wasm-trap-handler` (10)                  | 0 pass (9 hang, 1 trap)  | 10 pass              | 10 pass, 0 grows          |
| 16 builds, loader memory = module minimum, bounds checks (20) | 0 pass (20 trap)         | 18 pass (2 trap)     | 20 pass, 12 grows each    |
| builds + JS plugin, same memory and flag (20)                 | 0 pass (15 hang, 5 trap) | 17 pass (3 hang)     | 20 pass, 13-14 grows each |
| hold 1536 MiB in wasm, then builds + JS plugin (3)            | 0 pass (os error 28)     | 0 pass (os error 28) | 3 pass                    |
| 16 builds and default MT w2 builds, default loader (3 each)   | -                        | -                    | 6 pass, 0 grows           |

The held block sat at 0x40000010-0xa0000010 on the grow-ahead artifact (wasi-libc's
`sbrk` starts at the loader's 1 GiB); with the break it starts right after
`__heap_end`, and the build that follows stays under 2^31 (29442 pages of memory in
the debug run). Every lock run reported 0 blocks past the thread's refreshed size.
The debug artifact CI builds passes the same CI steps: 20/20, 10/10, 20/20 (12
grows each), 20/20 (14 grows each), ceiling and no-growth pass, and the default
stress cases report 0 grows.

Cost, release-wasi, judge loads, 10 interleaved rounds, all 150 runs pass. All
three artifacts are built from the same tree (the branch after its main merge):
the grow-ahead code, this change, and this change with the lock removed (not safe,
measurement only). Medians (ms) with [min-max]:

| load                                 | grow-ahead       | lock               | lock removed     |
| ------------------------------------ | ---------------- | ------------------ | ---------------- |
| MultiThread w4 16 builds             | 310.5 [303-326]  | 334.5 [325-349]    | 312 [307-339]    |
| MultiThread w4 16 builds + JS plugin | 1518 [1472-1563] | 1589.5 [1559-1693] | 1540 [1510-1629] |
| MultiThread w4 parse 16x3            | 218.5 [216-231]  | 241 [238-275]      | 228 [224-358]    |
| CurrentThread parse 16x3             | 216.5 [216-237]  | 240.5 [238-258]    | 231.5 [226-260]  |
| CurrentThread transform 16x3         | 623.5 [615-667]  | 640 [626-701]      | 624.5 [618-681]  |

So this change costs about 8% on 16 builds, 5% with a JS plugin, 10-11% on parse
and 3% on transform against the grow-ahead fix, and keeps most of that fix's
1.6-3.2x gain over base. With the lock removed the builds match the grow-ahead
fix, so the lock is the build cost; parse keeps 4-7% (the wrappers and the direct
dlmalloc entry path; not isolated). Two variants did not help and were dropped:
zeroing (`alloc_zeroed`) and copying (`realloc`) outside the lock, and a plain
load for the in-lock size check without `after` (16 builds 343-348 ms vs 344.5
for this change, in a noisier set). Not profiled.

## Design principles

1. **Refresh under the allocator lock, and publish growth before unlocking.**
   Without the trap handler the first stale access is dlmalloc's own chunk-header
   store, made while dlmalloc holds its lock, so a refresh in a wrapper before the
   call cannot close it: a thread waiting on dlmalloc's lock gets it right after the
   growing thread unlocks and before that thread publishes the new size (see
   Rejected alternatives). So Rolldown owns the lock:

   ```
   LOCK ─> behind (MAX_SEEN_PAGES > LOCAL_PAGES)? memory.grow(0)
       ─> dlmalloc call ─> sbrk hook grows ─> memory.grow(0), publish MAX_SEEN_PAGES
   UNLOCK (Release) ─> next holder's Acquire load sees the new size before dlmalloc runs
   ```

   `sbrk` is the only code that grows the memory, and dlmalloc only calls it under
   this lock, so every thread that touches the heap is current before its first
   store. calloc's memset and realloc's memcpy run inside dlmalloc, after the
   refresh, so they need no special handling. The lock spins like dlmalloc's own
   (`sched_yield` every 64 spins) and never uses `memory.atomic.wait`, which traps
   on a browser main thread. It adds no new serialization: dlmalloc already had one
   global lock.

2. **Refresh at the scheduler handoff too.** A MultiThread task can allocate a
   `Vec` on worker A, yield, and resume on worker B, then `copy_from_slice` /
   `write_bytes` / an atomic into the existing capacity without entering the
   allocator on B. A blocking closure built on one thread runs on another. So every
   task poll and every blocking closure start refreshes when another thread has
   seen a larger memory (`MAX_SEEN_PAGES > LOCAL_PAGES`). Cost on the hot path: one
   thread-local read and one atomic load.
3. **`memory.grow(0)`, not `memory.size`.** Only a grow updates this thread's
   memory size (see Mechanism); `memory.size` returns the same stale size the
   bounds checks use and updates nothing (one page short in every trap of the
   probe; a `memory.size` variant fails 10/10 in release).
4. **Use the memory the loader created before growing it.** wasi-libc's `sbrk`
   starts at `memory.size`, which is the loader's initial memory (16384 pages,
   1 GiB, from `napi.wasm.initialMemory`), so the pages between `__heap_end` (the
   module's own initial memory, about 64 MiB) and 1 GiB were never used and the
   heap grew from 1 GiB on. The hook keeps its own break starting at `__heap_end`:

   ```
   before: [stack+data 64 MiB][ unused 960 MiB ][ heap, grows from 1 GiB ][ 2^31 wall ]
   after:  [stack+data 64 MiB][ heap, no growth up to ~960 MiB ][ grows 16 MiB+ ][ 2^31 wall ]
   ```

   Those pages exist on every thread from instantiation, so no growth means no
   stale size to begin with, and the heap reaches about 1.94 GiB before a pointer
   crosses 2^31, where Node's `node:wasi` rejects it (`EINVAL`, os error 28, in any
   WASI call that takes a pointer; nodejs/node#62671, open). Before, it failed at
   about 1 GiB. When the break must pass the current memory, the hook grows at least
   16 MiB at once: every growth interrupts every other thread, and V8 changes the
   page permissions of the whole memory on each grow
   (`GrowWasmMemoryInPlace`, src/objects/backing-store.cc:534), so a few large
   steps are much faster than many small ones (the grow-ahead numbers in Workaround
   results). The hook grows by itself, so the old `malloc` + `free` grow-ahead
   (and the `black_box` that kept LLVM from deleting it) is gone.

5. **The allocator exports JS uses must be the wrappers, and a missed rename must
   fail loudly.** `@emnapi/core` calls the module's `malloc` / `free` exports from
   JS, and `--wrap=malloc` removes the export named `malloc`. A post-link step
   (`scripts/wasi/rename-wasm-allocator-exports.mjs`, run by `build-binding.ts`)
   adds `malloc` / `free` for the locked JS entry points. If a build path skips it,
   the module does not load ("malloc is not exported") and the dist check fails,
   instead of shipping dlmalloc's unlocked entry points.
6. **Threaded build only, no behavior change elsewhere.** The single-thread build
   has one thread and never sees a stale size; native builds keep mimalloc.

## Rejected alternatives

- **A bigger loader `initial`, or pre-growing through dlmalloc.** A bigger
  `initial` did nothing on its own: wasi-libc's `sbrk` starts at the top of memory
  (1 GiB, 1.5 GiB, 1.83 GiB initial all trapped 10/10). Pre-growing through
  dlmalloc (512 MiB) passed 45/45, but only until a load outgrew it. The break
  (principle 4) is the same idea done right: it uses the pages the loader already
  made, without allocating them.
- **Link the module's initial memory to the loader's** (`--initial-memory` = 1 GiB,
  so dlmalloc's first segment covers it). Same effect on growth (0 grows up to
  about 960 MiB, measured), but it ties the module to `napi.wasm.initialMemory`: a
  loader or host that passes a smaller memory gets a `LinkError`.
- **`memory.size` as the refresh.** Does not reload V8's bounds (principle 3).
- **Refresh on the emnapi / JS side.** The traps are inside wasm (Rust and C
  memset / memcpy, dlmalloc's header stores), in activations that never return to
  JS, so a JS-side refresh never runs on the trapping thread.
- **`--liftoff-only`.** Makes the trap rare but not impossible (1/20 and 1/5 in
  the probes, see Evidence), and removes the MultiThread speedup.
- **Refresh before the allocation** (in every wrapper, run `memory.grow(0)` first
  when `MAX_SEEN_PAGES > LOCAL_PAGES`, with dlmalloc's own lock inside). With
  `--wasm-enforce-bounds-checks` on the release-wasi artifact it failed 110/110
  (61 hang, 49 trap), every trap the same dlmalloc chunk-header store: the waiter
  takes dlmalloc's lock before the grower publishes (principle 1).
- **An outer lock that only adds reload points** (an earlier experiment: a lock
  around the allocator with no refresh under it and no `sbrk` hook). It passed the
  default-host repro only because the extra calls gave V8 more places to reload.
  The adopted lock is different: it refreshes after it is taken, and `sbrk`
  publishes before it is released.
- **Replace the allocator in Rust** (a `#[global_allocator]` over dlmalloc-rs).
  Alone it leaves emnapi's and wasi-libc's C calls (about 20-30 thousand per load,
  one per threadsafe-function call and per async work) on libc's dlmalloc, the same
  heap, so it does not close the bug; defining all ten libc allocator symbols in
  Rust would, but it is a bigger change for the same result.
- **Export the wrapper as `malloc` without a post-link step.** Rust cannot give an
  export a name other than its symbol on stable (`global_asm!` is unstable on
  wasm), and `--export=__real_malloc` exports the unlocked dlmalloc entry under
  `malloc`. A C shim with `__attribute__((export_name("malloc")))` works but needs a
  wasm C compiler in every build; teaching emnapi to read another export name needs
  an emnapi release. Both belong in the napi-rs follow-up that moves this layer
  upstream.
- **Host-driven worker loops** (a tokio-style hosted event loop, or a JS-resident
  loop). They refresh only between tasks, where the handoff hook already does; the
  traps are mid-task.

## Remaining gaps

| gap                                                          | before the lock (db90bbbcf)                      | now                                                                                |
| ------------------------------------------------------------ | ------------------------------------------------ | ---------------------------------------------------------------------------------- |
| G1: dlmalloc header store on a host without the trap handler | trap inside dlmalloc's lock, then a **hang**     | closed on the measured host (Workaround results): the store runs after the refresh |
| G2: a block received mid-poll after another thread grew      | open, rare                                       | open, but only once the heap passes the reserve (about 960 MiB); not observed      |
| a crash while a thread holds the allocator lock              | the other threads spin (dlmalloc's lock)         | the same (our lock)                                                                |
| heap pointer at or above 2^31                                | `EINVAL` from `node:wasi` at about 1 GiB of heap | at about 1.94 GiB of heap                                                          |

- **G2.** A block that reaches a running thread mid-poll (a channel message, an
  `Arc`, a threadsafe-function call on the JS thread) and is filled / copied / used
  with atomics there (or, without the trap handler, loaded or stored) before that
  thread's next allocation or poll boundary can still hit a stale size, if another
  thread grew the memory in between. Task migration and blocking-closure entry are
  covered by the handoff refresh (principle 2), and every allocator entry refreshes.
  Below the reserve nothing grows, so there is no stale size to hit; the forced
  growth runs (loader memory at the module minimum, 12-14 grows per run, under
  `--wasm-enforce-bounds-checks`) did not hit it either. Not proven closed; the V8
  fix closes it.
- **Crash inside the lock.** If a thread dies while it holds the lock (any trap in
  dlmalloc), the others spin on `sched_yield` and the loader's worker-crash latch
  cannot run, as with dlmalloc's own lock before.
- **Not measured:** x64, Linux and Windows hosts, real hosts without the handler
  (AIX, 32-bit Windows, FreeBSD arm64), and browsers (the wasi-browser loader asks
  for the same 16384 pages, so the break should behave the same). The flag runs
  used Node's `--wasm-enforce-bounds-checks` and `--disable-wasm-trap-handler` on
  macOS arm64.

- **Which hosts lack the handler** (where G1 mattered by default): Node's bundled
  `deps/v8/src/trap-handler/trap-handler.h` (v24.12.0 = V8 `13.6-lkgr`) sets
  `V8_TRAP_HANDLER_SUPPORTED` only for x64 on Linux (not Android), Windows,
  macOS and FreeBSD; arm64 on Linux (not Android), Windows and macOS; loong64
  and riscv64 on Linux. Older majors are narrower for arm64: v22.23.3 (V8 12.4)
  has macOS and Linux only, v20.20.2 (V8 11.3) macOS only. Node's `src/node.cc`
  (all three) installs the handler only on `__APPLE__ || __linux__ || _WIN32`
  and not under `--disable-wasm-trap-handler`. Node has no runtime check for
  it: no `process.config.variables` key, no `process.features` field; only the
  flag in `process.execArgv` / `NODE_OPTIONS` can be seen.

  | Official Node build        | Handler       | Native rolldown binding     |
  | -------------------------- | ------------- | --------------------------- |
  | darwin x64 / arm64         | yes           | yes                         |
  | linux-x64, win-x64         | yes           | yes                         |
  | linux-arm64                | Node 22+ only | yes                         |
  | win-arm64                  | Node 24+ only | yes                         |
  | linux-ppc64le              | no            | yes (`linux-ppc64-gnu`)     |
  | linux-s390x                | no            | yes (`linux-s390x-gnu`)     |
  | aix-ppc64                  | no            | **no** -> WASI fallback     |
  | linux-armv7l (Node 20, 22) | no            | yes (`linux-arm-gnueabihf`) |
  | win-x86 (Node 20, 22)      | no            | **no** -> WASI fallback     |

  Node 24 no longer ships linux-armv7l or win-x86 (nodejs.org/dist/index.json).
  Outside the official matrix, Android (V8 refuses the handler there) has native
  bindings, and FreeBSD arm64 has none. So the threaded WASI package runs without
  the handler by default on AIX, 32-bit Windows and FreeBSD arm64, and elsewhere
  only when forced (`NAPI_RS_FORCE_WASI`) or when the native binding fails to
  load.

## When to remove

When every Node version the threaded WASI package supports ships
v8/v8@34241014663390c72e08c123faef6fedf395be8e (or a backport of it), delete
`crates/rolldown_binding/src/wasm_heap_sync.rs`, its `#[global_allocator]` in
`lib.rs`, the `--wrap` link args in `build.rs`, the export rename
(`scripts/wasi/rename-wasm-allocator-exports.mjs`, its call in
`packages/rolldown/build-binding.ts`, and `scripts/wasi/wasm-sections.mjs` once
nothing else reads it), the handoff hook
(`crates/rolldown_utils/src/thread_handoff.rs`, its re-exports in
`rolldown_utils/src/lib.rs`, and its registration and `spawn_blocking` wrap in
`rolldown_binding/src/async_runtime.rs`), the heap-sync export checks in
`scripts/wasi/check-wasi-dist-files.mjs`, the heap-sync options and scripts of
`packages/rolldown/tests/wasi/threaded-memory-stress.mjs` with their CI steps,
`scripts/wasi/check-v8-shared-memory-grow.*`, the `check:v8-shared-memory-grow`
script in the root `package.json`, and this folder. Confirm first on each of those
Node versions with `pnpm check:v8-shared-memory-grow` and
`pnpm check:v8-shared-memory-grow --activation=warm`: remove the workaround only
when case 1 stops trapping in both (verdict `ABSENT`). Decide then whether to keep
the `sbrk` break (principle 4, `--wrap=sbrk` and `__wrap_sbrk` without the lock):
until Node accepts WASI pointers at or above 2^31, it is what raises the usable
heap from about 1 GiB to about 1.94 GiB.

## Related

- [implementation.md](./implementation.md) — the machinery that realizes this
- `../async-runtime/wasi-flavor-design.md` — the two WASI flavors and their loaders
