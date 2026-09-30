# WASI shared memory grow — Design & Principles

## Summary

The threaded WASI binding (`wasm32-wasip1-threads`) traps with "memory access out
of bounds" under concurrent load because of a V8 bug: a thread that did not run
`memory.grow` keeps a stale memory size in its running wasm code, and `memory.fill`,
`memory.copy`, Liftoff atomics and atomic wait/notify are bounds-checked against
it. Rolldown works around it in its own allocator: after an allocation that may
sit in pages this thread has not seen, the thread runs `memory.grow(0)`, which
makes V8 reload the size. The same refresh runs at every scheduler handoff (task
poll, blocking closure start), for work that moves between threads. The
workaround is what lets the threaded build default to the MultiThread flavor (2
workers). It lives only in the threaded build and goes away once the Node
versions we support ship the V8 fix. For the machinery, see
[implementation.md](./implementation.md). Addresses #10697.

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

Keeping the grow-ahead alive (`black_box`, principle 3), release-wasi, same five
loads, 5 interleaved rounds, all 50 runs pass. Medians (ms), without -> with:

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

The scheduler handoff refresh (principle 1), release-wasi, 10 interleaved rounds
of the five judge loads, without vs with the hook: 100/100 pass. Medians (ms):

| load                                 | without | with  |
| ------------------------------------ | ------- | ----- |
| MultiThread w4 16 builds             | 567     | 563.5 |
| MultiThread w4 16 builds + JS plugin | 1680    | 1671  |
| MultiThread w4 parse 16x3            | 437     | 444.5 |
| CurrentThread parse 16x3             | 450     | 440.5 |
| CurrentThread transform 16x3         | 604.5   | 601   |

## Design principles

1. **Refresh where new pages enter a thread: the allocator and the scheduler
   handoff.** The trapping operation is the first memset / memcpy / atomic on a block
   in pages this thread has not seen. Pages reach a thread in two common ways:
   - it allocates the block itself. Refreshing right after the allocation, before
     the first touch, closes the window for every block the thread allocates.
   - work moves onto it. A MultiThread task can allocate a `Vec` after growth on
     worker A, yield, and resume on worker B, then `copy_from_slice` /
     `write_bytes` / an atomic into the existing capacity without allocating on B.
     A blocking closure built on one thread runs on another. So every task poll and
     every blocking closure start refreshes when another thread has seen a larger
     memory (`MAX_SEEN_PAGES > LOCAL_PAGES`). The scheduler queue orders A's
     allocation before B's poll, so B's check sees A's size. Cost on the hot path:
     one thread-local read and one atomic load.
2. **`memory.grow(0)`, not `memory.size`.** Only a grow updates this thread's
   memory size (see Mechanism); `memory.size` returns the same stale size the
   bounds checks use and updates nothing (one page short in every trap of the
   probe; a `memory.size` variant fails 10/10 in release; it passed in debug only
   because extra calls gave V8 more reload points).
3. **Make growth rare.** The first thread to see a growth grows 16 MiB ahead
   (malloc + free through dlmalloc), so the heap grows in a few large steps
   instead of 64 KiB-2 MiB ones. LLVM removes a `malloc` + `free` pair whose block
   is never used, so the pointer goes through `core::hint::black_box`. Without it
   the release-wasi build had no call in `refresh` and never grew ahead; the first
   numbers here (3015 -> 14 per CurrentThread run, 11600 -> 60 MultiThread) came
   from a debug build. Release-wasi, 3 runs each, sbrk `memory.grow` calls and
   `refresh()` calls per load, without -> with `black_box`:

   | load                                          | grows              | refreshes            |
   | --------------------------------------------- | ------------------ | -------------------- |
   | MultiThread w4 16 builds                      | 2846-2848 -> 24    | 11277-11315 -> 52-59 |
   | MultiThread w4 16 builds + JS plugin x2 waves | 3111-3131 -> 26-27 | 11719-12304 -> 65-72 |
   | MultiThread w4 parse 16x3                     | 2824-2830 -> 25    | 13065-13090 -> 58-69 |
   | default MultiThread w2 16 builds              | 2826-2830 -> 24    | 5624-5657 -> 29-31   |
   | CurrentThread parse 16x3                      | 2820-2824 -> 25    | 12995-13271 -> 58-62 |

   Each grow also makes V8 set page permissions over the whole memory
   (`GrowWasmMemoryInPlace`, src/objects/backing-store.cc:534) and interrupts every
   other thread, so fewer grows is also much faster (see Workaround results).

4. **Threaded build only, no behavior change elsewhere.** The single-thread build
   has one thread and never sees a stale size; native builds keep mimalloc.

## Rejected alternatives

- **Pre-grow the heap** (bigger loader `initial`, or malloc + free a large block at
  startup). A bigger `initial` does nothing: dlmalloc still grows from the top of
  memory (1 GiB, 1.5 GiB, 1.83 GiB initial all trap 10/10). Pre-growing through
  dlmalloc (512 MiB) passes 45/45, but only until a load outgrows it, and heap
  addresses at or above 2^31 break node:wasi calls (os error 28), so the headroom
  cannot simply be raised.
- **`memory.size` as the refresh.** Does not reload V8's bounds (principle 2).
- **Refresh on the emnapi / JS side.** The traps are inside wasm (Rust and C
  memset / memcpy), in activations that never return to JS, so a JS-side refresh
  never runs on the trapping thread. emnapi's JS views have the same stale-size
  hazard, but `@emnapi/wasi-threads` already refreshes those with `memory.grow(0)`, and in
  rolldown runs the wasm trap always came first.
- **`--liftoff-only`.** Makes the trap rare but not impossible (1/20 and 1/5 in
  the probes, see Evidence), and removes the MultiThread speedup (release, 16
  builds x2 waves: 1107 ms vs 664 ms pre-grown).
- **An extra lock around the allocator.** Passed the repro (25/25 debug, 10/10
  release), but dlmalloc is already locked, so it cannot be fixing a race; the extra
  calls only add points where V8 happens to reload the size. It leaves the
  mechanism in place and serializes every allocation.
- **Refresh before the allocation** (in every wrapper and `HeapSyncAlloc::alloc`,
  run `memory.grow(0)` first when `MAX_SEEN_PAGES > LOCAL_PAGES`), to cover
  dlmalloc's header stores on hosts without the trap handler. With
  `--wasm-enforce-bounds-checks` on the release-wasi artifact it fails 110/110
  like the current code (61 hang, 49 trap, against 76 / 34), and every trap is
  still the same dlmalloc chunk-header store. The race stays open: a thread
  waiting on the dlmalloc lock takes it right after the growing thread unlocks
  and before that thread publishes the new size, so its check still sees no
  change. Hot-path cost was within 1% (5 loads, 10 runs each). Closing it needs
  a refresh after the lock is taken inside dlmalloc (a dlmalloc or `sbrk` hook,
  or a replacement allocator that owns the lock), not in the wrappers.

## Remaining gaps

- A block that reaches a running thread mid-poll (a channel message, an `Arc`,
  a threadsafe-function call on the JS thread) and is filled / copied / used with
  atomics there before that thread's next allocation or poll boundary can still
  hit its stale size. Task migration and blocking-closure entry are covered by
  the handoff refresh (principle 1); after any allocation or poll start, the
  thread covers every block allocated before it. With rare growth this was not
  observed in the measured runs. The V8 fix closes it.
- C code that calls `malloc` directly and then fills the block is not covered:
  `malloc` cannot be wrapped (see [implementation.md](./implementation.md)).
- Hosts without the V8 wasm trap handler bounds-check plain stores against the
  cached size too. Measured with Node's `--wasm-enforce-bounds-checks` (hosts
  without the handler should behave the same, not measured): dlmalloc's own
  chunk-header store into pages another thread grew traps inside dlmalloc
  (reached through `posix_memalign` or `malloc`), before the refresh and while
  dlmalloc holds its lock. When the trap does not end the process, the lock is
  never released and the process **hangs**: the main thread and the other
  workers spin on `sched_yield` at about 400% CPU. The loader's worker-crash
  latch cannot help, because it runs on the main thread's event loop, which
  never gets control back. `free` and the unwrapped `malloc` that emnapi calls
  from JS write chunk headers too.

  Direct MultiThread bundle runs, 4 workers, node 24.21 arm64:

  | Artifact     | `--wasm-enforce-bounds-checks` | Result                                |
  | ------------ | ------------------------------ | ------------------------------------- |
  | release-wasi | yes                            | 0/110 pass (76 hang, 34 trap)         |
  | release-wasi | no                             | 10/10 pass                            |
  | dev profile  | yes                            | 103/110 pass (1/10 fail in a control) |
  | dev profile  | no                             | 30/30 pass                            |

  A bigger loader `initial` (2 GiB, 3 GiB) does not help with the flag: 0/20
  pass. A refresh before the allocation does not either (see Rejected
  alternatives). Keeping the grow-ahead alive (principle 3) makes it rarer, not
  gone: MultiThread w4 16 builds with the flag went from 0/10 to 10/10 pass (10
  interleaved runs each), and from 0/13 to 10/13 and 0/8 to 5/8 in two earlier
  sets. The table above was
  measured while the release grow-ahead was dead code.

  Which hosts lack the handler: Node's bundled
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

- Not measured with rolldown: browsers (wasi-browser loader), Node on x64 (the
  pure V8 repro does trap on Node 25 x64).

## When to remove

When every Node version the threaded WASI package supports ships
v8/v8@34241014663390c72e08c123faef6fedf395be8e (or a backport of it), delete
`crates/rolldown_binding/src/wasm_heap_sync.rs`, its `#[global_allocator]` in
`lib.rs`, the `--wrap` link args in `build.rs`, the handoff hook
(`crates/rolldown_utils/src/thread_handoff.rs`, its re-exports in
`rolldown_utils/src/lib.rs`, and its registration and `spawn_blocking` wrap in
`rolldown_binding/src/async_runtime.rs`), the export check in
`scripts/wasi/check-wasi-dist-files.mjs`, `scripts/wasi/check-v8-shared-memory-grow.*`,
the `check:v8-shared-memory-grow` script in the root `package.json`, and this
folder. Confirm first on each of those Node versions with
`pnpm check:v8-shared-memory-grow` and `pnpm check:v8-shared-memory-grow --activation=warm`:
remove the workaround only when case 1 stops trapping in both (verdict `ABSENT`).

## Related

- [implementation.md](./implementation.md) — the machinery that realizes this
- `../async-runtime/wasi-flavor-design.md` — the two WASI flavors and their loaders
