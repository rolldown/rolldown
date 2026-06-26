# Async Runtime - Design & Principles

## Summary

Rolldown's optional async runtime uses one scheduling domain for async task
polling, CPU parallelism, and blocking filesystem work. It supports a
cooperative current-thread flavor for hosts without threads and a work-stealing
multi-thread flavor for native builds. The existing Tokio runtime remains the
default and is selected by the `tokio-runtime` feature; the new implementation
is selected by `async-runtime`. See [implementation.md](./implementation.md)
for the component map.

## Design Principles

1. **Thread availability is a build/runtime property, not an assumption.**
   `wasm32-wasip1` uses the current-thread flavor and must not import shared
   memory, construct workers, park with `Atomics.wait`, or call
   `std::thread::spawn`. Native builds default to the multi-thread flavor.

2. **CPU and async work share a pool.** Module-task futures run on the same
   Rayon pool used by link and generate stages. Nested Rayon work therefore
   uses the current pool instead of creating a second CPU pool.

3. **Blocking I/O is bounded without another pool.** Blocking jobs are queued
   alongside runnable futures. At most `max_blocking_tasks` workers may execute
   them concurrently. The default cap equals the worker count because it limits
   concurrency within the same fixed pool; it does not create more threads.

4. **Wakeups are batched.** A future wake enqueues a runnable. At most one
   bounded drain loop per worker is submitted to Rayon, and each loop processes
   multiple runnables. Submitting every wake as an individual Rayon job caused
   excessive context switches on large module graphs.

5. **Configuration is immutable after first use.** Runtime flavor, worker
   count, and blocking concurrency may be configured from the binding API or
   environment before the first async call. Lazy startup makes top-level
   configuration possible without changing module registration order.

6. **The compatibility path does not change.** Builds without
   `async-runtime` retain napi-rs's Tokio executor and Rolldown's previous
   behavior.

## Background

- [#6270](https://github.com/rolldown/rolldown/pull/6270) moved filesystem reads
  to Tokio's blocking pool and established that bounded concurrent reads are
  materially faster than blocking async workers.
- [#6272](https://github.com/rolldown/rolldown/pull/6272) increased Tokio worker
  count because CPU-heavy module tasks run inside async tasks.
- [#9086](https://github.com/rolldown/rolldown/pull/9086) exposed the resulting
  oversubscription when many Rolldown processes run concurrently.
- [#9942](https://github.com/rolldown/rolldown/pull/9942) demonstrated why
  thread parking cannot be part of a browser-main-thread path.

The new runtime treats these as one scheduling problem rather than independent
Tokio, Tokio-blocking, Rayon, and ad-hoc thread-pool tuning problems.

## Unresolved Questions

- Watch/dev mode still uses Tokio's timer API. The custom runtime supports its
  task and channel usage, but timer-driven watch coordination needs a
  runtime-independent timer before current-thread watch mode can be supported.
- The single-thread browser build currently uses the same package artifact name
  as the threaded WASI build, so release jobs must build them in separate
  package pipelines.

## Related

- [implementation.md](./implementation.md) - the scheduler and integration map
- [bundler-data-lifecycle](../bundler-data-lifecycle/implementation.md) -
  deferred drops that also run on the shared Rayon pool
