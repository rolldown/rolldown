<!-- The IMPLEMENTATION doc captures the "how": components, data flow,    -->
<!-- control flow, file pointers, invariants as realized in code.         -->
<!-- It pairs with design.md (the "why") — link to it for rationale.      -->

# Async Runtime — Implementation

> The rationale, principles, and rejected alternatives behind this live in
> [design.md](./design.md). This file is the implementation map: where the
> runtime is selected, configured, bridged, and consumed across rolldown's
> three layers. "Principle N" below refers to design.md; "§N" refers to a
> section here. It describes **facts** about the current code, not the
> narrative of any change.

## Summary

The scheduler itself does not live in this repo. The
[`napi-async-runtime`](https://crates.io/crates/napi-async-runtime) crate (a
git pin, see §9) owns every executor internal — the MultiThread work-stealing
executor, the `crossbeam` injector, parked-driver bookkeeping, the blocking
owner-lane lending machine, the timer heap, generations, and the
CurrentThread task-host registry/TSFN publication protocol. Rolldown only
**selects, configures, consumes, and bridges** it. This document maps that
integration surface:

```
  crates/rolldown_utils          thin facades  →  napi_async_runtime::*        (Rust core calls these)
        │
  crates/rolldown_binding        the napi backend adapter + host bridges       (JS ⇄ Rust boundary)
        │
  packages/rolldown/src/*.ts     config API, capability gating, host install   (register-only)
```

`crates/rolldown_utils/src/lib.rs` re-exports the crate wholesale
(`pub use napi_async_runtime::*` in the `async_runtime` module), so every
`try_spawn` / `drive_current_thread_tasks` / `Sleep` / `RuntimeOptions`
symbol named below is **provided by the crate**, not defined here.

---

## 1. Where the runtime lives, and the backend adapter

Rolldown does **not** use the crate's own napi adapter. It vendors a
zero-sized backend and registers it with napi at module init; napi's SPI then
routes every JS-triggered async operation through that adapter into the
crate's fallible `try_*` API.

- `crates/rolldown_binding/src/async_runtime.rs` — `struct RolldownAsyncRuntime`,
  `unsafe impl AsyncRuntime` — the vendored napi backend. Method map:
  - `spawn` → `try_spawn(task).detach()`
  - `block_on` → `try_block_on_dyn(future)`
  - `spawn_blocking` → `try_spawn_blocking(work).detach()`
  - `start` / `shutdown` → the crate's `start()` / `shutdown()`
  - The `unsafe impl` SAFETY comment records the no-tokio / waker-retention
    justification (napi permanently pins the native image after export — see
    Principle 7's addon-retention note).
- `crates/rolldown_binding/src/async_runtime.rs` — `install_async_runtime_backend()`
  (`#[napi_derive::module_init]`) — the single backend-selection/registration
  point: builds `RuntimeOptions` from the resolved snapshot (§3), calls the
  crate's `configure(options)`, then `register_async_runtime(RolldownAsyncRuntime)`.
  No `cfg` gate — every target compiles it (§9).
- `crates/rolldown_binding/src/utils/mod.rs` — `spawn_boxed_future()` — the
  JS-entry helper that boxes a future and hands it to `env.spawn_future`
  (i.e. into `RolldownAsyncRuntime::spawn`); used by the bundler entry points
  and `binding_dev_engine.rs`.

Backend flavor is **not** a runtime branch here: it is resolved once (§3) and
fed to `configure`; the crate picks the executor from `RuntimeOptions.flavor`.

---

## 2. Rust-core consumption (facades, module loader, Rayon)

- `crates/rolldown_utils/src/futures.rs` — thin facades `spawn`, `try_spawn`,
  `spawn_detached`, `spawn_blocking`, `block_on`, `block_on_spawn_all`, each
  delegating to `crate::async_runtime::*`; `JoinHandle` / `JoinError` /
  `SpawnError` re-exported from the crate.
- `crates/rolldown/src/module_loader/module_loader.rs`:
  - `spawn_module_task()` — boxes the (large) module future once at the spawn
    boundary, wraps it in `supervised_module_task`, submits via
    `try_spawn_detached`; drops the future on rejection.
  - `supervised_module_task()` + `struct ModuleTaskSupervisor` (and its `Drop`)
    — the **"one accepted supervised task"** guarantee (Principle 8, last
    bullet): a `catch_unwind` around the module future turns panic, shutdown
    cancellation, or rejected submission into exactly one `ModuleLoaderMsg`
    diagnostic so completion accounting cannot hang.
  - the consumer loop pumps supervisor messages with
    `rolldown_utils::futures::block_on(async { rx.next().await })` over an
    unbounded channel (a bounded one could deadlock the `block_on`-pinned JS
    thread) and updates module info with `into_par_iter()` on Rayon.
- CPU parallelism uses Rayon's **process-global** pool via
  `rolldown_utils::rayon` throughout `stages/link_stage/**` and
  `stages/generate_stage/**`. Under MultiThread this is the same pool the
  executor polls futures on (Principle 2); no second pool is built —
  `stages/generate_stage/minify_chunks.rs` sizes its `AllocatorPool` from
  `rayon::current_num_threads()`, and there is no `build_global` in
  production code.

---

## 3. Configuration: one read, one snapshot, frozen after first use

All env vars are read in exactly one place, resolved through one pure
per-target table, snapshotted in a `OnceLock`, and forced at module load so a
later `process.env` mutation cannot diverge the report from the built runtime
(Principle 6). JS overrides go through a separate validated patch API the
crate freezes after first use.

- `crates/rolldown_binding/src/async_runtime.rs`:
  - `RuntimeEnv::from_process()` — the **only** env-read site: `ROLLDOWN_RUNTIME`,
    `ROLLDOWN_WORKER_THREADS`, `ROLLDOWN_MAX_BLOCKING_THREADS`,
    `ROLLDOWN_PARK_DEADLINE_MS`, `ROLLDOWN_DRAIN_LINGER_US`.
  - `resolve_runtime_config_for(target, env)` — pure defaults table. Native ⇒
    MultiThread; wasm ⇒ CurrentThread, normalizing an inherited
    `ROLLDOWN_RUNTIME=multi` because the crate has no wasm MultiThread executor
    (Principle 1). MultiThread worker count `= requested.max(2)` (truthful
    two-worker minimum); CurrentThread `= 1`.
  - `clamp_shared_blocking_tasks()` — blocking cap: CurrentThread ⇒ 1;
    MultiThread ⇒ `requested.min(worker_threads - 1).max(1)` (reserve one
    runnable lane, Principle 3).
  - `resolved_runtime_config()` — the process `OnceLock<ResolvedRuntimeConfig>`.
  - `compiled_target()` — `Native` / `Wasi` / `WasiThreads` via
    `cfg!(rolldown_wasi_threads)` (§9).
  - `validate_binding_thread_count()` + `TryFrom<BindingRuntimeOptions> for
RuntimeOptionsPatch` — the **256-ceiling / positive-integer / atomic-reject**
    validation for the JS `configureAsyncRuntime` path
    (`MAX_ASYNC_RUNTIME_WORKER_THREADS`).
  - `configure_async_runtime()` (`#[napi]`) → the crate's `configure_partial`
    (merge+validate+commit under the controller mutex, frozen after the first
    backend); `get_async_runtime_config()` → `configured_options()` is the
    reporting authority.
- `crates/rolldown_binding/src/env_config.rs` — `resolve_thread_count(raw,
default, maximum)` — shared clamp; treats `0`/garbage as unset so it cannot
  panic the constructor's `validate()`.
- `crates/rolldown_binding/src/lib.rs` — `init()` (`#[module_init]`) — forces
  `resolved_runtime_config()` at load on every artifact.
- `crates/rolldown_utils/src/lib.rs` / `src/time.rs` —
  `MAX_ASYNC_RUNTIME_WORKER_THREADS = 256`; `max_async_runtime_worker_threads()`
  = `256.min(rayon::max_num_threads())` native, `1` on wasm.

---

## 4. CurrentThread task-host bridge (native TSFN, contract v4)

The task host is **register-only** on the JS side. Each importing napi env
installs a _weak_ threadsafe function whose JS function pointer is null and
whose native callback drives tasks; **no drive/cancel token ever crosses
JavaScript** (Principle 7's task-host boundary).

- `crates/rolldown_binding/src/async_runtime.rs`:
  - `struct NativeCurrentThreadTaskHostInner` — owns the TSFN raw slot, the
    `dead` / `environment_closing` flags, the host-registration id, and the
    crate-issued driver id.
  - `NativeCurrentThreadTaskHostInner::new()` — creates the weak TSFN
    (`napi_create_threadsafe_function` with null js_func +
    `call_native_current_thread_task_host` as the C callback), then unrefs it.
  - `call_native_current_thread_task_host()` (`extern "C"`) — the native turn:
    `drive_current_thread_tasks(delivery.capability())`, retain the opaque
    `callback_lease`, `acknowledge_current_thread_task_delivery` (or
    `fail_current_thread_task_delivery`), drop the payload, drop the lease
    last — all inside `contain_current_thread_task_host_unwind`.
  - `register_current_thread_task_host()` (`#[napi]`, `dispatch?: never`) — the
    JS entry: rejects any JS callback synchronously, claims the reserved
    registration id, `register_current_thread_task_driver(...)`, requests an
    initial drain, installs the env-cleanup hook.
  - Capability handshake: `reserve_current_thread_host_registration()` →
    `reserve_host_registration_id()` (SeqCst `fetch_update`, fail-closed on
    `u64` exhaustion); `claim_host_registration_id()` consumes it once;
    `BindingHostRegistration { high, low }` is the two-word capability.
  - `get_current_thread_task_host_contract_version()` — returns
    `CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION = 4`.

---

## 5. Timer host (host-delegated CurrentThread `sleep_until`)

`sleep_until` is a runtime-independent facade. MultiThread uses the crate's
timer heap + service thread (Principle 4). CurrentThread cannot park a helper
thread, so each timer is delegated to the JS event loop via `schedule`/`cancel`
callbacks registered per env.

- `crates/rolldown_utils/src/time.rs` — `sleep_until(deadline) -> Sleep` — the
  facade (delegates to `napi_async_runtime::sleep_until`).
- `crates/rolldown_watcher/src/watch_coordinator.rs` — the sole consumer
  (watch-mode debounce; a comment notes tokio's `sleep_until` would panic
  here).
- `crates/rolldown_binding/src/async_runtime.rs` — `struct JsTimerHost` /
  `JsTimerHostInner`, `impl TimerDriver` — `register(id, deadline, waker)` arms
  one host timeout via a detached relay task (races cancel vs the JS
  `schedule` future, with three-strike transient-failure eviction); `cancel`,
  `is_live`, `on_swept`. `register_timer_host()` (`#[napi]`, `schedule` /
  `cancel` JS callbacks) installs it via the crate's `register_timer_driver`.
  `get_runtime_capabilities().timers` = MultiThread ⇒ true; CurrentThread ⇒
  `has_live_timer_driver()`.

---

## 6. Deferred-destruction / serial maintenance worker

A dedicated single OS thread frees heavy post-build values off the critical
path so a one-worker rebuild never waits on a drop queued behind itself in the
shared pool (Principle 8). It is a plain `std::thread` + mpsc + Condvar — it
deliberately does **not** use the async runtime, so it stays off the shared
pool — but it shares the same panic-containment discipline.

- `crates/rolldown/src/utils/defer_drop.rs` — `spawn_drop<T>(value)` (enqueue;
  wasm-gated inline drop since the browser main thread cannot `Atomics.wait`),
  `drain()` (blocks on a `PENDING` Condvar; called at every shared-pool build
  entry), `run_drop_safely` / `PendingGuard` (nested `catch_unwind`, bottoming
  out with `mem::forget`, mirroring the binding's
  `contain_current_thread_task_host_unwind`).
- Call sites: `crates/rolldown/src/bundle/bundle.rs` (`spawn_drop`, one per
  build), `crates/rolldown/src/bundle/bundle_factory.rs` and
  `crates/rolldown/src/bundler/impl_bundler_hmr.rs` (`drain()` at the build /
  HMR-partial entries).
- Cross-links [bundler-data-lifecycle](../bundler-data-lifecycle/implementation.md).

---

## 7. The TypeScript host layer

The JS side installs the host bridges, gates workflow features on the native
capability report, and (for legacy artifacts only) manages runtime leases. It
never drives tasks.

- **Host install (register-only, contract-gated)** —
  `packages/rolldown/src/timer-host.ts` installs **both** the task host and the
  timer host as a module side effect. Before any native side effect it verifies
  `getCurrentThreadTaskHostContractVersion() === 4`, then reserves + validates
  the capability, then calls `registerCurrentThreadTaskHost(high, low)`
  (no callback) and `registerTimerHost(high, low, schedule, cancel)`. The
  timer host arms `setTimeout` hops (chunked to `MAX_HOST_TIMEOUT_MS`) and, on
  `cancel`, clears the timeout **and** resolves the relay promise (dropping a
  sleep must not wait out the deadline). Installed once per binding via a
  per-realm `Symbol.for('rolldown.current-thread-host-installations.v4')`
  WeakMap. Every native package entry pulls it in through a side-effect
  `import './timer-host'` (`setup.ts`, `config.ts`, `plugins-index.ts`,
  `parallel-plugin-worker.ts`, `experimental-index.ts`, `utils-index.ts`,
  `parse-ast-index.ts`, and `cli/timer-host-entry.ts`).
- **Config / metrics API** — `packages/rolldown/src/api/async-runtime.ts`
  (`configureAsyncRuntime`, `getAsyncRuntimeConfig` incl. the `drainLingerUs`
  field, `getAsyncRuntimeMetrics` with `max* ≥ live*` enforcement,
  `normalizeAsyncRuntimeTopology` enforcing CurrentThread ⇒ both counts = 1).
- **Capability gating** — `packages/rolldown/src/runtime-support.ts`
  (`getRuntimeCapabilityReportCompat`, `normalizeRuntimeCapabilities`
  cross-checks, `getRuntimeSupport` → `threadlessWasi` / `workerd` / `dev` /
  `watch`, `assertRuntimeFeature`). A binding with **no** capability reporter is
  treated as legacy: `getLegacyRuntimeCapabilities` synthesizes
  `backend:'tokio'`; a _partial_ contract throws `BindingMismatchError`.
- **Loaders** — `packages/rolldown/src/binding.cjs` (native; line-8
  `loadedBindingTarget='native'`, exported as `__rolldownBindingTarget`),
  `rolldown-binding.wasi.cjs` / `rolldown-binding.wasi-browser.js` (threaded
  WASI, target `wasi-threads`, emnapi TSFN/async-work plugins). The generated
  loaders are patched by `packages/rolldown/binding-loader-codegen.ts`, whose
  `assertAsyncRuntimeHostExports` guarantees every host export survives codegen.
- **Lifecycle leases** — `packages/rolldown/src/runtime-lifecycle.ts`
  (`acquireRuntimeLease`, `isRuntimeLeaseRequired` — real leases only for
  `target==='wasi-threads' && backend==='tokio'`, i.e. legacy; every current
  binding gets `NOOP_LEASE`). Acquire/release with `AggregateError`-aggregated
  cleanup at `api/experimental.ts` (`scan`), `api/watch/watcher.ts`
  (single-flight `close`), `api/dev/dev-engine.ts`, and
  `api/rolldown/rolldown-build.ts`.

---

## 8. Cross-layer data flow

A JS-triggered build (spawn path):

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

CurrentThread timer:

```
 rolldown_utils::time::sleep_until ──▶ crate TimerDriver ──▶ JsTimerHost::register
   └─▶ JS schedule(id, ms) ⇒ setTimeout hops ⇒ resolve relay ⇒ waker fires
```

---

## 9. Build, targets, and the no-tokio gate

- `crates/rolldown_binding/Cargo.toml` — `napi = { features = ["async-runtime"] }`
  enables the pluggable-SPI (napi4) but deliberately **not** `napi/async`
  (which would pull `tokio_rt`). The old `tokio-runtime` / `async-runtime`
  feature pair is gone (Principle 9); every target compiles the shared runtime
  unconditionally.
- `crates/rolldown_utils/Cargo.toml` — `napi-async-runtime = { git = …, rev =
9999dad3…, default-features = false }` (napi-free consumption). The root
  `Cargo.toml` `[patch.crates-io]` redirects the single shared `napi` node
  graph-wide to the same rev — one non-prerelease `3.12.0` node covers
  `rolldown_binding` **and** every `oxc_*_napi`.
- `crates/rolldown_binding/build.rs` — emits `cargo::rustc-cfg=rolldown_wasi_threads`
  only for `wasm32-wasip1-threads` (the two WASI targets are otherwise
  cfg-indistinguishable); consumed by `compiled_target()`.
- `Justfile` — recipe **`check-no-tokio`** proves the shipped graph is
  tokio-free via `cargo tree -i tokio` over four scopes:
  `-e no-dev -p rolldown_binding` (native), the same with
  `--target wasm32-wasip1` and `--target wasm32-wasip1-threads`, and
  `-p bench`. The lone `tokio` entry (in `crates/rolldown` `[dev-dependencies]`)
  is excluded by `-e no-dev`.

---

## Invariants (each tied to a file)

- **No tokio in the shipped graph** — `Justfile::check-no-tokio`;
  `rolldown_binding/Cargo.toml` uses `async-runtime`, not `async` (Principle 9).
- **Shared runtime on every target** — `install_async_runtime_backend`
  (`#[module_init]`, no cfg gate).
- **Single env read + frozen snapshot** — `RuntimeEnv::from_process` is the
  only reader; `resolved_runtime_config()` `OnceLock`; forced by `lib.rs::init`
  (Principle 6).
- **256 ceiling + reserve-one-lane** — `validate_binding_thread_count`,
  `clamp_shared_blocking_tasks` (Principle 3).
- **wasm flavor normalization** — `resolve_runtime_config_for` forces
  CurrentThread off-native (Principle 1).
- **Capability single-use / fail-closed ids** — `reserve_host_registration_id`
  (`u64`-exhaustion error) + `claim_host_registration_id` (Principle 7).
- **Contract v4, no token crosses JS** — the drive call is entirely inside
  `call_native_current_thread_task_host`; `timer-host.ts` gates on
  `getCurrentThreadTaskHostContractVersion() === 4` before any native effect.
- **Panic containment at every FFI/drop boundary** —
  `contain_current_thread_task_host_unwind` (binding),
  `run_drop_safely` / `PendingGuard` (`defer_drop.rs`).
- **Module-loader = one accepted supervised task** — `supervised_module_task`
  - `ModuleTaskSupervisor::Drop` (Principle 8).

## Related

- [design.md](./design.md) — the principles and trade-offs behind this
- [bundler-data-lifecycle](../bundler-data-lifecycle/implementation.md) —
  deferred drops and rebuild ownership (§6)
- [watch-mode](../watch-mode/implementation.md) — the `sleep_until` debounce
  consumer (§5)
