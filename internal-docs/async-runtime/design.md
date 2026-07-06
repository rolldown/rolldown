# Async Runtime - Design & Principles

## Summary

Rolldown's optional async runtime uses one scheduling domain for async task
polling, CPU parallelism, and bounded blocking filesystem work. Heavy
post-build destruction uses a separate serial maintenance worker so a rebuild
can never wait on a drop queued behind itself in the shared pool. The runtime
supports a cooperative current-thread flavor for hosts without threads and a
work-stealing multi-thread flavor for native builds. Shutdown is
generation-quiescent: accepted work is cancelled or completed, scheduler roles
exit, and physical workers retire before a restart may create the next pool.
The existing Tokio runtime remains the default and is selected by the
`tokio-runtime` feature; the new implementation is selected by
`async-runtime`. See
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
   worker from blocking admission. MultiThread has a truthful minimum of two
   configured and physical workers, and the blocking cap is at most
   `worker_threads - 1`. No hidden reserve worker exists. Blocking task
   start/completion metrics include exact-dependency work run through a nested
   `block_on`, while active/high-water metrics count admitted physical lanes;
   dependency lending reuses its owner's lane instead of reporting another one.
   Every dependency record carries its stable job id and, once owner lineage is
   known, the exact `BlockingOwnerToken` frame whose lane it may borrow. The
   cooperative driver claims that live pair atomically, reserves only that
   active owner frame with a unique transfer identity, and removes the exact job
   through an O(1) queue index. A stale transfer drop cannot release a newer
   reservation; a later unrelated job or dependency from another frame cannot
   use it. Dependencies that cross an async `JoinHandle` acquire the ambient
   owner frame when they enter the owner's lineage. A stolen Rayon descendant
   that has lost thread-local ancestry may bind its live untagged publication
   only when that executor's registry contains exactly one active, available
   owner frame. The binding is persisted before reservation so later targeted
   handoff sees the same lineage. Multiple or nested owner candidates remain
   ambiguous and never authorize an over-cap escape.
   Lending is driver-local and submits no global Rayon broadcast, so an
   unrelated parked worker cannot retain a probe batch, block later lending, or
   keep shutdown-idle accounting active. Each idle cooperative pass performs at
   most one exact claim attempt; there is no dependency-vector cloning or
   worker-count multiplier. Completing one transfer wakes at most one parked
   blocking-capable driver that published the same owner lineage, so an
   unrelated newer parker cannot absorb the handoff. This lets another exact
   dependency retry without wake amplification.

4. **Work classes receive bounded service.** Runnable locality remains the
   normal priority, but a continuously hot runnable stream yields to the
   blocking FIFO after a fixed quantum, and exhausting the LIFO budget forces
   one shared-FIFO turn even if polling the awaited future immediately refills
   the local slot. The timer timekeeper drains runnables only; that role remains
   runnable-only through nested `block_on` and never enters a potentially
   unbounded blocking closure.

5. **Wakeups are batched.** A future wake enqueues a runnable. At most one
   bounded drain loop per worker is submitted to Rayon, and each loop processes
   multiple runnables. Submitting every wake as an individual Rayon job caused
   excessive context switches on large module graphs. CurrentThread queue
   publication wakes every active explicit `block_on` driver after enqueueing,
   without polling inline. The fanout is bounded by the number of explicit
   drivers and prevents a newer driver blocked inside `poll` from absorbing the
   only wake while an older driver sleeps with runnable work available.

6. **Configuration is immutable after first use.** Runtime flavor, physical
   worker count, and blocking concurrency may be configured from the binding
   API or environment before the first async call. Lazy startup makes top-level
   configuration possible without changing module registration order: napi's
   initial lifecycle `start` leaves backend creation lazy. Once the backend is
   created, or shutdown begins, configuration remains frozen. Submissions
   during `Stopping` and after shutdown are rejected until `start` creates the
   next backend. Partial binding updates merge, validate, and commit while
   holding the controller mutex, so concurrent calls serialize against the
   latest committed options instead of overwriting disjoint fields from stale
   snapshots. A rejected candidate leaves the prior configuration unchanged.

7. **Lifecycle transitions linearize with submission and generations do not
   overlap.** Backend acquisition, explicit start, and shutdown share one
   controller mutex. A submission either registers as accepted work in the
   running generation before shutdown or observes `Stopping`/`Stopped`; it can
   never lazily recreate a backend after shutdown. Shutdown closes acceptance,
   aborts accepted async work, drops queued blocking work, waits for running
   work and scheduler roles, and observes every Rayon worker exit. Concurrent
   `start` waits for that quiescence. Calling `start` or `shutdown` from work in
   the generation being retired returns an error instead of self-deadlocking.
   Shutdown closes and drain-fires timers, wakes every runtime-owned `block_on`
   parker, and scopes queued/rejected destruction to the retiring generation.
   Rejected convenience submissions hold the lifecycle transition until their
   contained destructor finishes, so restart cannot expose a new generation to
   destructor re-entry. Public `block_on` keeps the same work and generation
   guards until its erased future has been destroyed after an early stop.
   CurrentThread host turns are scheduler work until their complete
   `Runnable::run` returns, including detached task-output destruction, so
   shutdown and restart cannot overlap a host callback from the old generation.
   CurrentThread blocking panic payloads are likewise converted and destroyed
   before their guards retire, so payload destructors cannot re-enter a newer
   generation or shut down the generation that is still executing them.
   User-owned destructors and timer wakers are isolated during shutdown. Caught
   panic payloads are dropped under a second unwind boundary; only the nested
   payload produced by a hostile payload destructor is quarantined, so normal
   payload state is reclaimed without letting a second panic leave the
   controller permanently stuck in `Stopping` or escape a napi environment
   cleanup callback. The deferred-destruction worker uses the same boundary so
   it cannot die and discard queued jobs while leaving their pending counts
   permanently registered.
   Threaded-WASI JavaScript ownership follows the same rule across host realms:
   every public async operation receives a native RAII token, and a restart
   waits off the JavaScript thread for the previous Tokio generation to retire.
   No realm-local "first owner" may stand in for process-global ownership.
   Environment teardown cancels pending acquisition waits, while native token
   finalization closes the gap between async-work completion and JavaScript
   delivery. Once generation shutdown begins, stop outranks queued work and
   stored self-wake permits in every explicit driver, so an always-self-waking
   future cannot prevent quiescence.

8. **Detached-task behavior matches Tokio.** Dropping Rolldown's `JoinHandle`
   detaches rather than cancels the task during normal operation. Runtime
   shutdown may cancel an accepted detached task by dropping its future, as
   Tokio runtime shutdown does. The complete async-task generator is held in a
   manually-dropped containment wrapper because async-task aborts the process if
   a future destructor unwinds; detached outputs use the same containment before
   async-task owns their destruction. Blocking results receive the same boundary
   when a completed value is still buffered in a detached join receiver.
   Handle retirement precedes dependency notification, and the buffered result
   destructor and arbitrary dependency waker have separate unwind boundaries,
   preventing a hostile pair from becoming a double panic.
   Normal dependency `set`, `clear`, and conditional-clear notifications use
   that same central panic boundary after committing their state transition.
   Waiter clone, replacement, wake, retirement, and final destruction are also
   generation-scoped and panic-contained, and no waiter destructor runs while
   the dependency mutex is held. Detaching a task clears its retained waiter
   before handing ownership to async-task, so a parent cannot remain reachable
   solely through an abandoned dependency registration.
   Internal module-loader execution and supervision are one accepted task, so
   panic, shutdown cancellation, or rejected submission becomes exactly one
   build diagnostic and completion accounting cannot hang.

9. **The compatibility path does not change.** Builds without
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
  Dropping a current-thread sleep clears the host timeout and resolves its relay
  task instead of leaving either alive until the deadline. Native watch mode
  therefore works on both flavors. The public capability contract marks binding
  dev mode unsupported on current-thread and watch unsupported on every WASI
  artifact. `dev()` rejects before callback/plugin/runtime setup, while
  `watch()` reports the unsupported runtime through its normal `ERROR`, `END`,
  and closable-emitter lifecycle before any setup side effects can run.
- The runtime layer normalizes an inherited `ROLLDOWN_RUNTIME=multi` override
  to `CurrentThread` before WebAssembly module initialization because the shared
  scheduler has no WebAssembly MultiThread executor.

## Dependent WASI Packaging

The dependent browser/WASI change owns the distinct artifact and publication
layout. It will retain the threaded build's `wasi` loader/wasm names and worker
scripts, while the single-thread build will use `wasip1` names, include the
deferred workerd loader, and ship no worker scripts.

## Related

- [implementation.md](./implementation.md) - the scheduler and integration map
- [bundler-data-lifecycle](../bundler-data-lifecycle/implementation.md) -
  deferred drops and rebuild ownership
