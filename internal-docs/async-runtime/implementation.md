# Async Runtime — Implementation

> The rationale lives in [design.md](./design.md). Cite sections by title.

## Summary

The scheduler is not in this repo. The
[`napi-async-runtime`](https://crates.io/crates/napi-async-runtime) crate owns
the executor, blocking lane, timer heap, generations and CurrentThread host
registry. Rolldown selects, configures, consumes and bridges it.
`rolldown_utils::async_runtime` re-exports the crate.

```
  crates/rolldown_utils          thin facades  →  napi_async_runtime::*        (Rust core calls these)
        │
  crates/rolldown_binding        the napi backend adapter + host bridges       (JS ⇄ Rust boundary)
        │
  packages/rolldown/src/*.ts     capability gating                              (no runtime API)
```

## Backend adapter

`rolldown_binding/src/async_runtime.rs` vendors the crate adapter's
`AsyncRuntime` impl (as `RolldownAsyncRuntime`) and its CurrentThread task host,
and registers the runtime with napi's `AsyncRuntime` SPI in
`install_async_runtime_backend()` (`#[module_init]`, the one registration
point). The timer host is a record-only stub, not upstream's JS relay (see
"Timer host").

- Why vendored: the crate's adapter is behind its default `napi` feature, which
  also exports `configureAsyncRuntime` and the metrics API and defines the same
  host export names as this binding. So the dependency uses
  `default-features = false`, and the copy must track upstream `adapter.rs`.
- `begin_shutdown` / `shutdown_work_pending` / `finish_shutdown` are called only
  from napi's wasm cleanup exports, so a loader can turn the event loop between
  the phases. Native napi calls only `shutdown`.

## Rust core: facades, module loader, Rayon

- `rolldown_utils/src/time.rs` re-exports `sleep_until`; `futures.rs` re-exports
  the rest, keeping `spawn` / `try_spawn` as one-generic-parameter wrappers (plus
  `block_on_spawn_all` and `can_spawn_os_threads`). The crate itself
  refreshes the shared-memory size on threaded WASI
  ([napi-rs `wasi-heap-sync-design.md`](https://github.com/napi-rs/napi-rs/blob/main/cli/docs/wasi-heap-sync-design.md);
  rolldown side: "Threaded WASI heap sync").
- The module loader wraps each module future in `supervised_module_task` and
  submits it with `try_spawn_detached`. `ModuleTaskSupervisor::Drop` turns a
  panic, cancellation or rejected submission into exactly one diagnostic. The
  consumer loop uses an **unbounded** channel: a bounded one could deadlock the
  `block_on`-pinned JS thread.
- CPU work runs in the Rayon registry of the calling thread. The crate builds
  the executor's own Rayon pool (never `build_global`) and polls spawned futures
  there, so their `par_iter` work stays on that pool. A Rayon call from a thread
  outside it (the JS thread inside `block_on`, say) would use the global
  registry.

### Spawned tasks do not inherit the tracing subscriber

The crate has no `tracing` dependency and propagates no thread-local
dispatcher. A spawned future runs under the global default subscriber unless
the caller wraps it (`with_current_subscriber()`). See
[devtools/design.md](../devtools/design.md).

### Rejected `try_spawn` fails the start

`Watcher::run()` and `DevEngine::run()` submit their coordinator with
`try_spawn` and map a rejection to an error (`WatcherStartError` for the
watcher, a `BuildResult` error for the dev engine), dropping the coordinator.
A refused start is unreachable from JS: native stops the runtime only when the
last env is torn down, and on WASI a restart after dispose does nothing. The
convenience `spawn` / `spawn_blocking` instead destroy rejected work and return
an already-failed handle, which a caller would not notice.

## Configuration

- `RuntimeEnv::from_process()` is the only env read: `ROLLDOWN_RUNTIME`,
  `ROLLDOWN_WORKER_THREADS`, `ROLLDOWN_MAX_BLOCKING_THREADS`. The park
  deadline and the drainer linger stay at the crate defaults.
- `resolve_runtime_config_for(target, env)` is the pure table:

  | target        | default flavor | `ROLLDOWN_RUNTIME=single` | `=multi`      | `ROLLDOWN_WORKER_THREADS`                     |
  | ------------- | -------------- | ------------------------- | ------------- | --------------------------------------------- |
  | `Native`      | MultiThread    | MultiThread               | MultiThread   | default `min(physical, logical)`, at most 256 |
  | `WasiThreads` | MultiThread    | CurrentThread             | MultiThread   | default 2, clamped to [2, 4]                  |
  | `Wasi`        | CurrentThread  | CurrentThread             | CurrentThread | ignored                                       |

  MultiThread runs at least 2 workers, CurrentThread 1. The blocking cap is 1
  on CurrentThread and `min(requested, worker_threads - 1)` on MultiThread.

- `resolved_runtime_config()` is a `OnceLock` forced by `lib.rs` `init()`, so
  a later `process.env` change cannot make the report differ from the runtime.
- Root `Cargo.toml` `[patch.crates-io]` holds only `parking_lot_core`. On
  `wasm32-wasip1-threads` the released crate picks a parker that panics
  (stable rustc never sets `target_feature = "atomics"`); the fork parks on
  std's futex `Mutex`/`Condvar` for the threaded WASI triples only. Drop it
  once a release ships
  [Amanieu/parking_lot#538](https://github.com/Amanieu/parking_lot/pull/538).

## CurrentThread task host

Each napi env registers a weak threadsafe function with a null JS function. Its
native callback calls `drive_current_thread_tasks`, acknowledges or fails the
delivery, and drops the payload inside
`contain_current_thread_task_host_unwind`. No drive or cancel token crosses JS.
`reserveCurrentThreadHostRegistration()` returns a two-word capability
(fail-closed on `u64` exhaustion); `registerCurrentThreadTaskHost(high, low)`
claims it once. The contract version is 4.

## Timer host

MultiThread uses the crate's timer heap. CurrentThread has no timer driver:
`registerTimerHost` only records the registration (so
`isCurrentThreadHostRegistrationActive` reads true and the cli loaders' host
contract holds) and never calls `schedule` / `cancel`. A CurrentThread
`sleep_until` hits the crate's "no live timer driver" panic. Why: design.md,
"Threads follow the artifact".

## Deferred drop worker

`crates/rolldown/src/utils/defer_drop.rs` frees heavy post-build values on one
plain `std::thread`, off the shared pool; its module doc lists the call-site
rules. On wasm the drop runs inline. If the thread cannot be created, drops run
synchronously. A panicking destructor is contained and a guard retires the
pending count, so `drain()` cannot wedge.

## TypeScript host layer

- No compatibility layer: package and binding ship from one commit. A missing
  host export fails with `ERR_NAPI_ASYNC_RUNTIME_BINDING_MISMATCH` (the workerd
  deferred loader throws a `TypeError` instead).
- The generated WASI loaders install both CurrentThread hosts through
  `installCurrentThreadHosts` from `@napi-rs/async-runtime`
  (`napi.wasm.asyncRuntime: true`), which owns the host protocol and the v4
  gate. The workerd deferred loader registers them through
  `registerWorkerdCurrentThreadTaskHost` / `registerWorkerdTimerHost` from
  `@napi-rs/async-runtime/workerd`. Native runs MultiThread, needs no host, and
  installs none.
- `__napiBindingTarget` reports the artifact ABI (`wasm32-wasi`), not the
  `capabilities.target` spelling (`wasi-threads`).

## WASI loaders and the crash latch

`@napi-rs/cli` emits every loader. At bundle time rolldown patches only
`binding.cjs` (the webcontainer fallback) and the threaded Node worker's runtime
require, and reads the threadless loader's metadata header (`build.ts`);
`workerd-loader.test.ts` matches the browser loader's import block. A cli bump
that drops a loader piece fails in the lane that runs it:

| loader piece           | caught by                                                                                                                           |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| context lifecycle      | `test:wasi-runtime-lifecycle`, `wasi-loader-behavior.test.ts`, `workerd-loader.test.ts`                                             |
| crash latch            | `test:wasi-worker-crash`                                                                                                            |
| pool worker preload    | `test:wasi-pool-preload`                                                                                                            |
| host exports           | every WASI load: `installCurrentThreadHosts` throws `ERR_NAPI_ASYNC_RUNTIME_BINDING_MISMATCH`, the workerd registrars a `TypeError` |
| any loader text change | CI "Check no diff" after the native build (`reusable-native-build.yml`)                                                             |

### Crash latch

This is the threaded Node loader's latch. The threaded browser loader shares
the addon crash flag with its workers (posted as `__napiRsAddonCrashFlag`) but
has no crash reject in its disposer, no exit listener, no `__wasiReentryClosed`
gate and no timer release.

After a pool worker's wasm thread dies, no teardown may re-enter wasm: it would
wait forever for the dead thread, in a raw atomic wait that ignores SIGTERM.

- The exit listener only terminates the workers.
- The disposer `Symbol.for('napi.rs.wasi.dispose')` terminates them and
  rejects, latched, with the crash as `cause` plus `workerThreadId`. It does not
  destroy the emnapi context, and it unrefs emnapi's request-counter port so the
  process exits.
- Inside wasm, the shutdown waits check a shared crash word between short
  slices and trap once it is set (`napi_wasm_thread_crash_flag_address` →
  `workerData.addonCrashFlag`).
- Both crash paths release the host timers in plain JS
  (`__releaseCurrentThreadHostTimers`), and emnapi deferred calls are dropped
  once `__wasiReentryClosed` is set.

Not covered: a worker that fails before its own code runs, or a Node pool
thread spawned before `beforeInit` that fails while loading, raises no addon
flag (one that loaded raises it through `napi_wasm_thread_crashed`). Still
ungated: `FinalizationRegistry` `_free` on GC, async-send type 1
`Promise.then`, `_emnapi_next_tick`.

Open product call: should an idle preloaded Worker's load failure latch the
crash flag at all? Today it does.

Test: `tests/wasi/worker-crash-latch.mjs` (CI "Threaded WASI worker crash latch").

## Data flow

```
 JS build call ──▶ env.spawn_future ──▶ RolldownAsyncRuntime::spawn
                                             └─▶ try_spawn(task).detach()   [crate executor]
 module_loader.spawn_module_task ──▶ supervised_module_task ──▶ try_spawn_detached
 stages/* ──▶ rolldown_utils::rayon par_iter  (same pool under MultiThread)
```

CurrentThread wake (no token crosses JS):

```
 crate executor needs a turn
   └─▶ NativeCurrentThreadTaskHost::dispatch ─(napi_call_threadsafe_function)─▶ JS event-loop turn
         └─▶ call_native_current_thread_task_host (extern "C", native)
               └─▶ drive_current_thread_tasks(capability)  ──▶ ack / fail delivery
```

## Build gates and the no-tokio check

- The binding enables napi's `async-runtime` feature, never `napi/async`
  (which pulls `tokio_rt`).
- `build.rs` emits `rolldown_wasi_threads` only for `wasm32-wasip1-threads`;
  the two WASI targets share one rustc cfg set.
- `just check-no-tokio` runs `cargo tree -i tokio -e no-dev` on
  `rolldown_binding` for native and both WASI targets, and `cargo tree -i tokio`
  (dev edges included) on `bench`. tokio stays a dev-dependency of `rolldown`,
  `rolldown_dev`, `rolldown_plugin_replace` and `rolldown_watcher`, and a
  dependency of the test crate `rolldown_testing`.
- `runtime-submission-failure-test` (`just build-rolldown-async-runtime`, not
  shipped) exports stop/start probes that make one submission reject;
  `async-runtime-worker-teardown.test.ts` uses them.

## Workflow gating

`src/runtime-support.ts` maps the binding report to public features;
`assertRuntimeFeature` throws `ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE`.

### Dev and watch support (`devSupported`, `watchSupported`)

- `devSupported` is true only on MultiThread. `dev()` checks it before any
  callback, plugin hook, worker or `BindingDevEngine`.
- `watchSupported` is false on every wasm artifact. `watch()` checks it first
  and throws synchronously, before any options hook or watcher exists.
- `workerd` is true only in the package build that exposes
  `@rolldown/browser/workerd`.
- `tests/wasi-runtime-lifecycle-case.mjs` (run by
  `tests/wasi-runtime-lifecycle.mjs` on both threaded lanes) asserts
  `getRuntimeSupport().dev === !laneIsSingle`.

### Parallel plugins

Native only: `defineParallelPlugin()` rejects on both WASI flavors. A
descriptor built by hand is still caught: `rolldown()` flattens
`input.plugins` and checks for the `_parallel` marker before the `options`
hook, and `initializeParallelPlugins` checks again once it finds a descriptor
(this also covers output plugins and hook-added plugins). The browser build
skips `initializeParallelPlugins`; there `bindingifyInputOptions` checks every
descriptor in the final plugin list and throws.
`getParallelPluginInfo` counts only an own data property, so an inherited or
accessor `_parallel` is not a marker.

### Plugin error metadata

Structured plugin error metadata works on every artifact. napi keeps a thrown value
without coercion (primitives too) and `ToNapiValue for JsError` returns it
verbatim; `BindingError::from_napi_error` shares it with `try_clone`;
`getErrorMessage` renders a thrown `null` / `undefined` as text.

**On every napi bump, rerun the three metadata regressions — threaded WASI,
threadless WASI, browser build.**

## WASI artifacts

Why two: design.md, "Why two WASI artifacts".

| Artifact                  | threaded (`wasm32-wasip1-threads`)                  | single-thread (`wasm32-wasip1`)                         |
| ------------------------- | --------------------------------------------------- | ------------------------------------------------------- |
| wasm                      | `rolldown-binding.wasm32-wasi.wasm`                 | `rolldown-binding.wasm32-wasip1.wasm`                   |
| node loader               | `rolldown-binding.wasi.cjs`                         | `rolldown-binding.wasip1.cjs`                           |
| browser loader            | `rolldown-binding.wasi-browser.js`                  | `rolldown-binding.wasip1-browser.js`                    |
| deferred (workerd) loader | —                                                   | `rolldown-binding.wasip1-deferred.js`                   |
| worker scripts            | `wasi-worker.mjs`, `wasi-worker-browser.mjs`        | —                                                       |
| npm dir / package         | `npm/wasm32-wasi` → `@rolldown/binding-wasm32-wasi` | `npm/wasm32-wasip1` → `@rolldown/binding-wasm32-wasip1` |

The short suffix `-wasm32-wasi` is the **threaded** package.

- `build-binding.ts` runs `napi build` (the cli commits its outputs
  atomically) and deletes the `.node` / `.wasm` artifacts on failure.
  `--preserve-generated-sources` restores the committed text files after a
  test-feature build (`just build-rolldown-async-runtime`).
- `scripts/wasi/stage-wasi-packages.mjs` installs the bundled loaders into both
  packages, removes the vendored `buffer` / emnapi / wasm-runtime /
  `@napi-rs/async-runtime` dependencies, and fails on a bare runtime import left
  (`bare-runtime-imports.mjs`).
- The `emnapi` override in `pnpm-workspace.yaml` pins the runtime matching the
  linked C archives.

### Binding version is set after the build

The generated loader hard-codes the version it expects from each
`@rolldown/binding-*` package, read from `packages/rolldown/package.json` during
`napi build`. pnpm drops semver build metadata when it packs, so a
`+commit.<sha>` version would never match the published manifests.
`reusable-release-build.yml` therefore runs `Build Node binding` before
`Determine Version`. The npm release flow leaves `version` at its default,
`noop`.

### Declaration regen order

A WASI build writes only its own flavor's loaders and `.d.cts`, and points
`browser.js` at its own package. A native build re-renders every WASI flavor's
loaders from the `// napi-rs-artifact-metadata:` header of the **committed**
loader, writes each `.d.cts` back unchanged, and points `browser.js` at the
threadless package. So a merge that takes another branch's WASI loader drops
this branch's exports at the next native build, until that flavor's WASI build
runs again. After a binding-surface change, a cli bump or such a merge, run each
WASI build, in any order, then native:

```text
just build-rolldown-wasi         # threaded loaders + .d.cts
just build-rolldown-wasi-single  # threadless loaders + .d.cts
just build-rolldown              # native last: browser.js and dist back to native
```

CI's "Check no diff" (`reusable-native-build.yml`) covers every committed
loader; the browser build's drift allowlist in `ci.yml` is `binding.d.cts`.

### Threadless memory

The threadless loaders start at `napi.wasm.threadlessInitialMemory` (about 64
MiB); `initialMemory` (1 GiB) is for the threaded flavor.
The cli checks the page range and that it stays below `maximumMemory`; a value
below the wasm's `env.memory` minimum fails at the first instantiate. `workerd-loader.test.ts` fails above 128 MiB.
Production committed memory needs Workers telemetry.

### Pool worker preload

Threaded Node loader only. After load, `__reconcileWasiThreadPool()` reads
`napi_wasm_runtime_pool_workers` (`workerThreads` under MultiThread, else 0, and
0 once a backend started) and fills emnapi's idle pool without waiting. A
terminated Worker stays tracked until it exits. emnapi replaces an idle Worker
whose load failed. Why: design.md, "Threads follow the artifact".

`tests/wasi/pool-worker-preload.mjs` checks the counts per flavor, import-only
exit, load failure, that every Worker exits after the disposer, and that the
disposer settles in-flight `transform()` work.

### Threaded WASI heap sync

Rolldown has no allocator, link-arg or handoff code of its own. `napi` turns on
napi-build's `wasi-heap-sync` feature, so `napi_build::setup()` in
`crates/rolldown_binding/build.rs` links the lock on `wasm32-wasip1-threads`.
On wasm the binding declares no `#[global_allocator]`
(`crates/rolldown_binding/src/lib.rs`), so std's `System` ends in wasi-libc's
`malloc` and goes through the lock. How: napi-rs `cli/docs/wasi.md`,
["Shared memory growth on `wasm32-wasip1-threads`"](https://github.com/napi-rs/napi-rs/blob/main/cli/docs/wasi.md#shared-memory-growth-on-wasm32-wasip1-threads).
Why: napi-rs
[`cli/docs/wasi-heap-sync-design.md`](https://github.com/napi-rs/napi-rs/blob/main/cli/docs/wasi-heap-sync-design.md).

| piece                   | where                                                                                                                                |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| stress harness          | `packages/rolldown/tests/wasi/threaded-memory-stress.mjs` (options in its header), scripts in `packages/rolldown/tests/package.json` |
| wasm section reader     | `packages/rolldown/tests/wasi/wasm-sections.mjs` (the `malloc` / `free` forward check, the memory import minimum)                    |
| worker-crash latch test | `packages/rolldown/tests/wasi/worker-crash-latch.mjs` (+ `crash-injector-{preload,worker}.mjs`)                                      |
| CI                      | `.github/workflows/reusable-wasi.yml`: the stress steps of the `run` job and the `release-threaded` job                              |

- **One napi-sys in the graph.** The shared size (`MAX_SEEN_PAGES`) is a static in
  napi-sys. With two copies (rolldown's napi and an `oxc_*_napi` crate on different
  ones) the allocator would publish into one and the handoff refresh would read
  the other. Check after any change to the napi pins:

  ```
  cargo tree --target wasm32-wasip1-threads -i napi-sys      # exactly one, from the registry
  cargo tree --target wasm32-wasip1-threads -d --depth 0     # no napi* line
  ```

- **A release-profile lane.** The code is profile-sensitive (LLVM in `release-wasi`
  once removed allocator code the debug build kept), so `release-threaded` builds
  the wasm the way the release workflow does and runs the heap-sync checks against
  that artifact.

**Exposure.** Without V8's wasm trap handler, plain loads and stores are
bounds-checked against a thread's stale size too. Of the official Node builds
without the handler, aix-ppc64 and win-x86 have no native rolldown binding, so
the loader falls back to the threaded WASI package there when it is installed.
Elsewhere the loader takes it when the native binding fails to load, with
`NAPI_RS_FORCE_WASI=true` (WASI first, native as the fallback) or `=error` (WASI
only), or with `NAPI_RS_WASI_FLAVOR=wasm32-wasi` (exactly the threaded flavor).

When napi-rs drops the workaround (its design doc, "When to remove"), delete:

- the heap-sync options, cases and forward check of `threaded-memory-stress.mjs`,
  and `wasm-sections.mjs` once nothing reads it;
- the `test:wasi-threaded-stress*` scripts and their steps in `reusable-wasi.yml`,
  the heap-sync part of the `release-threaded` job, the "No retry" comment on
  the MultiThread `Node Test` step, and the retry note on the CurrentThread step;
- the comments and links that point here or at the napi-rs design doc
  (`crates/rolldown_binding/src/async_runtime.rs`,
  `crates/rolldown_binding/build.rs`, `packages/rolldown/tests/threaded-wasi.test.ts`,
  design.md "Threads follow the artifact");
- this subsection.

### Publishing and CI

- `napi.rootPublisher: "pnpm"` makes the pre-publish validator check
  `publishConfig.exports` instead of `exports`. The threadless target makes
  pre-publish add the `./workerd` / `./wasm` / `./wasm.wasm` facade subpaths to
  that map.
  No CI lane runs that facade, npm/Yarn installs, or the threaded
  `wasi-browser.js` in Chromium.
- Packed packages are tested in Chrome (`packages/browser-tests`), in the
  WebContainer fallback test, and for the `workerd` export condition.
- Real workerd: `ci.yml` runs `packages/workerd-tests/suite.mjs`;
  `reusable-wasi.yml` and `reusable-release-build.yml` run `memory.mjs`.
- The managed workerd instance:
  [workerd-managed-instance](../workerd-managed-instance/implementation.md).

## Invariants

- No tokio in the shipped graph: `just check-no-tokio`.
- One env read, frozen snapshot: `RuntimeEnv::from_process`,
  `resolved_runtime_config()`.
- 256 ceiling, one lane kept runnable: `resolve_thread_count`
  (`crates/rolldown_binding/src/env_config.rs`), `clamp_shared_blocking_tasks`.
- Native is MultiThread, threadless WASI is CurrentThread:
  `resolve_runtime_config_for`.
- Host capabilities are single-use and fail closed:
  `reserve_host_registration_id`, `claim_host_registration_id`.
- No token crosses JS: the drive runs in `call_native_current_thread_task_host`.
- Panics are contained at every FFI or drop boundary; caught payloads go
  through `rolldown_std_utils::{panic_payload_message, discard_panic_payload}`
  (design.md, "Caught panic payloads are user code").
- One module task, one diagnostic: `supervised_module_task` and
  `ModuleTaskSupervisor::Drop`.
- No JS runtime-ownership protocol: the N-API env lifecycle owns the runtime.

## Related

- [design.md](./design.md) — principles and the two WASI artifacts
- [workerd-managed-instance](../workerd-managed-instance/implementation.md)
- [napi-rs `wasi-heap-sync-design.md`](https://github.com/napi-rs/napi-rs/blob/main/cli/docs/wasi-heap-sync-design.md)
  — the threaded WASI memory-size bug and its fix
- [bundler-data-lifecycle](../bundler-data-lifecycle/implementation.md) —
  rebuild ownership and `ScanStageCache`; deferred drops are in
  [Deferred drop worker](#deferred-drop-worker)
- [watch-mode](../watch-mode/implementation.md) — the `sleep_until` consumer
- `docs/guide/wasi.md` — flavors, environment variables, support matrix
