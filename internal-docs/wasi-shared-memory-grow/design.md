# WASI shared memory grow — Design & Principles

## Summary

The threaded WASI binding (`wasm32-wasip1-threads`) traps with "memory access out
of bounds" under concurrent load because of a V8 bug: a thread that did not run
`memory.grow` keeps a stale memory size in optimized code, and `memory.fill`,
`memory.copy` and atomics are bounds-checked against it. Rolldown works around it
in its own allocator: after an allocation that may sit in pages this thread has not
seen, the thread runs `memory.grow(0)`, which makes V8 reload the size. The
workaround lives only in the threaded build and goes away once the Node versions we
support ship the V8 fix. For the machinery, see
[implementation.md](./implementation.md). Addresses #10697.

## The bug

```
thread A (out of heap)            thread B (optimized wasm, long activation)
  dlmalloc -> sbrk
  memory.grow(n)  ── V8 refreshes A's cached size only
  hands out / frees blocks          malloc() -> block in the new pages
                                    memset / memcpy on it
                                      memory.fill / memory.copy
                                      bounds check vs B's OLD size -> trap
```

- wasi-libc's dlmalloc grows the one shared memory from whichever thread runs out
  of heap (`sbrk` is the only `memory.grow` site in the module).
- The stale size is per activation. A JS -> wasm entry refreshes it, so a thread
  that stays in wasm (rayon workers, emnapi async work threads, dlmalloc's spin
  lock) keeps the old size.
- Plain loads and stores do not trap: with the wasm trap handler (default Node on
  arm64 and x64) they are checked by guard pages against the real size.
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
- `--liftoff-only` (no optimized code): 0/900 pure-V8 traps, rolldown MultiThread
  10/10 pass.
- V8 fixed exactly this upstream: "[wasm] Atomic memory.size and dynamic bounds
  checks for shared memory", v8/v8@34241014663390c72e08c123faef6fedf395be8e
  (crrev 8466625, V8 bugs 529880019 / 533026477, 2026-09-29). Not in any Node
  release as of 2026-09-30.
- #10697 first read this as heap metadata corruption. Its V8 control used a plain
  store, which passes here too; `fill` / `copy` / atomics trap, a pre-grown heap
  cures it, and an allocator race would not be cured by pre-growing.

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

The MultiThread rows lift the WASI MultiThread guard in a local build only; the
shipped WASI binding runs CurrentThread, which the CurrentThread rows cover. The
"landing tree" rows ran on the commit that adds this workaround.

Timing is at noise level (CurrentThread release median 729 vs 723.5 ms;
MultiThread vs a pre-grown base 479.5 vs 480 ms).

## Design principles

1. **Refresh at the allocator, where the new pages enter a thread.** The trapping
   operation is the first memset / memcpy into a block that malloc just returned.
   Refreshing right after the allocation, before that first touch, closes the window
   for every block the thread allocates itself.
2. **`memory.grow(0)`, not `memory.size`.** Only a grow makes V8 reload the cached
   bounds of the running activation; `memory.size` returns the new size and leaves
   the bounds alone (a `memory.size` variant fails 10/10 in release; it passed in
   debug only because extra calls gave V8 more reload points).
3. **Make growth rare.** The first thread to see a growth grows 16 MiB ahead
   (malloc + free through dlmalloc), so the heap grows in a few large steps. Growth
   events, and so refreshes on every thread, drop from about 3000 to about 14 per
   CurrentThread run (11600 to 60 for MultiThread).
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
  memset / memcpy), in activations that never return to JS; a JS -> wasm entry
  already refreshes. emnapi's JS views have the same stale-size hazard, but
  `@emnapi/wasi-threads` already refreshes those with `memory.grow(0)`, and in
  rolldown runs the wasm trap always came first.
- **`--liftoff-only`.** Removes the trap but also the MultiThread speedup
  (release, 16 builds x2 waves: 1107 ms vs 664 ms pre-grown).
- **An extra lock around the allocator.** Passed the repro (25/25 debug, 10/10
  release), but dlmalloc is already locked, so it cannot be fixing a race; the extra
  calls only add points where V8 happens to reload the size. It leaves the
  mechanism in place and serializes every allocation.

## Remaining gaps

- A block allocated by thread A, handed to thread B, and filled / copied / used
  with atomics by B before B's own next allocation can still hit B's stale size.
  After any allocation through the wrappers, B covers every block allocated
  before it, so the window is only "no allocation on B in between"; with rare
  growth it was not observed in the measured runs.
- C code that calls `malloc` directly and then fills the block is not covered:
  `malloc` cannot be wrapped (see [implementation.md](./implementation.md)).
- Hosts without the wasm trap handler (`--wasm-enforce-bounds-checks`, some
  32-bit hosts) bounds-check plain stores against the cached size too, so
  dlmalloc's own header writes can trap before the refresh (1/6 with the flag).
- Not measured with rolldown: browsers (wasi-browser loader), Node on x64 (the
  pure V8 repro does trap on Node 25 x64).

## When to remove

When every Node version the threaded WASI package supports ships
v8/v8@34241014663390c72e08c123faef6fedf395be8e (or a backport of it), delete
`crates/rolldown_binding/src/wasm_heap_sync.rs`, its `#[global_allocator]` in
`lib.rs`, the `--wrap` link args in `build.rs`, the export check in
`scripts/wasi/check-wasi-dist-files.mjs`, and this folder. Confirm first with the
pure V8 repro (fill / copy / atomic on a page another thread grew).

## Related

- [implementation.md](./implementation.md) — the machinery that realizes this
- `../async-runtime/wasi-flavor-design.md` — the two WASI flavors and their loaders
