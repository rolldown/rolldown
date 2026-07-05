# Async Runtime - Implementation

> The rationale and principles behind this live in [design.md](./design.md).

## Summary

The `async-runtime` Cargo feature installs a Rolldown scheduler into napi-rs,
and routes Rolldown task creation through `rolldown_utils::futures`. The
`tokio-runtime` feature remains the default.

## Components

### napi-rs runtime registration

The sibling napi-rs checkout adds the `async-runtime` feature and
`AsyncRuntime` registration interface in `crates/napi/src/tokio_runtime.rs`.
When the feature is enabled, registered-runtime execution takes precedence even
if another dependency enables `tokio_rt` through Cargo feature unification.
This is required because OXC's NAPI crates enable napi-rs async support.

Promise resolution and panic rejection remain owned by napi-rs. Runtime start,
shutdown, entry, spawn, block-on, and blocking-work operations delegate to the
registered implementation. The optional `AsyncRuntime::spawn_blocking` hook is
implemented so napi and transitive callers use Rolldown's bounded blocking lane
instead of napi's dedicated-thread fallback.

### Rolldown scheduler

`crates/rolldown_utils/src/async_runtime.rs` owns the lazy global controller.

- `CurrentThreadExecutor` uses a reentrancy-safe FIFO runnable queue. Wakes drain
  cooperatively on the calling thread. Blocking work executes inline.
- `MultiThreadExecutor` schedules bounded queue-drain jobs on a custom Rayon
  pool. The same pool is inherited by nested `par_iter` calls. Rayon worker
  start hooks classify every nested worker for cooperative `block_on`; a
  separate driver marker limits the per-worker LIFO slot to scheduler frames
  that will actually drain it.
- A second FIFO holds blocking closures. `active_blocking` limits how many
  Rayon workers may block at once. Validation reserves one worker from
  blocking admission. A one-worker configuration creates one internal reserve
  worker, while configurations with two or more workers reserve capacity by
  clamping `max_blocking_tasks` to `worker_threads - 1`.
- Drain and cooperative loops force a blocking turn after 16 consecutive
  runnable polls when the blocking FIFO has capacity. The timer timekeeper uses
  the runnable-only path, so a stalled blocking closure cannot stop timer
  service. Parked-driver registration records whether a parker may consume
  blocking work; blocking submissions skip the runnable-only timekeeper and
  wake a cooperative driver or arm a normal drainer.
- `JoinHandle` normalizes async-task, blocking-job, and immediate results and
  detaches async tasks on drop to match Tokio.
- Atomic metrics expose task, poll, queue-depth, active-worker, panic, and
  blocking-concurrency counters. Reset clears cumulative event counters only;
  live gauges and lifetime high-water marks remain intact because active guards
  may still need to decrement them. A reset generation is part of the
  deadlock-detector fingerprint, preventing repeated counter values across a
  reset from being mistaken for no progress.

The binding adapter and JS-facing configuration live in
`crates/rolldown_binding/src/async_runtime.rs`. Configuration sources are:

- `ROLLDOWN_RUNTIME=single|current-thread|multi|multi-thread`
- `ROLLDOWN_WORKER_THREADS`
- `ROLLDOWN_MAX_BLOCKING_THREADS` (retained as the compatibility environment
  variable name; it now caps jobs within the fixed pool)
- `configureAsyncRuntime({ flavor, workerThreads, maxBlockingTasks })`, exported
  from `rolldown/experimental`

Configuration must happen before the first async binding call.

This API is feature-gated. `configureAsyncRuntime`, `getAsyncRuntimeConfig`, and
`getAsyncRuntimeMetrics` are exported on every build, but only the
`async-runtime` build honors them. On the default `tokio-runtime` build
`configureAsyncRuntime` throws a feature-disabled error (built without the
`async-runtime` feature), `getAsyncRuntimeConfig` reports values derived from the
environment variables and built-in defaults, and `getAsyncRuntimeMetrics` always
returns zeroed counters.

### Routed work

`rolldown_utils::futures` is the compatibility facade. The following work is
routed through the selected runtime:

- module-loader tasks
- blocking source reads
- asset/copy plugin reads
- dev and watch coordinator tasks
- binding close/flush blocking work

The native-magic-string sourcemap consumer deliberately uses one dedicated OS
thread in modes where threads are supported. It cannot occupy the bounded
blocking lane: its long-lived channel receive loop would monopolize the entire
blocking allowance of a one-worker configuration and delay unrelated blocking
work. The consumer is disabled for current-thread mode, where the existing
inline sourcemap path remains active.

Module-loader tasks are spawned with a supervisor. Normal completion still
arrives through `ModuleLoaderMsg`; a task panic or cancellation is converted
into `BuildErrors`, which retires the loader's `remaining` count instead of
leaving the build pending forever.

### Deferred destruction

`crates/rolldown/src/utils/defer_drop.rs` owns one process-global serial
maintenance worker. Heavy link-stage values are sent there after generation,
and every build entry calls `drain()` before starting new scan/link/render work.
The worker is deliberately outside Rayon: a one-worker rebuild may call
`drain()` on the same pool worker that queued the previous drop, so inheriting
that Rayon registry would deadlock the worker against its own queue.

### Timers and native watch mode

`rolldown_utils::time::sleep_until` routes watcher debounce timers to Tokio on
the default build and to the shared runtime otherwise. `MultiThreadExecutor`
uses an executor-owned timer heap and timekeeper role. `CurrentThreadExecutor`
uses the host `TimerDriver` registered by `packages/rolldown/src/timer-host.ts`,
which delegates to `setTimeout` in each importing environment.

Native watch mode is supported on both runtime flavors. Binding dev mode is
still skipped on CurrentThread, and WASI watch remains unsupported because it
stalls during the initial build before debounce timers are involved.

### Non-threaded WASI

The current-thread executor is the runtime half of the non-threaded
`wasm32-wasip1` build. Packaging, generated loaders, and the emnapi
memory-growth backport are handled in the dependent browser/WASI change.

The two WASI flavors have distinct artifact sets:

- threaded `wasm32-wasip1-threads`: `rolldown-binding.wasm32-wasi.wasm`,
  `.wasi.cjs`, `.wasi-browser.js`, and worker scripts
- single-thread `wasm32-wasip1`: `rolldown-binding.wasm32-wasip1.wasm`,
  `.wasip1.cjs`, `.wasip1-browser.js`, and `.wasip1-deferred.js`, without
  worker scripts

## Metrics And Baseline

Superseded: committed, reproducible measurements now live in
[benchmarks.md](./benchmarks.md) (harness:
`scripts/misc/bench-async-runtime/`). They confirm the earlier illustrative
observation — the Tokio-async + Tokio-blocking + Rayon thread population
collapses to a single shared pool (56 → 25 peak threads on the measured host)
— and add wall-time, instruction, RSS, and context-switch comparisons across
four fixtures. Those measurements predate the production-hardening reserve
lane and dedicated deferred-drop worker; [benchmarks.md](./benchmarks.md)
records them as historical evidence and calls out the required re-measurement.

## Related

- [benchmarks.md](./benchmarks.md) - committed tokio-vs-shared measurements
- [design.md](./design.md) - goals and trade-offs
- [bundler-data-lifecycle](../bundler-data-lifecycle/implementation.md) -
  deferred-drop interaction with rebuild ownership
