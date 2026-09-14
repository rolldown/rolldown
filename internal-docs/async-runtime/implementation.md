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
published release, see §9) owns every executor internal — the MultiThread
work-stealing executor, the `crossbeam` injector, parked-driver bookkeeping, the blocking
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

The binding compiles the shared runtime unconditionally, on every target;
there is no build that selects a different executor (§9).

---

## 1. Where the runtime lives, and the backend adapter

Rolldown does **not** use the crate's own napi adapter. It vendors a
zero-sized backend and registers it with napi at module init; napi's SPI then
routes every JS-triggered async operation through that adapter into the
crate's fallible `try_*` API.

The vendored copy is deliberate, not a fork: `RolldownAsyncRuntime` mirrors
`napi-async-runtime` 0.2.0's own `adapter.rs` method for method. It exists
only because 0.2.0 has no public API for installing that adapter from a
downstream crate. When 0.2.1 exposes one, the vendored struct is deleted and
the crate's adapter is installed directly; until then, edits to the mirrored
methods must track upstream rather than diverge from it.

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
  Unconditional — every target compiles it (§9).
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
- If the operating system refuses to create the maintenance thread, deferred
  destruction falls back to synchronous, panic-contained drops — moving
  destruction off the caller is an optimization, not a correctness
  requirement. The pending count is retired only after both unwind boundaries
  complete, so the next build cannot begin while a caught panic payload is
  still being destroyed.
- Call sites: `crates/rolldown/src/bundle/bundle.rs` (`spawn_drop`, one per
  build), `crates/rolldown/src/bundle/bundle_factory.rs` and
  `crates/rolldown/src/bundler/impl_bundler_hmr.rs` (`drain()` at the build /
  HMR-partial entries).
- Cross-links [bundler-data-lifecycle](../bundler-data-lifecycle/implementation.md).

---

## 7. The TypeScript host layer

The JS side installs the host bridges and gates workflow features on the
native capability report. It never drives tasks.

There is no compatibility layer for a binding built from another commit. The
JavaScript package and the binding ship from the same commit with exact
version pins, so a loaded binding that does not match this package is a
broken install, not a lane: the capability report is required, every field is
validated, and a report that does not agree fails with
`ERR_ROLLDOWN_BINDING_MISMATCH` instead of being synthesized or defaulted.

- **Host install (register-only, contract-gated)** —
  `packages/rolldown/src/timer-host.ts` installs the task host and (on
  non-browser builds) the timer host as a module side effect. Before any
  native side effect, it verifies
  `getCurrentThreadTaskHostContractVersion() === 4`, then reserves + validates
  the capability, then calls `registerCurrentThreadTaskHost(high, low)`
  (no callback) and `registerTimerHost(high, low, schedule, cancel)`. The
  timer host arms `setTimeout` hops (chunked to `MAX_HOST_TIMEOUT_MS`) and, on
  `cancel`, clears the timeout **and** resolves the relay promise (dropping a
  sleep must not wait out the deadline). On the **browser** build the timer
  registration is guarded by `!import.meta.browserBuild` (timer-host.ts), so a
  browser entry installs only the task host and reports `timers: false` — the
  browser event loop backs timers directly. Installed once per binding via a
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
  `watch`, `assertRuntimeFeature`). A missing reporter or a partial contract
  throws `BindingMismatchError`.
- **Loaders** — `packages/rolldown/src/binding.cjs` (native; line-8
  `loadedBindingTarget='native'`, exported as `__rolldownBindingTarget`),
  `rolldown-binding.wasi.cjs` / `rolldown-binding.wasi-browser.js` (threaded
  WASI, target `wasi-threads`, emnapi TSFN/async-work plugins). The generated
  loaders are patched by `packages/rolldown/binding-loader-codegen.ts`, whose
  `assertAsyncRuntimeHostExports` guarantees every host export survives codegen.

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

- `crates/rolldown_binding/Cargo.toml` — there is no runtime feature to pick.
  The binding depends on `napi` with the pluggable-SPI (napi4) plus
  `AsyncTask`, but deliberately **not** `napi/async` (which would pull
  `tokio_rt`), so every target compiles the shared runtime (Principle 9).
- `crates/rolldown_utils/Cargo.toml` — `napi-async-runtime = { version =
"0.2.0", default-features = false }` from crates.io (napi-free
  consumption). The root `Cargo.toml` pins
  the napi stack to **published crates.io releases** — `napi 3.12.1`,
  `napi-build 2.4.1`, `napi-derive 3.6.3` (resolving `napi-derive-backend
6.1.2` and `napi-sys 3.3.0`) — and carries **no** `[patch.crates-io]`
  section: that single registry `napi` node covers `rolldown_binding` **and**
  every `oxc_*_napi`. The comment above those pins records why the minimum is
  a pin rather than a range.
- `crates/rolldown_binding/build.rs` — emits `cargo::rustc-cfg=rolldown_wasi_threads`
  only for `wasm32-wasip1-threads` (the two WASI targets are otherwise
  cfg-indistinguishable); consumed by `compiled_target()`.
- `Justfile` — recipe **`check-no-tokio`** proves the shipped graph is
  tokio-free via `cargo tree -i tokio` over four scopes:
  `-e no-dev -p rolldown_binding` (native), the same with
  `--target wasm32-wasip1` and `--target wasm32-wasip1-threads`, and
  `-p bench`. The lone remaining `tokio` entry (in `crates/rolldown`
  `[dev-dependencies]`) is excluded by `-e no-dev`, which is why the scopes
  carry that flag.

---

## 10. Dedicated test builds

Two test-only features harden the runtime lane (the justfile's dedicated
test-binding recipe enables both; their exports are absent from production
artifacts):

- `runtime-submission-failure-test` — raw-binding-only stop/start probes shut
  down the real scheduler so one `Env::spawn_future` submission rejects
  before a retry executes the already-memoized close future. The same fixture
  verifies that `BindingWatcher.run()` returns a rejected Promise while
  stopped, retains its coordinator, and starts it exactly once after restart.
- `runtime-waker-teardown-test` — backs the worker-teardown probe
  (`async-runtime-worker-teardown.test.ts`): the suite loads the raw addon
  only inside a worker after the public package installs that environment's
  normal hosts. A pending shared-scheduler task clones its real waker to an
  external native thread. The unreferenced task host allows the worker to
  exit naturally; after environment cleanup has returned, the parent releases
  that thread, which calls `wake_by_ref`, drops the waker, and publishes
  completion. No test-only unregister or forced `Worker.terminate()` masks
  host ownership. The parent process never imports the addon, so survival
  cannot be explained by another live environment retaining the image
  (Principle 7's addon retention). The probe adds no module-count hooks or
  lifecycle locks.

---

## 11. Capability compatibility and workflow gating

Native watch mode is supported on both runtime flavors. Public `dev()` checks
`devSupported` before reading callbacks, running plugin hooks, creating
workers, or constructing `BindingDevEngine`.
Public `watch()` creates its emitter first, checks `watchSupported` before
calling `createWatcher`, and routes failure through `failSetup`; callers
therefore observe `ERROR` followed by `END`, and `close()` remains usable
without any worker or native watcher having been created. WASI watch
remains unsupported because entering the native initial build can park the
JavaScript host thread before debounce timers are involved. The public
`getRuntimeSupport()` report and `ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE`
errors are the workflow-level contract layered over the lower-level binding
capabilities. Its `threadlessWasi` field is deliberately an artifact
compatibility marker rather than a managed-workerd availability claim:
`@rolldown/browser/workerd` is a package-entry contract. Build-time
`import.meta.workerdPackageApi` distinguishes that package from the standalone
threadless `rolldown` artifact, so the enumerable `workerd` field remains a
truthful workflow-level report.

`getRuntimeCapabilities()` also exposes stable public-workflow gates.
`devSupported` follows the effective runtime flavor and is false on
`CurrentThread`; `watchSupported` is false on every WebAssembly artifact. The
TypeScript `runtime-support.ts` layer maps those binding facts to named public
features and throws `ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE` before entering
unsupported setup paths. A missing, invalid, or internally inconsistent field
fails with `ERR_ROLLDOWN_BINDING_MISMATCH`; when loader metadata is available,
its target must also agree with the reporter.
Binding export, reporter, loader-target, and report-field getter failures
preserve their original `cause` under the same mismatch identity. This prevents
a malformed threaded-WASI report from enabling unsupported worker-backed
features. Import-time task and timer host registration reads the same
validated report, so a malformed report fails
before either host can be registered. Stacked host integrations can still
declare richer or narrower workflow support without changing the low-level
scheduler contract. Parallel-plugin descriptor consumption has an additional
synchronous preflight at the public build, rolldown, scan, and dev boundaries
and at `createBundlerOptions`. The latter repeats the preflight immediately
after synchronous `outputOptions` hooks, before normalizing hook-injected
plugins. Each pass recursively inspects only already-materialized own data
properties of plugin arrays without assimilating neighboring thenables,
executing accessors, or using indexed proxy `get` operations. Proxy metadata
reflection (`ownKeys` and `getOwnPropertyDescriptor`) may still run; failures
are contained and the value is deferred to normal plugin materialization.
Accessor-produced values are likewise checked by the post-normalization
capability guard. A fabricated descriptor on an unsupported artifact
therefore fails before the next asynchronous setup boundary, worker registry,
or binding construction. The synchronous descriptor walk tracks visited
arrays, so a malformed cyclic plugin list stays bounded while a materialized
descriptor elsewhere in the graph is still found. Ordinary object plugins do
not trigger that gate.

Structured plugin errors are supported on every artifact, including both WASI
flavors and the managed browser package. N-API errors retain the original
JavaScript exception reference while Rolldown adds `code`, `pluginCode`,
`plugin`, `hook`, and applicable `id` metadata. The same object, stack, own
properties, and nested `cause` chain must survive replayable Rust lifecycle
state and worker/host boundaries. Native, threaded-WASI, threadless-WASI,
browser-build, and packed-browser tests exercise this contract;
`pluginErrorMetadata` is therefore a universal public-support invariant rather
than a target capability.

This invariant depends on the workspace's `napi 3.12.1` registry pin (§9).
Synchronous
threadsafe-function exceptions use
`Error::capture_unknown_with_status_and_diagnostics`, and Promise rejections use
`Error::from_unknown_without_coercion`; both retain the exact JavaScript value
through an owning-environment `napi_ref`. Rolldown's
`downcast_napi_error_diagnostics` and `BindingError::from_napi_error` preserve
that reference with `try_clone`, napi-rs `JsError::into_value` reuses it on the
owning JavaScript thread, and `normalizeBindingError` returns the resulting
`field0` object directly. None of those capture paths is target-gated. Updating
the napi-rs revision requires rerunning the threaded, threadless, browser-build,
and packed-browser metadata regressions before retaining the universal support
claim.

Parallel JavaScript plugins are a native-only workflow. The wasm binding
compiles out the cross-environment parallel plugin registry, so
`defineParallelPlugin()` rejects with
`ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE` on both WASI flavors instead of
spawning workers whose hooks cannot be registered. `getRuntimeSupport()`
reports this through `parallelPlugins`.

Parallel-plugin workers are supervised from construction through shutdown, not
only until their bootstrap message. Delayed worker `error` events and
unexpected exits are retained as close failures instead of becoming uncaught
parent-process events. A supervisor that has already exited does not physically
terminate again, but rejects one cleanup attempt with its retained fault so the
existing retryable-cleanup protocol preserves ownership; the next attempt
clears that logical owner. Bootstrap pools invoke every initializer before
observing the first rejection, and each production initializer registers its
worker synchronously before awaiting bootstrap. Cleanup therefore owns every
constructed sibling without waiting for a startup Promise that may never
settle. The bootstrap Promise has an observer from construction, so an early
worker `error` or `exit` remains replayable without passing through Node's
unhandled-rejection machinery. Readiness and terminal bootstrap messages
travel over a transferred `MessagePort`, isolated from inherited preload and
loader messages on `parentPort`; those private messages and worker exit release
the physical-termination barrier because each proves native-addon registration
has finished. Bootstrap failures are normalized to a cloneable
`ParallelPluginBootstrapError` before crossing `postMessage`. If the
control-port send itself fails, the worker closes/unrefs that port and throws
the cloneable diagnostic from a microtask, ensuring the supervisor sees an
`error` or terminal exit even when unhandled promise rejections are configured
to warn instead of terminating the worker. Once a pool is initialized, every
remaining option-access, warning, binding-conversion, and callback-wrapping
step runs inside the same cleanup boundary so a synchronous setup failure
cannot abandon those workers.

`packages/rolldown/tests/wasi-runtime-lifecycle.mjs` is where the WASI end of
these contracts is pinned: it runs against the threaded artifact, asserts it
reports `backend: 'shared'` with `devSupported: false`, and exercises isolated
loader contexts, pending-promise settlement during context cleanup, same-realm
reload after destruction, selective inherited-worker-argument retry,
overlapping public callers, operation and binding-construction failures, worker
realms, a rejected `dev()` that leaves the runtime usable, fail-closed watch
and parallel-plugin capability detection, and duplicate JavaScript package
copies that resolve one shared binding. The watch case verifies `ERROR`/`END`, repeated close, and that plugin
option hooks never run. A parent-process watchdog runs the suite in a child
process, so a synchronous WASI loader stall fails with a bound instead of
consuming the whole CI job.

---

## 12. Non-threaded WASI

The current-thread executor is the runtime half of the non-threaded
`wasm32-wasip1` build; the browser build targets it directly.

The napi-rs CLI changes from napi-rs#3353 link `libemnapi-basic-napi-rs.a`
(the non-threaded napi-rs flavor shipped by the released emnapi package), emit
unshared `WebAssembly.Memory`, set `asyncWorkPoolSize: 0`, and omit Worker
imports and factories. `packages/rolldown` keeps the threaded WASI scripts and
adds `build-binding:wasi-single`; browser-package scripts select the
single-thread variant.

Each WASI flavor has its own artifact names end to end (napi CLI
`parseTriple`: non-threaded `wasm32-wasipX` triples get their own
`platformArchABI`, threaded flavors keep the legacy `wasm32-wasi` name for
back-compat):

| Artifact                  | threaded (`wasm32-wasip1-threads`)                  | single-thread (`wasm32-wasip1`)                         |
| ------------------------- | --------------------------------------------------- | ------------------------------------------------------- |
| wasm                      | `rolldown-binding.wasm32-wasi.wasm`                 | `rolldown-binding.wasm32-wasip1.wasm`                   |
| node loader               | `rolldown-binding.wasi.cjs`                         | `rolldown-binding.wasip1.cjs`                           |
| browser loader            | `rolldown-binding.wasi-browser.js`                  | `rolldown-binding.wasip1-browser.js`                    |
| deferred (workerd) loader | —                                                   | `rolldown-binding.wasip1-deferred.js`                   |
| worker scripts            | `wasi-worker.mjs`, `wasi-worker-browser.mjs`        | —                                                       |
| npm dir / package         | `npm/wasm32-wasi` → `@rolldown/binding-wasm32-wasi` | `npm/wasm32-wasip1` → `@rolldown/binding-wasm32-wasip1` |

Unshared memory growth detaches the previous JavaScript `ArrayBuffer`, so
emnapi must refresh TSFN atomic views after event-loop turns and NAPI result
DataViews after reentrant JavaScript calls. The pinned `emnapi@2.0.0-alpha.4`
release carries those refreshes and the `@emnapi/runtime` CJS entry the
generated CJS WASI loaders require, and supplies the per-flavor napi-rs link
archives. The browser package build bundles that
emnapi/wasm runtime into the published `workerd.mjs` and
`workerd.browser.mjs` entries. It aliases the deferred loader's bare `buffer`
import to the npm polyfill and bundles that implementation too; packed
validation rejects any remaining `buffer`, `node:buffer`, emnapi, or wasm
runtime import. Managed workerd consumers therefore do not depend on Node
compatibility flags.
`@rolldown/browser` nevertheless stays `private: true`, so `vp pm publish -r`
skips it. The reason is the prerelease ABI line recorded in
`scripts/wasi/check-workerd-packed-consumer.mjs`: the `.wasm` links the emnapi
`2.0.0-alpha` C archives and the manifest pins the matching `2.0.0-alpha`
runtimes, so publishing is gated on emnapi v2 going stable.
The same build
emits the threadless CJS/browser/deferred loaders plus a dedicated release
artifact containing the threaded CJS/browser/Node-worker/browser-worker graph.
`scripts/wasi/stage-wasi-packages.mjs` installs those bundles into both WASI
binding packages, copies each package's declaration from the matching generated
profile, and removes its now-vendored `buffer`/emnapi/wasm-runtime dependency
closure. The released CLI's pre-publish validator accepts either that fully
self-contained staged form or the complete generated external dependency set;
partial dependency sets and staged loaders with a remaining direct Buffer
import fail validation. For the threadless package, staging publishes the standalone managed
`workerd.browser.mjs` bundle at the generated `./workerd` target instead of the
raw deferred loader. The root keeps both optional packages for historical
threaded fallback compatibility; its generated `rolldown/workerd` facade
therefore forwards to the same managed factory.

Staging also owns package-directory recovery. A clean checkout bootstraps only
the missing napi-generated WASI package skeletons in an isolated directory,
while release staging preserves an already downloaded package-local Wasm binary
when the source-tree binary is unavailable. Each flavor is validated inside a
staging directory under the package root and then swapped into place by removing
the destination and renaming the staged copy over it; the staging directory is
always removed afterwards. Every lane runs staging exactly once, on a checkout it
owns, so there is no concurrent writer to serialize against and no later process
that could read recovery state back.

The managed workerd entry must register both the runnable task host and timer
host for every independently created instance, including callers of the root
instance factory rather than only the package convenience wrapper. Task-host
contract v4 is native-owned: JavaScript first reserves and validates an exact
registration capability via `reserveCurrentThreadHostRegistration()`, passes
its two words to `registerCurrentThreadTaskHost()`, and retains an exact
disposer that calls `unregisterCurrentThreadTaskHost()`. Runnable delivery
capabilities, fresh-turn scheduling, and failed-delivery recovery remain
entirely inside Rust and the native threadsafe-function callback. If
initialization fails and context destruction also fails, object errors with an
available `cause` retain cleanup there. Primitive errors, hostile objects, and
errors whose `cause` is already occupied preserve the primary synchronous
failure and surface a later asynchronous cleanup failure on a microtask.
Synchronous rollback failures are combined with cleanup errors in an aggregate
so the unrecoverable cleanup failure is not hidden.
Direct managed-call results whose accessor-backed `then` reads as a
non-function are returned synchronously after that single read. Nested managed
results use the same explicit settlement rules as callback results: a
non-function fulfills with the original identity, a callable getter result is
assimilated while the managed operation remains active, and a throwing getter
preserves its original error. Cycle checks include both the path of assimilated
thenables and the public promise returned to the caller. No identity-breaking
proxy is introduced.

The two WASI flavors have distinct artifact sets:

- threaded `wasm32-wasip1-threads`: `rolldown-binding.wasm32-wasi.wasm`,
  `.wasi.cjs`, `.wasi-browser.js`, and worker scripts
- single-thread `wasm32-wasip1`: `rolldown-binding.wasm32-wasip1.wasm`,
  `.wasip1.cjs`, `.wasip1-browser.js`, and `.wasip1-deferred.js`, without
  worker scripts

`packages/rolldown/build-binding.ts` snapshots the exact generated binding
surface before invoking napi-rs, including the root `browser.js` facade. A
failure in Rolldown's post-build patching, validation, or loader generation
restores every overwritten generated file and removes only files created by
that invocation. The root facade is managed explicitly rather than by a broad
JavaScript-file pattern, so unrelated sources remain outside the transaction.

`packages/rolldown/generate-workerd-loader.ts` deterministically hardens the
generated deferred loader after every napi build. The same generation pass
post-processes the napi-rs CJS and browser loaders for BOTH wasm flavors —
`wasm32-wasip1` and threaded `wasm32-wasip1-threads` — registering the v4
native CurrentThread runnable host and the JavaScript timer host before
exposing the binding: since the registry napi pin, every wasm artifact runs the
shared CurrentThread flavor, so a raw import of either shipped loader set must
carry its own task/timer hosts or a build never completes. Both loader sets
install that bootstrap unconditionally. The task-host bootstrap validates contract
version 4 and the exact reserved registration capability, captures that
capability for cleanup, and never exposes JavaScript drive or cancellation
functions. The CJS
bootstrap remains inside napi-rs's isolated-context initialization guard, so
registration failure unregisters any installed host, destroys the emnapi
context, and preserves cleanup diagnostics before the module load fails. The
transform uses explicit generated-block markers, is idempotent, validates all
loader anchors before writing any output, and fails when the expected napi-rs
shape changes; committed loaders must therefore be regenerated rather than
edited. The binding wrapper bypasses napi-rs's feature-blind declaration cache
for dedicated WASI targets and native `async-runtime` builds — each gets a fresh
type-def scratch folder, so no stale fragments survive — and restores the
inactive flavor's `rolldown-binding.*.d.cts` around every build. Only a build
whose target IS a WASI flavor regenerates that flavor's declaration; every other
build writes the committed bytes back, so a native build can rewrite neither.
`binding.d.cts` is the opposite: `--dts` is honoured on every build, so it keeps
whichever flavor built last. Regenerating declarations therefore has a required
order:

```text
build-binding:wasi        # threaded d.cts; repoints browser.js to -wasm32-wasi
build-binding:wasi-single # threadless d.cts
just build-rolldown       # native LAST: restores binding.d.cts and browser.js
```

The deferred loader's
`instantiate` export aliases its managed `createInstance` factory; no published
workerd entry returns raw binding exports or host controls.
The deferred declaration imports `rolldown-binding.wasip1.cjs`, which resolves
to `rolldown-binding.wasip1.d.cts` instead of the generic native declaration, so
its `exports` type follows the exact target feature set. Packed validation
compares the live binding export names against the bundled workerd declaration.
Public package entries do not register a second WASI task or timer host after
loading those generated artifacts; doing so would replace the generated
per-environment drivers. Native CurrentThread entries still install one
package-side v4 task host per environment. Its unreferenced native threadsafe
function supplies the fresh event-loop turn and environment cleanup retires the
registration; no JavaScript runnable callback or dispatch token exists. If
package-side timer-host setup fails after task-host registration, initialization
unregisters that exact task-host capability before propagating the failure.
The canonical `@rolldown/browser/workerd` package entry, the staged
threadless optional-package facade, and the generated `rolldown/workerd`
facade expose `createInstance` and its compatibility alias `instantiate`.
Both names register the CurrentThread timer host and the v4 #9977 native
runnable task host, expose per-instance memory diagnostics, and return an
idempotently disposable handle. Successful disposal unregisters the exact
task-host capability and synchronously clears and resolves every pending
JavaScript timer relay, so a destroyed N-API context cannot remain retained
until a long host deadline expires. Runnable drains are deferred through the
native host's fresh threadsafe-function turn; polling inline from a waker could
re-enter a future that still holds its waker lock. Exact delivery identity,
stale callback rejection, bounded replacement, and failure acknowledgement stay
inside the crate's registry and executor (see the Summary) rather than
crossing the JavaScript facade.
Timer cancellation contains host `clearTimeout` failures at the JavaScript
boundary and still resolves the Rust relay, because the callback enters through
a non-catching threadsafe function.
All package, managed, and generated timer hosts split delays above
`2_147_483_647` milliseconds into host-safe chunks. Initial or chained
`setTimeout` failures reject the relay; duplicate IDs, cancellation, and
managed disposal clear the active host timeout when possible and settle the
retired relay even when `clearTimeout` throws.
Each invocation owns independent N-API state, emnapi context, and
unshared memory. Callers must first close all binding objects so napi-rs can
complete task cancellation before environment teardown. Managed disposal then
explicitly unregisters both exact Rust hosts before asking emnapi to destroy
the context. This ordering does not depend on emnapi's LIFO cleanup queue
continuing after a throwing hook. It attempts both timer-host and task-host
cleanup even if one fails; successfully released hosts are forgotten, failed
host disposers remain retryable through a later `dispose()` call, and context
destruction does not begin until every host is evicted. Multiple host failures
are reported together.
If host registration fails before a handle can be returned, failed host
disposers are retried once immediately and persistent failures are aggregated
with the registration error.
The generated managed factory registers task and timer hosts against the raw
binding before constructing the public facade, removes all host-control
exports, and never publishes a raw-binding accessor. Package facades only
re-export that managed factory. The facade mediates binding objects nested in
plain records and arrays, inherited callbacks on class-based input records,
and binding objects delivered through caller callbacks, so retaining a plugin
context, output, event, constructor, prototype, bound constructor, or method
cannot call into a destroyed N-API environment. Constructors, prototypes, and
binding objects expose synchronized shadow targets for normal reflection,
expandos, derived constructors, and object-integrity operations without
retaining the raw N-API target after disposal. Facade construction registers the
full superclass prototype ancestry for both constructor canonicalization and
binding-object detection. A non-exported base constructor therefore cannot leak
inherited static methods around operation accounting, and close-bearing objects
returned by inherited factories enter the same disposal barrier as instances of
exported constructors. Their retained wrappers and methods reject after
disposal.
The imported Buffer constructor is injected into emnapi. Before classifying an
input as a record, the public facade uses the realm-neutral
`ArrayBuffer.isView` check plus captured intrinsic `ArrayBuffer` and
`SharedArrayBuffer` byte-length getters. Native or duplicate-bundle Buffer
values, typed-array subclasses, and foreign-realm or subclassed buffers
therefore retain strict identity even when workerd has no global `Buffer`.
Close-bearing binding objects increment a disposal barrier until `close()`
settles successfully, a rejected close reports `closed === true`, or their
wrapper is collected. The rejected-close rule distinguishes terminal cleanup
diagnostics, such as `BindingBundler.closeBundle` failures, from retryable
transport or teardown failures that leave `closed === false`. Wrapper
invalidation uses weak holder references plus `FinalizationRegistry`; repeated
builds therefore do not retain every raw target for the full managed-instance
lifetime. Each barrier token records the original raw `close` function. Only a
call to that exact function can release the token, and object/prototype proxies
reject assignment, definition, or deletion of `close`, so replacing it with a
no-op cannot bypass disposal.
Managed caller-provided memories are claimed once for the lifetime of the
memory object before emnapi instantiation begins. Failed initialization keeps
the claim because emnapi or Wasm import setup may already have mutated the
memory; only inputs rejected before memory validation leave it reusable.
For an extensible memory, an opaque monotonic claim operation is pinned directly
on that memory as a non-configurable symbol property. This survives buffer
replacement and prototype changes. A non-extensible memory cannot change its
prototype, so the operation instead uses its immediate prototype as the stable
host. Duplicate loaders, including loaders evaluated in distinct JavaScript
realms, use the global symbol registry and therefore still coordinate when they
share one memory object. Every claim invokes the discovered operation twice and
accepts only the `true`, then `false` transition; duplicates must remain
`false`, and non-monotonic or malformed preinstalled functions fail closed.
Prototype traversal is bounded and cycle-checked before either host is
accepted. Same-realm code that runs before every loader can preinstall a
semantically equivalent monotonic operation and is outside this lifecycle
coordination boundary; descriptor immutability prevents replacement after the
first legitimate installation.
Managed disposal commits the disposed state only after emnapi context
destruction succeeds. A thrown cleanup hook leaves the context and handle
available for a later retry. emnapi marks the context as stopping before it
drains cleanup hooks, so a thrown hook may leave partial teardown behind; the
pre-destroy explicit host eviction ensures the retryable handle cannot retain a
selected task or timer host. Context setup retries transient
`beforeExit` listener removal and listener-limit restoration failures before
aborting. It tracks listener occurrence counts rather than identities alone, so
an emnapi listener that reuses an existing function object is still removed
without removing the caller's prior occurrence. Eager Node loaders hand
successful context ownership from `beforeExit` to `exit` transactionally: they
register the replacement first, clear registration state only after physical
listener removal succeeds, and roll back a newly registered replacement if the
old owner cannot be removed. Failed bootstrap destruction, including an
asynchronous rejection, retains or rearms a cleanup listener so the context is
not abandoned. The supplied module promise is resolved and validated before a
managed context is created, so a pending or rejected module cannot retain
context state.
When best-effort initialization cleanup still fails, the loader always throws
an `AggregateError` that retains the primary error as `cause` and includes both
the primary and cleanup diagnostics; it never relies on a possibly stateful or
hostile `cause` accessor to retain cleanup information.
This ordering must remain aligned with napi-rs#3352's environment lifecycle as
that upstream API evolves.

The threadless loaders take their initial page count from
`napi.wasm.threadlessInitialMemory` in `packages/rolldown/package.json`, a
~64 MiB budget instead of the threaded flavor's inherited 16384 pages (1 GiB);
`napi.wasm.initialMemory` remains 16384 for the threaded flavor. The number is
not repeated in prose because it moves with every toolchain bump: the module's
own `env.memory` minimum is the floor, `generate-workerd-loader.ts` refuses a
configured value below it, and `scripts/wasi/check-wasi-threadless.mjs` caps
the configured value at the last measured budget, so a raise is a conscious
re-measurement rather than drift. When the ignored build artifact is present,
generation also fails if the configured maximum exceeds the binary's import
maximum; bounds always fail above memory32. Native builds also run the
post-generator from clean checkouts, so they skip binary inspection when no
threadless Wasm has been built. The focused unit and packed-consumer gates
repeat a 256-module graph three times, require the declared floor, and fail if
the representative build crosses 128 MiB. The threadless static check instantiates
the built module at the configured floor rather than trusting generated source,
so the actual threadless build remains fail closed. These are
local address-space regression gates; production committed-memory validation
still requires Workers platform telemetry.

Build and dev objects memoize their close sequence so concurrent or repeated
callers observe the same teardown result.
Watch close uses the same single-flight contract and attempts every
parallel-plugin worker teardown plus binding close before reporting cleanup
errors. Its public close function is installed before asynchronous watcher
setup, so same-tick close waits for initialization, prevents the deferred run,
and emits the close event exactly once.

The browser package declares explicit `workerd` export conditions for the
managed loader and compiled Wasm module. The stable compiled-module specifier
ends in `.wasm` so Wrangler classifies the import before package export
resolution. Wrangler consumers must apply a `CompiledWasm` module rule; the
public guide includes the minimal rule. The package-root browser and default
loaders are post-bundled with their emnapi/wasm runtime dependencies, as are
the managed workerd entries. Release assembly reuses those hardened loader
bundles for both standalone WASI binding flavors, including both threaded
worker entry points. Published browser, standalone-flavor, and root-facade
consumers therefore do not depend on any repository-local build tooling or
resolve registry emnapi at runtime.

---

## 13. Committed WASI loaders and codegen checks

`packages/rolldown/src` commits BOTH flavors' loader sets side by side under
their per-flavor names (plus `browser.js`, which re-exports the single-thread
binding package — the browser story). Distinct names are what make the rest of
this section possible: no restore step, no drift-allowlist arm, and no
build-order coupling is needed to keep one flavor from overwriting the other.

- The per-flavor naming and loader codegen (napi-rs#3353) ship in the released
  `@napi-rs/cli`, pinned to `^3.9.0` in the workspace catalog. A build whose
  target is NOT wasi regenerates
  EVERY declared wasi flavor's loader set, each with `hasThreads` derived from
  its own triple, so loader regeneration is deterministic and byte-identical to
  the committed copies on every host and under every build variant. A wasi
  build regenerates only the flavor being built, and it alone owns that
  flavor's export list: a non-wasi build re-derives each flavor's loaders from
  the `// napi-rs-artifact-metadata:` header of the COMMITTED
  `rolldown-binding.<flavor>.cjs` (and re-writes `.<flavor>.d.cts` verbatim),
  never from the native type-def. So a merge that takes upstream's copy of a
  committed wasi loader silently drops this branch's exports from the next
  native build (`assertAsyncRuntimeHostExports` fails) until that flavor's
  wasi build runs again. Loaders need no restore step;
  CI's "Check no diff" in `reusable-native-build.yml` has full coverage of all
  committed loaders. Declarations do: `binding.d.cts` keeps whichever flavor
  built last, so the native build must run last — which is why ci.yml restores
  it after `just build-browser`.
- `packages/rolldown/package.json` declares `napi.rootPublisher: "pnpm"`
  (honored from `@napi-rs/cli` 3.8.5, per napi-rs#3450). Pre-publish
  validation is publisher-aware: because the manifest also declares
  `publishConfig.exports` — the map pnpm substitutes for `exports` while
  packing — the validator checks that publish-effective map and generates the
  `./workerd` / `./wasm` / `./wasm.wasm` facade subpaths inside it, leaving the
  raw dev `exports` map (whose `dev: ./src/*.ts` conditions are never packed)
  untouched. Without the declaration the validator would judge the raw dev map
  and reject those unpacked `./src/*.ts` targets.
- The Node Validation job in `ci.yml` asserts a drift allowlist after
  `just build-browser`; the allowlist is `binding.d.cts` alone.
- The threadless-ness of the single-thread loaders is guarded by
  `scripts/wasi/check-wasi-threadless.mjs` in the WASI workflow (it inspects
  the committed/regenerated `rolldown-binding.wasip1.*` loaders); a wrong
  `hasThreads` resolution now additionally misnames the output, so imports
  fail loudly instead of silently swapping flavors.
- Immediately after the threaded build, the WASI workflow runs the dedicated
  `test:wasi-threaded` Node profile against the still-wired threaded dist. The
  profile executes concurrent builds without
  collecting managed-workerd tests that require the threadless Wasm file or
  child-process `--input-type` probes that file-based worker entrypoints cannot
  inherit.
- `packages/workerd-tests/memory.mjs` repeatedly creates concurrent
  managed instances, verifies memory isolation and idempotent disposal, and
  emits local RSS/address-space samples. Production committed-memory
  validation still requires Workers DevTools and platform metrics.
- `scripts/wasi/check-wasi-binding-packed-consumer.mjs` imports the published
  threadless package root through both its CJS and browser conditions, executes
  an async binding build, and requires the generated task/timer bootstrap to
  report `timers: true`. The threaded Chromium exercise raw-imports the packed
  `rolldown-binding.wasi-browser.js` the same way and completes a build purely
  on the loader's own gated bootstrap — no package entry ever runs there — so
  it pins the same `timers: true` contract for the threaded flavor. Its
  managed-workerd pass also checks runtime export names against the bundled
  declaration and repeats the representative memory lifecycle described above.

`napi artifacts` routes each flavor's wasm + generated loaders into its own npm
dir (`npm/wasm32-wasi`, `npm/wasm32-wasip1`) by exact-name match. Release
assembly then overwrites the threadless flavor's three runtime-bearing loaders
and the threaded flavor's CJS/browser/two-worker graph with bundled outputs,
then validates both packed packages outside the workspace. The packed
consumers assert the exact threaded and threadless capability reports as well
as executing builds, so stale or cross-wired Wasm artifacts fail even when
their loader graph still initializes. The threaded browser package is served
with COOP/COEP isolation and exercised in Chromium; the check observes actual
Worker construction and script fetch, a shared WebAssembly memory and Wasm
fetch, a completed build, and successful binding close without worker/page
errors. The publish-stage browser and root packages are assembled with their
downloaded `dist` directories. The root package is installed under pnpm and
npm, then executed through `rolldown` and `rolldown/experimental` with the
threaded and threadless optional packages isolated into separate layouts. Its
managed workerd facade is also executed in the threadless layout. This prevents
committed raw loader copies, notice-only packages, fake root facades, or
workspace-only pnpm patches from defining a published WASI runtime.

---

## 14. Metrics and baseline

The one committed measurement of this change is
[benchmarks.md](./benchmarks.md): the Tokio-async + Tokio-blocking + Rayon
thread population collapses to a single shared pool (56 → 25 peak threads on
the measured host), with wall-time, instruction, RSS, and context-switch
comparisons across four fixtures. That file states its own limits — it
measures a tokio side that can no longer be built, and it predates the
scheduler hardening — so treat it as the frozen baseline, not a claim about
head.

---

## Invariants (each tied to a file)

- **No tokio in the shipped graph** — `Justfile::check-no-tokio`;
  `rolldown_binding/Cargo.toml` depends on napi's pluggable SPI, not
  `napi/async` (Principle 9).
- **Shared runtime on every shipped target** — `install_async_runtime_backend`
  (`#[module_init]`, compiled unconditionally).
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
- **No JavaScript runtime-ownership protocol on any target** — the N-API
  environment lifecycle owns the runtime; `lib.rs` exports no lease surface.

## Related

- [design.md](./design.md) — the principles and trade-offs behind this
- [benchmarks.md](./benchmarks.md) — the frozen tokio-vs-shared measurements
  (§14)
- [wasi-flavor-design.md](./wasi-flavor-design.md) — why two WASI artifacts
  exist instead of one runtime-switchable binary (§12)
- [bundler-data-lifecycle](../bundler-data-lifecycle/implementation.md) —
  deferred drops and rebuild ownership (§6)
- [watch-mode](../watch-mode/implementation.md) — the `sleep_until` debounce
  consumer (§5)
