# WASI shared memory grow — Implementation

> The rationale and principles behind this live in [design.md](./design.md).

## Summary

The workaround now lives in napi-rs (napi-rs/napi-rs#3552, released as napi 3.14.0 /
napi-sys 3.4.0 / napi-build 2.6.0 / napi-async-runtime 0.2.4, which the root
`Cargo.toml` pins; the loaders still come from a vendored `@napi-rs/cli` tarball of
the same source until `@napi-rs/cli` 3.10.6 is installable). On `wasm32-wasip1-threads`
only, napi takes one spin lock around every call into wasi-libc's dlmalloc and
refreshes the thread's view of the memory size (`memory.grow(0)`) under it when
another thread has seen a larger memory; its `sbrk` hands out the pages between the
module's own initial memory and the loader's memory before it grows, and publishes
each growth before the lock is released. napi-build adds the `--wrap` link args and
a `malloc` / `free` export shim. napi-async-runtime and napi's own cross-thread
entries refresh at every scheduler handoff. Rolldown no longer has any allocator,
link-arg, export-rename or handoff-hook code of its own: its `#[global_allocator]`
on WASI stays std's `System`, which ends in wasi-libc's `malloc` and so goes
through napi's lock. What rolldown keeps is the checks: the dist check, the stress
harness and its CI steps, and the worker-crash test.

## Where the pieces live

```
napi-rs (napi-rs/napi-rs#3552)                           rolldown
────────────────────────────────                         ────────────────────────────────
crates/build/src/wasi.rs          --wrap args  ─┐        Cargo.toml [patch.crates-io]
crates/build/src/wasi_heap_sync_exports.{c,o}   ├──────> crates/rolldown_binding/build.rs
                                  malloc/free   │          setup() (nothing WASI-specific)
                                  export shim  ─┘
crates/napi/src/wasi_heap_sync.rs   lock, __wrap_*, __wrap_sbrk, napi_wasm_heap_sync_stat
crates/napi/src/wasi_heap_break.rs  sbrk page math (unit-tested natively)
crates/sys/src/wasi_heap_sync.rs    MAX_SEEN_PAGES, refresh_if_behind (one copy per graph)
crates/sys/src/wasi_thread_crash.rs addon crash flag word
crates/async-runtime/src/async_runtime.rs   on_thread_handoff at polls / blocking starts
crates/napi/src/{async_work,tokio_runtime}.rs  napi's own cross-thread entries
cli/src/api/templates/*             crash-flag bridge ────> packages/rolldown/src/
                                                           rolldown-binding.wasi.cjs,
                                                           wasi-worker.mjs (generated)
cli/docs/wasi.md                    "Shared memory growth on wasm32-wasip1-threads",
                                    "Shutdown polls never wait"
```

| piece                       | where                                                                                                                                              |
| --------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| `--wrap` link args          | napi-build `crates/build/src/wasi.rs` (feature `wasi-heap-sync`, which napi turns on for its build-dependency; `napi_build::setup()` emits them)   |
| `malloc` / `free` exports   | napi-build `crates/build/src/wasi_heap_sync_exports.c`, linked as the committed `.o`: `malloc` / `free` forward to `__wrap_malloc` / `__wrap_free` |
| lock, wrappers, break       | napi `crates/napi/src/wasi_heap_sync.rs` (`__wrap_*`, `__wrap_sbrk`, `napi_wasm_heap_sync_stat`), page math in `wasi_heap_break.rs`                |
| shared memory-size state    | napi-sys `crates/sys/src/wasi_heap_sync.rs` (`MAX_SEEN_PAGES`, `refresh_if_behind`)                                                                |
| scheduler handoff refresh   | napi-async-runtime `crates/async-runtime/src/async_runtime.rs` (`on_thread_handoff`), napi `async_work.rs` / `tokio_runtime.rs`                    |
| crash flag in linear memory | napi-sys `crates/sys/src/wasi_thread_crash.rs`, exported by napi as `napi_wasm_thread_crash_flag_address` / `napi_wasm_thread_crashed`             |
| opt-out                     | `--cfg napi_wasi_no_heap_sync` in an addon's target rustflags (rolldown does not set it)                                                           |
| version pin                 | root `Cargo.toml` `[workspace.dependencies]`: napi 3.14.0, napi-build 2.6.0, napi-derive 3.6.10 (napi-sys 3.4.0, napi-async-runtime 0.2.4)         |
| dist check                  | `scripts/wasi/check-wasi-dist-files.mjs` with `scripts/wasi/wasm-sections.mjs` (Checks below)                                                      |
| stress harness              | `packages/rolldown/tests/wasi/threaded-memory-stress.mjs` and its scripts in `packages/rolldown/tests/package.json`                                |
| worker-crash latch test     | `packages/rolldown/tests/wasi/worker-crash-latch.mjs` (+ `crash-injector-{preload,worker}.mjs`)                                                    |
| generated-loader anchors    | `packages/rolldown/binding-loader-codegen.ts` (`WASI_THREAD_CRASH_LATCH_*_SIGNATURES`, `assertWasiThreadCrashLatch`)                               |
| V8 bug probe                | `scripts/wasi/check-v8-shared-memory-grow.mjs` (`pnpm check:v8-shared-memory-grow`)                                                                |

## One napi-sys in the graph

`MAX_SEEN_PAGES` is a static in napi-sys. If the graph held two napi-sys copies
(say a git one for rolldown's napi and a crates.io one for an `oxc_*_napi` crate),
the allocator would publish growth into one copy and the handoff refresh would
read the other, which never moves. So all six napi crates (napi, napi-sys,
napi-build, napi-derive, napi-derive-backend, napi-async-runtime) come from
crates.io releases that every `oxc_*_napi` crate accepts too. Check after any
change to the napi pins:

```
cargo tree --target wasm32-wasip1-threads -i napi-sys      # exactly one, from the registry
cargo tree --target wasm32-wasip1-threads -d --depth 0     # no napi* line
```

A second napi-build copy (one without the `wasi-heap-sync` feature) also fails the
link, with `undefined symbol: napi_wasi_heap_sync_needs_napi_build_setup_with_wasi_heap_sync`.

## Checks

- `node scripts/wasi/check-wasi-dist-files.mjs threaded` (CI `reusable-wasi.yml`,
  after the debug and the release threaded builds) fails unless the threaded wasm
  exports `__wrap_malloc`, `__wrap_free`, `__wrap_calloc`, `__wrap_realloc`,
  `__wrap_posix_memalign`, `__wrap_sbrk` and `napi_wasm_heap_sync_stat` as
  functions; the bodies of the `malloc` and `free` exports are one-instruction
  forwarders (`local.get 0; call X`) whose `X` is the function exported as
  `__wrap_malloc` / `__wrap_free` (`readForwardTarget` in `wasm-sections.mjs`); and
  no import is named `__real_*` or `__wrap_*` (an unresolved `--wrap` symbol that
  `--import-undefined` turned into an import). A wrong `malloc` export is silent at
  runtime (the module loads and runs without the lock), which is why the check
  reads the call target, not the name.
- `node scripts/wasi/check-wasi-dist-files.mjs single [packages/browser/dist]` fails
  if the single-thread wasm exports any `__wrap_*` or `napi_wasm_heap_sync_*`.
- `packages/rolldown/tests/wasi/threaded-memory-stress.mjs` runs each case in a
  child process with a 60 s timeout, and every child checks
  `napi_wasm_heap_sync_stat(2)` (blocks past the allocating thread's refreshed
  size) is 0 and prints the counters (0 grows, 1 lock refreshes, 2 late
  refreshes, 3 break pages, 4 `__heap_end` pages, 5 handoff refreshes). Its
  options: `--cases`, `--runs`, `--node-flag` (Node refuses
  `--wasm-enforce-bounds-checks` / `--disable-wasm-trap-handler` in `NODE_OPTIONS`,
  so the parent puts them on the child's command line), `--initial-pages N|min`
  (the child wraps `WebAssembly.Memory` before the loader runs, so the loader's
  shared memory starts at N pages or at the module's declared minimum; the loader
  is not changed), `--hold-mib N` (the child mallocs N MiB through the module's
  `malloc` export and keeps it) and `--expect-grows zero|some`. The child captures
  the module's exports by wrapping `WebAssembly.Instance`.
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
- The steps above use the debug-profile wasm (`just build-rolldown-wasi`). The
  allocator code is profile-sensitive (in `release-wasi` LLVM once removed an
  unused grow-ahead `malloc` + `free` pair that the debug build kept), so the
  `release-threaded` job in `reusable-wasi.yml` builds the wasm with
  `build-wasi:release` (the release workflow's `build-binding:wasi:release` with
  `TARGET_CC=clang`, after the native binding that bundles the dist) and runs the
  dist check, `test:wasi-threaded`, every script in the table except
  `test:wasi-threaded-stress`, and `test:wasi-worker-crash` against it. The dist
  holds only the stripped `.wasm`; the job removes `src/*.debug.wasm`, fails if the
  dist has one (the loader would pick it), checks the dist wasm equals the `src`
  one, and prints its sha256 to compare with the release run's
  `bindings-wasm32-wasip1-threads` artifact.
- `test:wasi-worker-crash` (`worker-crash-latch.mjs`): its in-flight case arms the
  crash right before the disposal's first poll turn, without waiting for the
  loader's crash flag, so a worker can die while that poll is inside wasm. It
  passes only because napi's work-pending poll never waits for a lock and the
  dying worker raises the addon's crash flag through the view the loader hands it.

## Related

- [design.md](./design.md) — the V8 bug, evidence, rejected alternatives, and when to remove this
- `../async-runtime/implementation.md` — WASI artifact naming and the loaders
- napi-rs `cli/docs/wasi.md` — the upstream description, cost numbers and known limits
