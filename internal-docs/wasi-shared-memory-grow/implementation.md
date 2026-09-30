# WASI shared memory grow — Implementation

> The rationale and principles behind this live in [design.md](./design.md).

## Summary

On `wasm32-wasip1-threads` only, every call into wasi-libc's dlmalloc runs under
one Rust spin lock, and the thread refreshes its view of the memory size
(`memory.grow(0)`) right after it takes the lock when another thread has seen a
larger memory. `sbrk` is wrapped too: it hands dlmalloc the pages between the
module's own initial memory and the loader's memory before it grows, and when it
grows it publishes the new size before the lock is released. Rust allocations
reach the lock as the `#[global_allocator]`, C allocations (emnapi, wasi-libc)
through `--wrap` link args, and JS (`@emnapi/core`) through the `malloc` / `free`
exports, which a post-link step points at the wrappers. Every task poll and
blocking closure start also refreshes, through a hook in
`rolldown_utils::async_runtime`. A dist check keeps all of this in the threaded
wasm and out of the single-thread one.

## Components

| piece                       | file                                                      | role                                                                                                                                                                                          |
| --------------------------- | --------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| cfg `rolldown_wasi_threads` | `crates/rolldown_binding/build.rs`                        | set when cargo `TARGET` is `wasm32-wasip1-threads` (the two WASI targets have identical `rustc --print cfg` sets); declared with `cargo::rustc-check-cfg`                                     |
| `--wrap` link args          | `crates/rolldown_binding/build.rs`                        | same branch as the cfg; every dlmalloc entry and `sbrk` (table below)                                                                                                                         |
| global allocator            | `crates/rolldown_binding/src/lib.rs`                      | `HeapSyncAlloc` under `cfg(all(target_family = "wasm", rolldown_wasi_threads))`                                                                                                               |
| lock, wrappers, break       | `crates/rolldown_binding/src/wasm_heap_sync.rs`           | `locked`, `__wrap_*`, `__wrap_sbrk`, `HeapSyncAlloc`, `rolldown_heap_sync_malloc` / `_free` (JS entries), `rolldown_heap_sync_stat` (test counters)                                           |
| handoff refresh             | `crates/rolldown_binding/src/wasm_heap_sync.rs`           | `refresh_if_behind`: `memory.grow(0)` when `MAX_SEEN_PAGES > LOCAL_PAGES`, outside the lock                                                                                                   |
| handoff hook                | `crates/rolldown_utils/src/thread_handoff.rs`             | wasm only: `set_thread_handoff_hook`, `on_thread_handoff`, `HandoffHook<F>`, and hooked `spawn` / `try_spawn` / `(try_)spawn_detached` / `spawn_blocking` / `block_on` / `(try_)block_on_dyn` |
| hook re-export              | `crates/rolldown_utils/src/lib.rs`                        | on wasm, `async_runtime` re-exports the hooked names over the `napi_async_runtime::*` glob; native keeps the plain glob                                                                       |
| hook registration           | `crates/rolldown_binding/src/async_runtime.rs`            | `install_async_runtime_backend` (module init) registers `refresh_if_behind`; the adapter's `spawn_blocking` boxes its closure with the hook                                                   |
| export rename               | `scripts/wasi/rename-wasm-allocator-exports.mjs`          | after the link: drop `__wrap_malloc` / `__wrap_free`, add `malloc` / `free` for the functions exported as `rolldown_heap_sync_malloc` / `_free`; refuses to run twice                         |
| rename call                 | `packages/rolldown/build-binding.ts`                      | `renameWasiAllocatorExports()` right after the napi build of `wasm32-wasip1-threads`, on `src/rolldown-binding.wasm32-wasi{,.debug}.wasm`                                                     |
| wasm section reader         | `scripts/wasi/wasm-sections.mjs`                          | dependency-free export-section read / write and memory-import read, shared by the rename, the dist check and the stress test                                                                  |
| artifact check              | `scripts/wasi/check-wasi-dist-files.mjs`                  | threaded wasm: the wrappers exist, and `malloc` / `free` share a function index with `rolldown_heap_sync_malloc` / `_free`; single-thread wasm: no `__wrap_*` / `rolldown_heap_sync_*` export |
| stress and flag tests       | `packages/rolldown/tests/wasi/threaded-memory-stress.mjs` | loads under both flavors, with options for node flags, a smaller loader memory, a held block and a grow-count expectation (Checks below)                                                      |

## Data flow

```
C (emnapi, wasi-libc) ──> __wrap_malloc / free / calloc / realloc / ... ─┐
Rust ──> HeapSyncAlloc (malloc | calloc | realloc | posix_memalign) ─────┤
JS (@emnapi/core) ──> export malloc / free = rolldown_heap_sync_* ───────┤
                                                                         v
locked(f):  LOCK (CAS; spin, sched_yield every 64 spins; no atomic.wait)
            LOCAL_PAGES == 0 || MAX_SEEN_PAGES > LOCAL_PAGES ? memory.grow(0)
            f = __real_xxx(...)            dlmalloc: its own lock around chunk
              └─ sbrk ──> __wrap_sbrk        headers; calloc's memset, realloc's
                                             memcpy after that lock, still in LOCK
            after(ptr, size): block end > LOCAL_PAGES? count + refresh (never seen)
            UNLOCK (Release)

__wrap_sbrk(n)  (only called by dlmalloc, so always under LOCK):
  BRK == 0 ? BRK = __heap_end (end of the module's own initial memory)
  n == 0 ? return BRK
  memory.grow(0) -> current pages          refresh this thread
  BRK + n > current ? memory.grow(max(need, 16 MiB)); memory.grow(0)
                     (LOCAL_PAGES = MAX_SEEN_PAGES = new size, before UNLOCK)
  BRK += n; return old BRK
```

- `LOCAL_PAGES` (thread local): the size this thread's V8 bounds were last reloaded
  to. Only ever set from `memory.grow(0)` on this thread, so it never exceeds what
  V8 checks here.
- `MAX_SEEN_PAGES` (global atomic): the largest `LOCAL_PAGES` any thread stored.
  `__wrap_sbrk` raises it before the lock is released, so the next holder's
  Acquire load sees every growth.
- `BRK` (global atomic, `Relaxed`): dlmalloc's break. Only `__wrap_sbrk` reads or
  writes it, always under the lock.
- The lock is not re-entrant, and it does not need to be: dlmalloc's only calls out
  of its object are `sbrk` (our hook, which does not lock) and `sched_yield`
  (relocations of wasi-libc's `dlmalloc.c.obj`; the linked module agrees: the real
  `malloc` / `free` / `calloc` / `realloc` / `posix_memalign` / `aligned_alloc`
  reach only `dlmalloc`, `dlfree`, `dispose_chunk`, `internal_memalign`,
  `prepend_alloc`, `__wrap_sbrk` and `sched_yield`). Inside the object the public
  names are thin wrappers over static `dl*` functions, so `realloc`'s internal
  `malloc` is not redirected by `--wrap`.
- `LOCK` is held longer than dlmalloc's own lock: the real `calloc` runs
  `memory.fill` after `dlmalloc` returns (and has released its lock), and the
  real `realloc`'s move path runs `memory.copy` between its `dlmalloc` and
  `dlfree` calls. Both still run inside `locked`, so a large zeroed allocation or
  realloc copy holds every other thread's allocator calls. Kept on purpose; see
  [design.md](./design.md), principle 1 (trade-off).
- `memory.grow(n > 0)` exists in exactly one place, `__wrap_sbrk`; wasi-libc's own
  `sbrk.c.obj` is no longer linked. `memory.grow(0)` runs in `grow_zero` (the lock,
  the hook, the handoff refresh).
- `HeapSyncAlloc` picks the dlmalloc entry the way std's `System` does on wasm32
  (malloc / calloc / realloc when the alignment is at most 8 and at most the size,
  posix_memalign otherwise) and calls the `__real_*` symbol under the lock, so Rust
  never goes through a `__wrap_*` function. Its large-alignment `alloc_zeroed` and
  `realloc` zero or copy after the lock: the blocks lie within this thread's
  refreshed size.
- The page math: pages and byte addresses fit a 32-bit `usize` everywhere except a
  block that ends exactly at 4 GiB, so `after` compares `u64`s. `const` asserts pin
  the edges; they run when the threaded target compiles. There is no native unit
  test because the module only compiles for that target.

Scheduler handoff (threaded WASI; the hook is unset on threadless wasm, so each
call there is one load of an unset `OnceLock`; native does not compile it):

```
spawn / try_spawn / spawn_detached / block_on ──> HandoffHook(future)
   every poll ──> on_thread_handoff ──> refresh_if_behind ──> inner.poll
spawn_blocking (facade) / adapter spawn_blocking ──> closure:
   on_thread_handoff ──> refresh_if_behind ──> body
refresh_if_behind: MAX_SEEN_PAGES (Acquire) > LOCAL_PAGES? ──> memory.grow(0)
```

- `try_spawn_blocking` in `rolldown_utils` is not hooked: its rejection returns the
  caller's closure, which a wrapper cannot give back. Its one in-tree caller, the
  napi adapter, wraps its `Box<dyn FnOnce>` itself.

## The `--wrap` list

`nm` on the rustc 1.98.1 sysroot's self-contained `libc.a` for
`wasm32-wasip1-threads`, and on emnapi's `libemnapi-napi-rs-mt.a`:

| symbol               | defined in                  | referenced by                                                      |
| -------------------- | --------------------------- | ------------------------------------------------------------------ |
| `malloc`             | `dlmalloc.c.obj`            | libc (stdio, dirent, `pthread_create`, ...), emnapi                |
| `free`               | `dlmalloc.c.obj`            | libc, emnapi                                                       |
| `calloc`             | `dlmalloc.c.obj`            | libc (environ, preopens, regex), emnapi (`napi_create_async_work`) |
| `realloc`            | `dlmalloc.c.obj`            | libc (`getdelim`, `glob`, `reallocarray`, ...), emnapi             |
| `posix_memalign`     | `dlmalloc.c.obj`            | std's `System` only                                                |
| `aligned_alloc`      | `dlmalloc.c.obj`            | no caller today                                                    |
| `malloc_usable_size` | `dlmalloc.c.obj`            | no caller today (reads a chunk header, so it locks too)            |
| `__libc_malloc`      | alias of `malloc`           | libc locale (`duplocale`, `newlocale`)                             |
| `__libc_free`        | alias of `free`             | libc locale (`freelocale`)                                         |
| `__libc_calloc`      | alias of `calloc`           | libc `atexit`                                                      |
| `sbrk`               | `sbrk.c.obj` (now unlinked) | `dlmalloc.c.obj` only                                              |

`--wrap` rewrites references to the symbol from every object; calls that
dlmalloc makes inside its own object go to its static `dl*` functions, which are
not symbols `--wrap` can see. Each wrapper calls the matching `__real_*`.

## The `malloc` / `free` exports

napi-build links with `--export=malloc --export=free`, and `@emnapi/core` requires
`exports.malloc` / `exports.free` (`dist/emnapi-core.js:325-330` in
2.0.0-alpha.5: "malloc is not exported"). Under `--wrap=malloc` the linker emits
that export as `__wrap_malloc` and no `malloc` at all (measured). rust-lld has no
export-rename or defsym option, so after the link:

```
before:  __wrap_malloc -> f1   rolldown_heap_sync_malloc -> f2   (no malloc)
after:                         rolldown_heap_sync_malloc -> f2   malloc -> f2
```

`scripts/wasi/rename-wasm-allocator-exports.mjs` rewrites only the export section
(every other byte is kept), validates the result, and replaces the file. It
refuses a module that has no `rolldown_heap_sync_malloc` export or whose `malloc`
already points at it. `packages/rolldown/build-binding.ts` runs it after every
threaded napi build, inside the build-artifact transaction, on the `.wasm` and
the `.debug.wasm`; the napi cli copies a fresh wasm from the target dir on every
build, so a warm rebuild renames again. Every workflow that builds the threaded
artifact goes through `build-binding.ts`: `reusable-wasi.yml`
(`just build-rolldown-wasi`), `reusable-release-build.yml`
(`build-binding:wasi:release`) and `reusable-browser.yml`
(`packages/browser-tests/scripts/prepare-fixture.mjs`). A path that skips it
fails closed: the module does not load, and the dist check fails.

## Checks

- `node scripts/wasi/check-wasi-dist-files.mjs threaded` (CI `reusable-wasi.yml`,
  after the threaded build) fails unless the threaded wasm exports the wrappers
  (`__wrap_calloc`, `__wrap_realloc`, `__wrap_posix_memalign`, `__wrap_sbrk`,
  `rolldown_heap_sync_malloc` / `_free` / `_stat`), `malloc` and `free` have the
  function index of `rolldown_heap_sync_malloc` / `_free`, and `__wrap_malloc` /
  `__wrap_free` are gone. The un-renamed wasm has no `malloc` export, so it fails.
- `node scripts/wasi/check-wasi-dist-files.mjs single [packages/browser/dist]` fails
  if the single-thread wasm carries any `__wrap_*` or `rolldown_heap_sync_*` export.
- `packages/rolldown/tests/wasi/threaded-memory-stress.mjs` runs each case in a
  child process with a 60 s timeout, and every child checks
  `rolldown_heap_sync_stat(2)` (blocks past the thread's refreshed size) is 0 and
  prints the counters. Its options: `--cases`, `--runs`, `--node-flag` (Node
  refuses `--wasm-enforce-bounds-checks` / `--disable-wasm-trap-handler` in
  `NODE_OPTIONS`, so the parent puts them on the child's command line),
  `--initial-pages N|min` (the child wraps `WebAssembly.Memory` before the loader
  runs, so the loader's shared memory starts at N pages or at the module's declared
  minimum; the loader is not changed), `--hold-mib N` (the child mallocs N MiB
  through the module's `malloc` export and keeps it) and `--expect-grows zero|some`.
  The child captures the module's exports by wrapping `WebAssembly.Instance`.
- CI steps in `reusable-wasi.yml`, no retries, `timeout 300` each (scripts in
  `packages/rolldown/tests/package.json`):

  | script                                      | load                                                                                   |
  | ------------------------------------------- | -------------------------------------------------------------------------------------- |
  | `test:wasi-threaded-stress`                 | every default case once (both flavors)                                                 |
  | `test:wasi-threaded-stress:bounds-checks`   | MT w4 bundles + JS plugin, `--wasm-enforce-bounds-checks`, 20 runs                     |
  | `test:wasi-threaded-stress:no-trap-handler` | the same, `--disable-wasm-trap-handler`, 10 runs                                       |
  | `test:wasi-threaded-stress:forced-growth`   | MT w4 16 builds, loader memory = module minimum, bounds checks, expects grows, 20 runs |
  | `test:wasi-threaded-stress:handoff`         | MT w4 bundles + JS plugin, same memory and flag, expects grows, 20 runs                |
  | `test:wasi-threaded-stress:heap-ceiling`    | hold 1536 MiB, then MT w4 bundles                                                      |
  | `test:wasi-threaded-stress:no-growth`       | MT w4 16 builds and default MT bundles, expects 0 grows                                |

- CI also runs `test:wasi-threaded`, `test:stability` and
  `test:wasi-runtime-lifecycle` on the default MultiThread flavor, and again with
  `ROLLDOWN_RUNTIME=single` for CurrentThread.

## Related

- [design.md](./design.md) — the V8 bug, evidence, rejected alternatives, and when to remove this
- `../async-runtime/implementation.md` — WASI artifact naming and the loaders
