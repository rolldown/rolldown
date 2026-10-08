# Async Runtime — Design & Principles

## Summary

Rolldown runs async task polling, CPU parallelism and bounded blocking I/O in
one scheduling domain. On MultiThread, futures are polled on the executor's own
Rayon workers, so `par_iter` splits stay in that pool.

The scheduler is the external
[`napi-async-runtime`](https://crates.io/crates/napi-async-runtime) crate; its
README documents the executor internals. This doc records why rolldown uses it
the way it does. Heavy post-build destruction runs on a separate serial
maintenance thread, so a rebuild never waits on a drop queued behind itself in
the shared pool. Every shipped artifact compiles this runtime, so no production
build contains tokio.

The machinery: [implementation.md](./implementation.md).

## Design Principles

1. **Threads follow the artifact.** Native runs only MultiThread and
   threadless `wasm32-wasip1` only CurrentThread; both ignore
   `ROLLDOWN_RUNTIME`. `wasm32-wasip1-threads` defaults to MultiThread, and
   `ROLLDOWN_RUNTIME=single` selects CurrentThread there. Threadless wasm must
   not import shared memory, construct workers, park with `Atomics.wait` or
   spawn threads.

   No CurrentThread artifact runs timers. The watch debounce is the only
   `sleep_until` caller, and watch runs only on native, which is MultiThread.
   So the binding answers the cli loaders' `registerTimerHost` without a timer
   driver, and a CurrentThread `sleep_until` panics loudly instead of hanging.

   On `wasm32-wasip1-threads` the default is 2 workers (clamped to [2, 4]).
   Every thread allocates through wasi-libc dlmalloc, whose global lock spins
   on `sched_yield`, so 3, 4 or 8 workers burn more CPU on that lock and build
   slower than 2. MultiThread there also needs the `parking_lot_core` patch
   and napi's heap-sync allocator lock
   ([napi-rs `wasi-heap-sync-design.md`](https://github.com/napi-rs/napi-rs/blob/main/cli/docs/wasi-heap-sync-design.md)).

   The capability contract follows the flavor in effect: dev mode needs
   MultiThread, and watch is unsupported on every WASI artifact. `watch()`
   throws `UnsupportedRuntimeFeatureError` synchronously and `dev()` rejects
   with it, both before any callback, plugin hook, watcher or runtime setup
   exists, so there is nothing to tear down. Callers check
   `getRuntimeSupport()` first or catch the error.

   On threaded WASI under Node the loader preloads one pool Worker per
   MultiThread worker at load, taking the Worker boot off the first build. The
   cost is a few ms more load time and about 20 MB more peak RSS in a process
   that only imports rolldown. Upstream (cli loader + emnapi) owns it.

2. **CPU and async work share a pool.** Module-task futures run on the Rayon
   pool the link and generate stages use, so nested Rayon work never creates a
   second CPU pool.

3. **Blocking I/O cannot take every lane.** The blocking cap is
   `worker_threads - 1`, so a burst of blocking reads cannot starve runnable
   futures. There is no hidden reserve worker.

4. **Wakeups are batched.** A wake enqueues a runnable; at most one bounded
   drain loop per worker goes to Rayon. One Rayon job per wake would turn
   every wake into a job submission and a thread wakeup, a context-switch
   storm on large module graphs.

5. **Configuration is frozen at load.** Flavor, worker count and blocking cap
   come from `ROLLDOWN_RUNTIME`, `ROLLDOWN_WORKER_THREADS` and
   `ROLLDOWN_MAX_BLOCKING_THREADS`, read once when the binding loads, so the
   threaded WASI loader can size its Worker pool from the same snapshot.
   There is no JS configuration API.

6. **Generations do not overlap.** Shutdown closes admission, cancels or
   finishes accepted work and joins every worker thread, so TLS destructors
   retire inside the barrier, before a restart creates the next pool. Cloned
   task wakers can outlive that barrier, so napi-rs pins the native addon image
   once a module that registered a custom backend has exported.

7. **Caught panic payloads are user code.** A payload's destructor can panic
   again. Drop a caught payload under its own `catch_unwind` and never let the
   nested panic out: `rolldown_std_utils::panic_payload` leaks the nested
   payload. Otherwise a second panic can escape a napi cleanup callback or
   leave a close path stuck. The deferred-drop thread catches a panicking
   `Drop` with one `catch_unwind` and drops the caught payload as is. It
   retires its pending count from a guard built before the drop, so even a
   panic that escapes cannot wedge `drain()`.

8. **Detached tasks behave like tokio.** Dropping a `JoinHandle` detaches the
   task; runtime shutdown may cancel accepted work by dropping its future. The
   module loader's execution and supervision are one accepted task, so a panic,
   a shutdown cancellation or a rejected submission becomes exactly one build
   diagnostic and completion accounting cannot hang.

9. **No tokio in any shipped build.** There is no runtime feature to pick.
   `just check-no-tokio` proves the binding (native and both WASI targets) and
   the bench harness are tokio-free. On threaded WASI, napi async work
   (`parse`, `transform`) is served by the loader's emnapi worker pool, not by
   a runtime rolldown owns.

10. **No slack in timers or locks.** A timer fires at its exact deadline, so a
    zero-length sleep is ready on its first poll, and the runtime's locks are
    fair only on average. Code must not depend on timer rounding or on the
    order in which waiters get a lock. The watcher enforces this with its
    debounce floor and by taking queued input before a due deadline; see
    "Rolldown's Approach" in
    [watch-mode/implementation.md](../watch-mode/implementation.md).

## Why two WASI artifacts

Can one `.wasm` switch between threaded and single-threaded at runtime? No.
The reasons are in the binary, not preference.

**JS controls size, not sharedness.** Both binaries import `env.memory`, and
the loader builds the `WebAssembly.Memory`, so JS picks `initial`, `maximum`
and growth. The `shared` flag is part of the module's declared import type,
fixed at compile time, and `WebAssembly.instantiate` checks it in both
directions:

- non-shared memory → threaded module: `LinkError`
- shared memory → threadless module: `LinkError`

The built binaries show it (flags byte: bit 0 = has maximum, bit 1 = shared):

```
rolldown-binding.wasm32-wasi.wasm        (wasm32-wasip1-threads)
  memory import env.memory   flags=0x03  shared=true
  host import  wasi.thread-spawn
  export       wasi_thread_start

rolldown-binding.wasm32-wasip1.wasm      (wasm32-wasip1)
  memory import env.memory   flags=0x01  shared=false
  (no thread-spawn import, no wasi_thread_start export)
```

**Three compile-time facts block a runtime switch:**

1. The memory type: one binary declares `shared=true`, the other `false`, and
   instantiation enforces it.
2. The thread ABI: the threaded binary imports `wasi.thread-spawn`, which the
   JS runtime answers with a Worker that re-instantiates the same module on the
   same shared memory at `wasi_thread_start`. The threadless binary has none of
   this: the import, the entry and the TLS setup are absent.
3. Rust std: on `wasm32-wasip1-threads`, `thread::spawn` works through that
   ABI. On `wasm32-wasip1` it compiles but fails at runtime, so the scheduler
   runs CurrentThread on host-driven turns.

**Rejected: compile the threadless target with `+atomics`.** The spec allows
atomics on non-shared memory (only `memory.atomic.wait` traps). It changes none
of the three facts above.

**Rejected: ship only the threaded binary.** Shared memory needs
`SharedArrayBuffer`. workerd, StackBlitz-class embedders and pages without
cross-origin isolation have none, so the one binary would fail with
`LinkError` exactly where the threadless flavor is needed.

## Why one scheduling domain

How the design answers each pressure:

| Pressure                                             | Answer                                                                                                                                                                                |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Filesystem reads block                               | Reads go to the blocking lane (`spawn_blocking`). On MultiThread its cap keeps one lane free for futures; CurrentThread admits one blocking task and runs it inline on its sole lane. |
| CPU-heavy module tasks run inside async tasks        | Futures are polled on the Rayon workers that run `par_iter`.                                                                                                                          |
| Many rolldown processes on one host oversubscribe it | Async, CPU and blocking work share one pool whose size `ROLLDOWN_WORKER_THREADS` sets; blocking work takes existing lanes, not new threads.                                           |
| A browser main thread cannot park                    | Threadless wasm runs CurrentThread on host-driven turns and never parks; deferred drops run inline on wasm.                                                                           |

**Rejected: a dedicated read pool.** It brings back the second pool this
design removes.

**Rejected: a small fixed blocking cap.** On a dedicated pool a small cap cuts
the number of threads issuing kernel I/O. Here blocking work runs on existing
workers, so the cap does not change the thread count; it only limits how many
workers may occupy the blocking lane. On MultiThread, `worker_threads - 1` is a
liveness rule (one lane always stays runnable), not a tuned value; CurrentThread
admits one blocking task and runs it inline.

## Unresolved Questions

- **Should the vitest suite run inside workerd?** Not today: the tests read
  fixtures from disk, spawn processes and use snapshots, and workerd has no
  `fs` or `process`. Preferred: port scenarios case by case into
  `packages/workerd-tests/worker.js` (fixtures as virtual modules, the Node
  driver asserts). `@cloudflare/vitest-pool-workers` would be a standalone
  project, worth it only if many tests must live inside workerd.
- Should an idle preloaded Worker's load failure latch the crash flag? See
  implementation.md, "Crash latch".

## Related

- [implementation.md](./implementation.md) — the machinery
- [napi-rs `wasi-heap-sync-design.md`](https://github.com/napi-rs/napi-rs/blob/main/cli/docs/wasi-heap-sync-design.md) —
  the threaded WASI memory-size bug and its fix
- [bundler-data-lifecycle](../bundler-data-lifecycle/implementation.md) —
  rebuild ownership and `ScanStageCache`; deferred drops are in
  implementation.md, "Deferred drop worker"
