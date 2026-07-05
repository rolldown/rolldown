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
Generated JavaScript-facing futures and `#[napi(async_runtime)]` entry points
use the registered implementation whenever `async-runtime` is enabled, even if
another dependency also enables `tokio_rt` through Cargo feature unification.
Rolldown has this combined feature profile because OXC's NAPI crates enable
napi-rs's Tokio support.

In that combined profile, napi-rs's established public free `spawn`,
`spawn_blocking`, `block_on`, and runtime-entry helpers retain their Tokio API
and behavior. Only a pure `async-runtime` build routes those free helpers
through the registered implementation. Rolldown's own task creation uses
`rolldown_utils::futures`, so it still reaches the shared scheduler directly;
arbitrary transitive calls to napi-rs's free helpers must not be assumed to use
Rolldown's scheduler or bounded blocking lane.

Promise resolution, panic rejection, and cancellation handles remain owned by
napi-rs. `AsyncRuntime::spawn` transfers an opaque `AsyncRuntimeTask` and
Rolldown returns it untouched when the controller is stopped. The optional
`AsyncRuntime::spawn_blocking` hook follows the same ownership rule for its
closure. Accepted tasks and closures retain napi-rs's cancel-on-drop guards.
Generated-task submission and custom runtime lifecycle operations delegate to
the registered implementation; `start` and `shutdown` report failures through
`napi::Result`.

### Rolldown scheduler

`crates/rolldown_utils/src/async_runtime.rs` owns the lazy global controller.

- The controller lifecycle is `Initial -> Running -> Stopping -> Stopped`.
  Initial work may lazily create the backend. napi invokes `start` during addon
  registration, so the initial `start` leaves `Initial` unchanged to preserve
  the documented pre-first-async-call configuration window. Shutdown changes
  `Running` to `Stopping` under the controller mutex before closing the
  generation. Submissions in `Stopping` or `Stopped` return their task or
  closure untouched. An explicit restart waits while `Stopping`, then creates
  the next `Running` generation. Configuration remains frozen after shutdown.
  Start, shutdown, and submission use the same mutex, so a racing submission
  cannot recreate the backend after shutdown.
- Each backend owns a generation work registry. Async tasks register an abort
  handle and all accepted operations register a retirement guard while the
  controller mutex is held. Shutdown atomically closes that registry, aborts
  accepted async work, closes and drains the queued blocking FIFO, and waits
  for every guard to retire. Async-task scheduler closures and heap sleeps hold
  weak executor references, so completed or cancelled work cannot keep an old
  pool alive accidentally.
- `CurrentThreadExecutor` uses a reentrancy-safe FIFO runnable queue. Wakes drain
  cooperatively on the calling thread. Blocking work executes inline.
- `MultiThreadExecutor` schedules bounded queue-drain jobs on a custom Rayon
  pool. The same pool is inherited by nested `par_iter` calls. Rayon worker
  start hooks classify every nested worker for cooperative `block_on`; a
  separate driver marker limits the per-worker LIFO slot to scheduler frames
  that will actually drain it.
- A second FIFO holds blocking closures. `active_blocking` limits how many
  Rayon workers may block at once. Validation reserves one worker from
  blocking admission. MultiThread promotes a requested worker count of one to
  an effective count of two, then clamps `max_blocking_tasks` to
  `worker_threads - 1`. The Rayon pool creates exactly the effective configured
  count; configuration and metrics therefore report physical workers, with no
  hidden reserve.
- Drain and cooperative loops force a blocking turn after 16 consecutive
  runnable polls when the blocking FIFO has capacity. The timer timekeeper uses
  the runnable-only path, so a stalled blocking closure cannot stop timer
  service. Parked-driver registration records whether a parker may consume
  blocking work; blocking submissions skip the runnable-only timekeeper and
  wake a cooperative driver or arm a normal drainer.
- `JoinHandle` normalizes async-task, blocking-job, and immediate results and
  detaches async tasks on drop to match Tokio. Scheduler shutdown instead
  aborts accepted async tasks and resolves retained handles with `JoinError`.
- MultiThread shutdown waits in three stages: accepted work retirement,
  drainer/timekeeper exit, then physical Rayon worker exit. Only after all
  three stages does the controller publish `Stopped` and wake a waiting
  `start`. A lifecycle call made from a task poll, blocking closure, or Rayon
  worker of the generation being stopped returns an error rather than waiting
  on itself. Queued blocking closures are dropped one at a time behind
  `catch_unwind`; a submission that races queue closure is rejected and dropped
  with the same isolation outside the queue lock. Shutdown timer wakes are
  isolated too, so user-owned `Drop`/`RawWaker` panics cannot strand the
  lifecycle transition.
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

On shared WebAssembly builds, the resolver always reports and configures
`CurrentThread`. `ROLLDOWN_RUNTIME=multi` is accepted as an inherited
environment value but normalized before the module-init `configure` call;
otherwise loading a threadless WASI artifact would panic while registering the
addon.

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

Native plugin-context logging uses `spawn_detached`: log callbacks are
fire-and-forget by contract and must survive dropping the local spawn handle.
The Vite import-glob plugin pre-resolves bare glob patterns asynchronously
before its synchronous AST rewrite. Results are grouped per
`import.meta.glob` call, so an unresolved entry that aborts one array cannot
shift unused resolutions into a later call.

The native-magic-string sourcemap consumer deliberately uses one dedicated OS
thread in modes where threads are supported. It cannot occupy the bounded
blocking lane: its long-lived channel receive loop would monopolize the entire
blocking allowance of the smallest MultiThread configuration and delay
unrelated blocking work. The consumer is disabled for current-thread mode,
where the existing inline sourcemap path remains active.

Module-loader execution and its supervisor are submitted as one scheduler
task. Normal completion still arrives through `ModuleLoaderMsg`; a task panic
is caught inside that task, while scheduler cancellation or rejected submission
drops the same supervised future. Each failure path emits exactly one
`BuildErrors`, which retires the loader's `remaining` count instead of leaving
the build pending forever.

### Deferred destruction

`crates/rolldown/src/utils/defer_drop.rs` owns one process-global serial
maintenance worker. Heavy link-stage values are sent there after generation,
and every build entry calls `drain()` before starting new scan/link/render work.
The worker is deliberately outside Rayon: a rebuild may call `drain()` from a
pool worker while every other execution lane is unavailable, so inheriting
that Rayon registry could deadlock the build against its own maintenance
queue.

### Timers and native watch mode

`rolldown_utils::time::sleep_until` routes watcher debounce timers to Tokio on
the default build and to the shared runtime otherwise. `MultiThreadExecutor`
uses an executor-owned timer heap and timekeeper role. `CurrentThreadExecutor`
uses the host `TimerDriver` registered by `packages/rolldown/src/timer-host.ts`,
which delegates to paired `setTimeout`/`clearTimeout` callbacks in each
importing environment. The Rust relay records whether the JS schedule callback
has returned before sending cancellation, preventing cancel from overtaking
timeout creation. Cancellation clears the timeout and resolves the schedule
Promise so the detached relay task retires immediately.
MultiThread timer wakes, including shutdown drain-fire, are individually
wrapped with `catch_unwind`; a user-supplied `RawWaker` cannot unwind the
timekeeper or strand shutdown.

Native watch mode is supported on both runtime flavors. Binding dev mode is
still skipped on CurrentThread, and WASI watch remains unsupported because it
stalls during the initial build before debounce timers are involved.

### Threaded WASI runtime ownership

The binding starts with one implicit threaded-WASI runtime owner. Its
`startAsyncRuntime` and `shutdownAsyncRuntime` exports update a mutex-serialized
native owner count and return napi errors from the fallible napi-rs lifecycle
APIs. Only the `0 -> 1` transition starts the runtime, and only the `1 -> 0`
transition shuts it down. A failed start leaves the count at zero; a failed
shutdown retains the last owner so a later release can retry. Releasing at
zero is idempotent, and concurrent releases cannot underflow the count.

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
lane, exact two-thread minimum, accepted-work cancellation tracking,
generation-quiescent shutdown, and dedicated deferred-drop worker;
[benchmarks.md](./benchmarks.md) records them as historical evidence and calls
out the required re-measurement.

## Related

- [benchmarks.md](./benchmarks.md) - committed tokio-vs-shared measurements
- [design.md](./design.md) - goals and trade-offs
- [bundler-data-lifecycle](../bundler-data-lifecycle/implementation.md) -
  deferred-drop interaction with rebuild ownership
