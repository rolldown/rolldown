# WASI shared memory grow — Implementation

> The rationale and principles behind this live in [design.md](./design.md).

## Summary

On `wasm32-wasip1-threads` only, every allocation goes through a thin layer over
wasi-libc's dlmalloc that runs `memory.grow(0)` on the current thread when the
returned block may lie in pages this thread's V8 bounds do not cover yet. Rust
allocations reach it as the `#[global_allocator]`; C allocations (emnapi, wasi-libc)
reach it through `--wrap` link args. Every task poll and blocking closure start runs
the same check against the largest size any thread has seen, through a hook in
`rolldown_utils::async_runtime`. A dist check keeps the wrappers in the threaded
wasm and out of the single-thread one.

## Components

| piece                       | file                                            | role                                                                                                                                                                                          |
| --------------------------- | ----------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| cfg `rolldown_wasi_threads` | `crates/rolldown_binding/build.rs`              | set when cargo `TARGET` is `wasm32-wasip1-threads` (the two WASI targets have identical `rustc --print cfg` sets); declared with `cargo::rustc-check-cfg`                                     |
| `--wrap` link args          | `crates/rolldown_binding/build.rs`              | same branch as the cfg; `--wrap=calloc,realloc,aligned_alloc,posix_memalign`                                                                                                                  |
| global allocator            | `crates/rolldown_binding/src/lib.rs`            | `HeapSyncAlloc` under `cfg(all(target_family = "wasm", rolldown_wasi_threads))`                                                                                                               |
| refresh logic + wrappers    | `crates/rolldown_binding/src/wasm_heap_sync.rs` | `needs_refresh`, `refresh`, `HeapSyncAlloc`, `__wrap_*`                                                                                                                                       |
| handoff refresh             | `crates/rolldown_binding/src/wasm_heap_sync.rs` | `refresh_if_behind`: `refresh()` when `MAX_SEEN_PAGES > LOCAL_PAGES`                                                                                                                          |
| handoff hook                | `crates/rolldown_utils/src/thread_handoff.rs`   | wasm only: `set_thread_handoff_hook`, `on_thread_handoff`, `HandoffHook<F>`, and hooked `spawn` / `try_spawn` / `(try_)spawn_detached` / `spawn_blocking` / `block_on` / `(try_)block_on_dyn` |
| hook re-export              | `crates/rolldown_utils/src/lib.rs`              | on wasm, `async_runtime` re-exports the hooked names over the `napi_async_runtime::*` glob; native keeps the plain glob                                                                       |
| hook registration           | `crates/rolldown_binding/src/async_runtime.rs`  | `install_async_runtime_backend` (module init) registers `refresh_if_behind`; the adapter's `spawn_blocking` boxes its closure with the hook                                                   |
| artifact check              | `scripts/wasi/check-wasi-dist-files.mjs`        | threaded wasm must export `__wrap_calloc` and `__wrap_realloc`; single-thread wasm must not                                                                                                   |

## Data flow

```
Rust alloc ──> HeapSyncAlloc.alloc ──> System.alloc ──> malloc (real)
                                                    └─> posix_memalign ──> __wrap_posix_memalign
C calloc   ──> __wrap_calloc  ──> malloc (real) ──> refresh_if_stale ──> memset
C realloc  ──> __wrap_realloc ──> refresh old / malloc (real) ──> refresh_if_stale ──> memcpy ──> free
                                                                        │
                           needs_refresh(end, LOCAL_PAGES, MAX_SEEN_PAGES)?
                                                                        │ yes
                                   memory.grow(0) ──> LOCAL_PAGES = size, MAX_SEEN_PAGES = max
                                   first to see growth? ──> malloc(16 MiB) + free, memory.grow(0) again
```

Scheduler handoff (threaded WASI; the hook is unset on threadless wasm, so each
call there is one load of an unset `OnceLock`; native does not compile it):

```
spawn / try_spawn / spawn_detached / block_on ──> HandoffHook(future)
   every poll ──> on_thread_handoff ──> refresh_if_behind ──> inner.poll
spawn_blocking (facade) / adapter spawn_blocking ──> closure:
   on_thread_handoff ──> refresh_if_behind ──> body
refresh_if_behind: MAX_SEEN_PAGES (Acquire) > LOCAL_PAGES? ──> refresh()
```

- `try_spawn_blocking` in `rolldown_utils` is not hooked: its rejection returns the
  caller's closure, which a wrapper cannot give back. Its one in-tree caller, the
  napi adapter, wraps its `Box<dyn FnOnce>` itself (a rejection returns the wrapped
  box, which runs the same work).
- Not covered: a block that arrives mid-poll (channel, `Arc`, a threadsafe-function
  call on the JS thread) and is filled or copied before the next allocation or poll
  boundary on that thread. See design.md, Remaining gaps.

- `LOCAL_PAGES` (thread local): the size this thread's V8 bounds were last reloaded
  to. Only ever set from `memory.grow(0)` on this thread, so it never exceeds what V8
  checks here.
- `MAX_SEEN_PAGES` (global atomic): the largest `LOCAL_PAGES` any thread stored. A
  thread refreshes when a block ends past `LOCAL_PAGES` or when another thread has
  seen more.
- calloc and realloc are replaced, not forwarded: libc's versions would memset or
  memcpy inside dlmalloc before any refresh. `alloc_zeroed` zeroes after
  `alloc`'s refresh for the same reason. `__wrap_realloc` also refreshes on the
  in-place path (old usable size >= new size), since the caller writes into the block
  next.
- The page math is `u64`: memory can reach 65536 pages (4 GiB), where
  `pages * 65536` and `ptr + size` overflow a 32-bit `usize`. `const` asserts in the
  module pin the edges; they run when the threaded target compiles. There is no
  native unit test because the module only compiles for that target.

## The `--wrap` list

| symbol                     | why                                                                                                                                                                                                                                  |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `calloc`                   | C code zeroes new blocks inside libc; emnapi's `napi_create_async_work` calloc is the path a Rust-only version left trapping (2/70)                                                                                                  |
| `realloc`                  | copies into a new block; std's `System.realloc` calls it for normal alignments                                                                                                                                                       |
| `posix_memalign`           | std's `System.alloc` uses it for large alignments, and `realloc_fallback` allocates through it before copying                                                                                                                        |
| `aligned_alloc`            | the C11 aligned entry point                                                                                                                                                                                                          |
| `malloc` — **not wrapped** | `@emnapi/core` requires the module's `malloc` export and `--wrap=malloc` removes it ("TypeError: malloc is not exported"); a bare malloc only writes dlmalloc chunk headers with plain stores, which V8 checks against the real size |

`--wrap` rewrites undefined references to the symbol; calls that wasi-libc's
dlmalloc resolves inside its own object are not rewritten. The wrappers call the
real entry points (`__real_*`, or `malloc` itself), so they never re-enter.

## Checks

- `node scripts/wasi/check-wasi-dist-files.mjs threaded` (CI `reusable-wasi.yml`,
  after the threaded build) fails if the threaded wasm lacks `__wrap_calloc` or
  `__wrap_realloc`.
- `node scripts/wasi/check-wasi-dist-files.mjs single [packages/browser/dist]` fails
  if the single-thread wasm carries them.
- `vp run --filter rolldown-tests test:wasi-threaded-stress`
  (`packages/rolldown/tests/wasi/threaded-memory-stress.mjs`, CI step "Threaded
  WASI memory stress", no retry) runs 16 concurrent builds, `parse()` and
  `transform()` calls under CurrentThread and MultiThread w4, each case in a child
  process with a 60 s timeout. Without the workaround it traps on most cases.
- CI also runs `test:wasi-threaded` and `test:stability` a second time with
  `ROLLDOWN_RUNTIME=multi ROLLDOWN_WORKER_THREADS=4`, the MultiThread opt-in that
  this workaround made safe to accept (see `../async-runtime/design.md`
  principle 1).

## Related

- [design.md](./design.md) — the V8 bug, evidence, rejected alternatives, and when to remove this
- `../async-runtime/implementation.md` — WASI artifact naming and the loaders
