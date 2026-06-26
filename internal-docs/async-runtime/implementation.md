# Async Runtime - Implementation

> The rationale and principles behind this live in [design.md](./design.md).

## Summary

The `async-runtime` Cargo feature installs a Rolldown scheduler into napi-rs,
routes Rolldown task creation through `rolldown_utils::futures`, and builds the
browser artifact for `wasm32-wasip1`. The `tokio-runtime` feature remains the
default.

## Components

### napi-rs runtime registration

The sibling napi-rs checkout adds the `async-runtime` feature and
`AsyncRuntime` registration interface in `crates/napi/src/tokio_runtime.rs`.
When the feature is enabled, registered-runtime execution takes precedence even
if another dependency enables `tokio_rt` through Cargo feature unification.
This is required because OXC's NAPI crates enable napi-rs async support.

Promise resolution and panic rejection remain owned by napi-rs. Runtime start,
shutdown, entry, spawn, and block-on operations delegate to the registered
implementation.

### Rolldown scheduler

`crates/rolldown_utils/src/async_runtime.rs` owns the lazy global controller.

- `CurrentThreadExecutor` uses a reentrancy-safe FIFO runnable queue. Wakes drain
  cooperatively on the calling thread. Blocking work executes inline.
- `MultiThreadExecutor` schedules bounded queue-drain jobs on a custom Rayon
  pool. The same pool is inherited by nested `par_iter` calls.
- A second FIFO holds blocking closures. `active_blocking` limits how many
  Rayon workers may block at once.
- `JoinHandle` normalizes async-task, blocking-job, and immediate results.
- Atomic metrics expose task, poll, queue-depth, active-worker, panic, and
  blocking-concurrency counters.

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

`rolldown_utils::futures` is the compatibility facade. The following work no
longer calls Tokio or `std::thread` directly under the new feature:

- module-loader tasks
- blocking source reads
- asset/copy plugin reads
- dev and watch coordinator tasks
- the native-magic-string sourcemap consumer
- binding close/flush blocking work

The sourcemap consumer is disabled for current-thread mode because a blocking
channel receiver cannot make progress on the same cooperative thread. The
existing inline sourcemap path remains active.

### Non-threaded WASI

The browser build uses:

```text
wasm32-wasip1
--no-default-features
--features async-runtime
```

The napi-rs CLI changes from napi-rs#3353 link `libemnapi-basic.a`, emit
unshared `WebAssembly.Memory`, set `asyncWorkPoolSize: 0`, and omit Worker
imports and factories. `packages/rolldown` keeps the threaded WASI scripts and
adds `build-binding:wasi-single`; browser-package scripts select the
single-thread variant. Until those napi-rs CLI changes are published, the
single-thread build loads the pnpm-patched CLI source from the installed
package; other build variants use the normal package entry.

Unshared memory growth detaches the previous JavaScript `ArrayBuffer`. The
emnapi fix in emnapi#220 refreshes TSFN atomic views after event-loop turns and
refreshes NAPI result DataViews after reentrant JavaScript calls. Rolldown
applies the equivalent published-package workaround through
`patches/@emnapi__core@1.11.1.patch`.

## Metrics And Baseline

Superseded: committed, reproducible measurements now live in
[benchmarks.md](./benchmarks.md) (harness:
`scripts/misc/bench-async-runtime/`). They confirm the earlier illustrative
observation — the Tokio-async + Tokio-blocking + Rayon thread population
collapses to a single shared pool (56 → 25 peak threads on the measured host)
— and add wall-time, instruction, RSS, and context-switch comparisons across
four fixtures, plus the blocking-cap A/B that validated keeping the
`max_blocking_tasks = worker_threads` default.

## Related

- [benchmarks.md](./benchmarks.md) - committed tokio-vs-shared measurements
- [design.md](./design.md) - goals and trade-offs
- [bundler-data-lifecycle](../bundler-data-lifecycle/implementation.md) -
  deferred-drop interaction with Rayon
