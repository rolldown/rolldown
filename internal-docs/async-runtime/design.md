# Async Runtime - Design & Principles

## Summary

Rolldown's optional async runtime uses one scheduling domain for async task
polling, CPU parallelism, and bounded blocking filesystem work. Heavy
post-build destruction uses a separate serial maintenance worker so a rebuild
can never wait on a drop queued behind itself in the shared pool. The runtime
supports a cooperative current-thread flavor for hosts without threads and a
work-stealing multi-thread flavor for native builds. The existing Tokio runtime
remains the default and is selected by the `tokio-runtime` feature; the new
implementation is selected by `async-runtime`. See
[implementation.md](./implementation.md) for the component map.

## Design Principles

1. **Thread availability is a build/runtime property, not an assumption.**
   `wasm32-wasip1` uses the current-thread flavor and must not import shared
   memory, construct workers, park with `Atomics.wait`, or call
   `std::thread::spawn`. Native builds default to the multi-thread flavor.

2. **CPU and async work share a pool.** Module-task futures run on the same
   Rayon pool used by link and generate stages. Nested Rayon work therefore
   uses the current pool instead of creating a second CPU pool.

3. **Blocking I/O cannot consume every execution lane.** Blocking jobs are
   queued alongside runnable futures, but multi-thread validation reserves one
   worker from blocking admission. The default cap is therefore
   `max(worker_threads - 1, 1)`. A configured one-worker runtime creates one
   internal reserve worker so its single admitted blocking closure cannot
   freeze async progress.

4. **Work classes receive bounded service.** Runnable locality remains the
   normal priority, but a continuously hot runnable stream yields to the
   blocking FIFO after a fixed quantum. The timer timekeeper drains runnables
   only; it never enters a potentially unbounded blocking closure.

5. **Wakeups are batched.** A future wake enqueues a runnable. At most one
   bounded drain loop per worker is submitted to Rayon, and each loop processes
   multiple runnables. Submitting every wake as an individual Rayon job caused
   excessive context switches on large module graphs.

6. **Configuration is immutable after first use.** Runtime flavor, worker
   count, and blocking concurrency may be configured from the binding API or
   environment before the first async call. Lazy startup makes top-level
   configuration possible without changing module registration order.

7. **Detached-task behavior matches Tokio.** Dropping Rolldown's `JoinHandle`
   detaches rather than cancels the task. Internal module-loader tasks are
   supervised so panic or cancellation is converted into a build diagnostic
   and completion accounting cannot hang.

8. **The compatibility path does not change.** Builds without
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

## Implemented Follow-ups

- Watch debounce uses a runtime-independent `sleep_until` facade. Multi-thread
  mode owns a timer heap; current-thread mode delegates to the host event loop.
  Native watch mode therefore works on both flavors. Binding dev mode remains
  unsupported on current-thread, and WASI watch still stalls during the initial
  build.
- Threaded and single-thread WASI builds use distinct artifact names. The
  threaded build retains the `wasi` loader/wasm names and worker scripts; the
  single-thread build uses `wasip1` names, includes the deferred workerd loader,
  and ships no worker scripts.

## Related

- [implementation.md](./implementation.md) - the scheduler and integration map
- [bundler-data-lifecycle](../bundler-data-lifecycle/implementation.md) -
  deferred drops and rebuild ownership
