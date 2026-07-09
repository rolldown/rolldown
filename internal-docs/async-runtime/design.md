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
   worker from blocking admission. MultiThread first clamps the requested count
   to Rolldown's 256-worker production ceiling and
   `rayon::max_num_threads()`, has a truthful minimum of two configured and
   physical workers, and limits the blocking cap to `worker_threads - 1`. No
   hidden reserve worker exists. Blocking task
   start/completion metrics include exact-dependency work run through a nested
   `block_on`, while active/high-water metrics count admitted physical lanes;
   dependency lending reuses its owner's lane instead of reporting another one.
   CurrentThread has one physical blocking lane. Uncontended work and lexical
   same-frame nesting execute inline, but a native cross-driver contender is
   queued instead of parking inside a task poll. If the lane owner is awaiting
   that task, the existing exact `JoinHandle` dependency chain authorizes the
   owner's `block_on` frame to service only that queued job on its own lane.
   Unrelated jobs remain FIFO-serialized until the lane is released. Ownership
   is therefore neither tied only to the polling OS thread nor inherited by
   every task spawned from a blocking closure.
   Every dependency record carries its stable job id and, once owner lineage is
   known, the exact `BlockingOwnerToken` frame whose lane it may borrow. The
   cooperative driver claims that live pair atomically, reserves only that
   active owner frame with a unique transfer identity, and removes the exact job
   through an O(1) queue index. A stale transfer drop cannot release a newer
   reservation; a later unrelated job or dependency from another frame cannot
   use it. Dependencies that cross an async `JoinHandle` acquire the ambient
   owner frame when they enter the owner's lineage. Lending requires that exact
   token to remain lexically ambient on the cooperative driver; active-owner
   registry cardinality is never treated as ancestry. A stolen Rayon descendant
   that has lost its thread-local token, or a scheduler runnable driven from an
   owner frame, therefore cannot borrow an unrelated lane even when only one
   owner is active.
   Dependency propagation creates one local liveness link per async join hop
   over a shared one-shot exact-job claim. Link withdrawal and final claiming
   serialize through that shared claim, so either a withdrawal invalidates the
   chain first or the exact claim commits first; validation cannot tear across
   the two. A task retains every blocking dependency publication observed
   during one user poll; a later pending handle cannot erase an earlier sibling.
   Every task poll withdraws all previous local publications before user code runs.
   That immediately invalidates ancestors that depended on the old hops, while
   leaving a child's still-live source claims available for the child to
   republish. Dropping a blocking handle clears the direct dependency by stable
   job id even if owner enrichment changed the publication metadata.
   Lending is driver-local and submits no global Rayon broadcast, so an
   unrelated parked worker cannot retain a probe batch, block later lending, or
   keep shutdown-idle accounting active. Each idle cooperative pass selects at
   most one eligible live publication without cloning the full collection; an
   owner-handoff probe snapshots only parker and dependency handles, releases
   the parked-driver registry lock, then evaluates dependency predicates.
   Completing one transfer wakes at most one parked blocking-capable driver that
   published the same owner lineage, so an unrelated newer parker cannot absorb
   the handoff. This lets another exact dependency retry without wake
   amplification. Every reservation release, including a failed exact claim,
   performs that same owner-targeted handoff. If it races just before parker
   publication, the registered driver rechecks availability for its lexically
   ambient exact owner before sleeping. The selected parker retains the owner
   identity until retry; if its publication is withdrawn first or the driver
   exits, it forwards the handoff to the next live same-owner waiter instead of
   consuming the only wake.

4. **Work classes receive bounded service.** Runnable locality remains the
   normal priority, but a continuously hot runnable stream yields to the
   blocking FIFO after a fixed quantum. A `block_on` driver gives its live exact
   blocking dependency first claim on that forced turn, including when the
   normal blocking cap is saturated; unrelated FIFO work cannot displace the
   dependency the driver is awaiting. Exhausting the LIFO budget forces one
   shared-FIFO turn even if polling the awaited future immediately refills the
   local slot. Timer deadlines are serviced by one lifecycle-managed non-Rayon
   thread per MultiThread generation. That thread executes no runnable or
   blocking user work, so an armed timer cannot reduce the configured Rayon
   capacity or become an unreported CPU worker. The first timer poll starts the
   service thread before publishing its heap entry, so an OS-thread creation
   failure cannot leave an unreachable waker in a generation with no
   timekeeper.

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
   initial lifecycle `start` leaves backend creation lazy. Once a backend is
   created, configuration remains frozen across its shutdown and every later
   generation. A zero-work environment teardown preserves the pre-first-use
   window: shutdown enters a non-restartable zero-backend stopping state until
   rejected user destructors quiesce, then records stopped-before-first-use
   without creating a backend. The next lifecycle `start` restores lazy
   `Initial`. Submissions while stopped are rejected until that start. Partial
   binding updates merge, validate, and commit while holding the controller
   mutex, so concurrent calls serialize against the latest committed options
   instead of overwriting disjoint fields from stale snapshots. A rejected
   candidate leaves the prior configuration unchanged.
   Native `ROLLDOWN_*` worker counts clamp to 256 and native Tokio blocking
   counts clamp to 512 before runtime construction. Explicit JavaScript options
   above 256 reject atomically. The native Tokio builder also checks the
   combined worker/blocking capacity before entering Tokio's internal addition.
   Every native shared-runtime package entry installs both CurrentThread host
   bridges before that window can be used. Host installation is independent of
   the import-time flavor, so a legal synchronous `MultiThread -> CurrentThread`
   update cannot leave a module-cached environment without runnable or timer
   delivery. Tokio builds skip those bridges.

7. **Lifecycle transitions linearize with submission and generations do not
   overlap.** Backend acquisition, explicit start, and shutdown share one
   controller mutex. A submission either registers as accepted work in the
   running generation before shutdown or observes `Stopping`/`Stopped`; it can
   never lazily recreate a backend after shutdown. Shutdown closes acceptance,
   aborts accepted async work, drops queued blocking work, waits for running
   work and scheduler roles, and joins every Rayon worker's native thread.
   Joining, rather than observing only Rayon's exit hook, includes thread-local
   destructors in the retirement barrier. A retiring worker keeps its runtime
   identity through TLS teardown, so lifecycle reentry from a TLS destructor
   fails fast instead of waiting for shutdown to join that same thread.
   Concurrent `start` waits for that quiescence. Calling `start` or `shutdown`
   from work in the generation being retired returns an error instead of
   self-deadlocking.
   The controller takes the generation's stop-publication mutex while holding
   the lifecycle mutex, publishes stop, and only then changes `Running` to
   `Stopping`. The MultiThread final verdict takes the same publication mutex
   after closing its admission gate. If the verdict wins that mutex, the
   lifecycle remains `Running` until its final checks complete; if shutdown
   wins, the verdict must observe stop. The controller releases publication
   before aborting accepted tasks, so an abort wake blocked behind the verdict
   gate cannot form a cycle.
   Shutdown closes and drain-fires timers, wakes every runtime-owned `block_on`
   parker, and scopes queued/rejected destruction to the retiring generation.
   Rejected convenience submissions hold the lifecycle transition until their
   contained destructor finishes, including when construction of the first
   backend failed. Initial and already-stopped shutdown first enter an explicit
   zero-backend stopping state; concurrent start and configuration remain
   closed until the initiating shutdown publishes its final stopped state.
   Reentrant start or shutdown from the destructor fails instead of waiting on
   itself, while an external initial start waits for destruction to retire.
   Restart therefore cannot expose a new generation to destructor re-entry.
   Accepted async and blocking submissions likewise keep
   their generation registration outside compiler-generated capture drop order:
   if shutdown closes an executor queue after controller admission, user
   captures are destroyed before the registration retires. Public `block_on`
   keeps the same work and generation guards until its erased future has been
   destroyed after an early stop. If backend acquisition rejects an owned
   `block_on` input, its destruction is registered with the same lifecycle
   barrier used by rejected convenience submissions, so first-backend retry,
   shutdown, and restart cannot overlap that destructor. The driven future is
   also held behind the scheduler's contained-drop wrapper, preventing a poll
   panic plus a panicking future destructor from aborting an unwind-enabled
   native process.
   CurrentThread host turns and host-dispatch publications are
   generation-scoped scheduler work until their complete native callback
   returns.
   This includes every `Runnable::run`, detached task-output destruction, and
   every initial, recovery, host-replacement, or bounded-turn continuation
   dispatch call. Shutdown and restart therefore cannot overlap a host callback
   from the old generation, even if queue cancellation retires the accepted
   task that originally requested that callback.
   Every executor dispatch owns one globally unique, nonzero internal
   capability. The registry races it across live hosts through distinct,
   globally unique delivery capabilities scoped to one host registration and
   one attempt. Admission first resolves that exact delivery back to the still
   live internal capability, then atomically consumes the internal capability
   and claims the executor's scheduler role while the controller lifecycle
   mutex still proves its generation is `Running`. A delayed callback from an
   unregistered host, an older attempt, or a serviced dispatch cannot resolve
   and is a no-op.
   Each host has one physical delivery slot plus one coalesced latest-pending
   internal capability. A blocked event loop therefore retains at most one
   queued/in-flight threadsafe-function callback while responsive hosts keep
   advancing; completion or failure schedules at most the latest still-needed
   replacement.
   A publication remains provisional until every host call in that broadcast
   has returned. Synchronous queue rejection and unregister/sweep retirement
   are retained behind that barrier, so the final result distinguishes a
   broadcast that nobody accepted from one whose accepting attempts all
   failed. Removing the last accepted in-flight or coalesced reference reports
   that exact dispatch failure even when another dispatch was reserved while
   the physical callback was still running.
   Stable scheduler identities fail closed before `u64` reuse. Generation,
   executor, blocking-job/owner/reservation, host-registration, and timer
   identities participate in stale-handle rejection or indexed lookup, so
   wrapping any of them would turn an impossible exhaustion event into an ABA
   authorization bug.
   Delivery acknowledgement and failure are tracked per host attempt and
   internal dispatch. One host failure cannot invalidate another host's
   accepted or delayed attempt. Only the last unserviced accepting attempt may
   begin recovery. Every registry transition to `Cancelling` emits an exact
   failure capability containing the internal dispatch and its failure epoch.
   Reopening that dispatch for an armed rebroadcast advances the epoch. The
   executor validates the complete capability while holding the scheduler-idle
   mutex before it consumes the pending dispatch, so a delayed completion from
   an older epoch cannot cancel a rebroadcast already accepted in a newer one.
   The executor consumes a current old capability and reserves its one exact
   replacement under the same mutex, so a newly registering host can only join
   the tagged replacement and cannot publish an untagged dispatch in between.
   Publication start and completion are coordinated per internal capability
   under that mutex. A registration arriving while the capability is being
   broadcast requests one coalesced rebroadcast from the existing publication
   owner; an older `Unavailable` result therefore cannot retire a capability
   that the later host request accepted. If the older broadcast returns
   `Failed`, the owner reopens only that exact closed registry epoch after its
   publications and host references reach zero, then rebroadcasts the same
   capability. Rebroadcasting a tagged replacement never allocates a second
   replacement. If no rebroadcast is armed, removing the failed publication
   owner and reserving its replacement or claiming terminal cancellation are
   one scheduler-locked transition. A late host request therefore either joins
   the old owner before completion, joins the reserved replacement afterward,
   or observes cancellation; it cannot create a fresh owner for the closed
   registry dispatch between those decisions. Exact registry cancellation
   remains RAII-owned while any failed-dispatch action publishes the reserved
   replacement, and removes state only when both dispatch and epoch still
   match. A nested replacement unwind therefore retires the old closed epoch
   without consuming the still-retryable replacement.
   The executor catches a host-dispatch unwind at the publication-owner
   boundary: an already-armed rebroadcast runs under that owner before the
   original panic resumes. A scoped payload owner keeps that first panic
   authoritative while scheduler resolution and every nested recovery
   publication run under another unwind boundary. Later payloads, including
   payloads with panicking destructors, are discarded through the contained
   payload-drop path before the first panic resumes. Without an armed request,
   or after stop or terminal cancellation begins, unwind cleanup releases the
   owner without retrying.
   Registry publication owners remain RAII-scoped, and the registry allocates
   every host delivery capability and required output capacity before mutating
   any physical delivery slot, preventing a partial allocation failure from
   leaving an undispatched slot occupied. If every attempt for the replacement
   also fails without a newly armed request, the executor claims terminal
   cancellation while retiring that publication under the same mutex and
   cancels the coalesced queued work under the normal generation and
   panic-containment boundaries.
   This bounds scheduler failure without requiring an unrelated later wake,
   while future submissions can start a new dispatch chain.
   The task-host boundary is native-only. Each environment registers a weak
   Node-API threadsafe function whose JavaScript function is null and whose
   custom native callback receives the exact delivery payload on a fresh event
   loop turn. That callback calls `drive_current_thread_tasks` and acknowledges
   only a confirmed exact claim. No delivery token or claim result crosses
   JavaScript, and the binding exports no drive/cancel capability functions.
   `registerCurrentThreadTaskHost()` accepts no arguments; direct binding misuse
   with a callback fails synchronously before the callback can run, create a
   Promise, or expose a thenable return. Promise constructors, `then` methods,
   species, and rejection observation are therefore outside the scheduler host
   contract rather than mutable authorities it must defend.
   The JavaScript package checks an explicit native task-host contract version
   before invoking either host registration. A preceding callback-accepting
   binding has no version export and fails as a package/binding mismatch before
   JavaScript can call its incompatible registration boundary.
   The TSFN's raw slot is also its one initial Node-API acquisition. Normal
   environment cleanup takes and releases that capability exactly once before
   Node's own TSFN cleanup runs. An explicit rollback takes it with abort mode;
   a `napi_call_threadsafe_function()` result of `napi_closing` takes it without
   another API call because Node has already decremented that acquisition and
   invalidated the pointer. Finalization only invalidates a still-visible slot
   and never re-enters Node from its own finalizer.
   Shutdown therefore either transitions first and rejects the native turn, or
   observes the claimed role and waits for it. Shutdown retires coalesced
   pending deliveries while retaining the one physical in-flight slot until its
   native callback or host registration retires. A delayed callback from an
   older generation, registration, or attempt is a no-op and cannot drain
   replacement work.
   Scheduler-owned blocking execution retains the active generation through
   result delivery and caught panic-payload destruction, including exact
   owner-lane lending. Payload destructors therefore cannot re-enter a newer
   generation or shut down the generation that is still executing them. Every
   successful result retained by a join handle, including queued blocking and
   immediate CurrentThread results, remains tagged with its producing
   generation until a poll transfers ownership to the caller. Destruction of an
   unpolled old-generation result re-enters that generation, and controller
   entry rejects its submission or lifecycle calls once another generation is
   running or stopping.
   Generation quiescence does not prove that every externally cloned task
   waker has been dropped: its wake, clone, and drop vtable can remain callable
   after the task future and all runtime-owned workers retire. Native
   async-runtime builds therefore deliberately retain the addon image. After a
   module that registered a custom backend exports successfully, napi-rs pins
   its native image permanently. This guarantee is independent of Tokio being
   enabled and independent of runtime-generation cleanup. Failures before any
   native callback or handle can escape roll back without retention; failures
   after exports or environment support may have exposed native values retain
   the image conservatively.
   User-owned destructors and timer wakers are isolated during shutdown. Caught
   panic payloads are dropped under a second unwind boundary; only the nested
   payload produced by a hostile payload destructor is quarantined, so normal
   payload state is reclaimed without letting a second panic leave the
   controller permanently stuck in `Stopping` or escape a napi environment
   cleanup callback. The deferred-destruction worker uses the same boundary so
   it cannot die and discard queued jobs while leaving their pending counts
   permanently registered.
   Deadlock-detection durations that cannot be represented as an `Instant`
   deadline are treated as effectively unbounded. Idle drivers remain
   wakeable, and the same rule keeps an armed host-timer wait live, instead of
   allowing oversized environment configuration to panic or self-deadlock the
   scheduler.
   A MultiThread deadline verdict is also an admission linearization point.
   Only when the opt-in detector is enabled, runnable publication, blocking
   publication, LIFO-slot flushing, timer registration, and every timer firing
   path announce an active admission and advance a publication epoch. Timer
   firing retains admission from heap removal through waker invocation, so the
   verdict cannot observe both the timer and its resulting runnable as absent.
   The final verdict closes that gate, rejects a verdict while an earlier
   admission remains active, then takes the stop-publication mutex and rechecks
   queues, permits, timers, progress, the publication epoch, and generation
   stop while later admissions are excluded. Exact-owner reservation release
   and every targeted handoff/forwarding path use a second detector-only
   publication mutex. Release takes it before making the owner lane available
   and retains it through handoff selection and unpark. The final verdict takes
   it after stop publication, then rechecks both a pending handoff on its parker
   and exact-owner lane availability. Thus a release either follows a completed
   verdict or is necessarily visible to its final predicates, including when no
   parker remained registered to receive the handoff.
   Lock ordering is lifecycle mutex then stop publication for shutdown, and
   verdict gate then stop publication then exact-owner publication for
   detection. Exact-owner publication precedes existing owner, dependency,
   parked-driver, and parker locks; no reverse path retains one of those locks
   while acquiring publication. Direct executor shutdown publishes stop before
   closing the blocking queue, matching the verdict's stop-before-queue order.
   All verdict guards reopen before `panic_any`, because panic hooks may
   synchronously submit diagnostic, cleanup, owner-release, or lifecycle work.
   The default detector-disabled runtime retains only predictable option
   branches and performs no admission atomics or publication locking.
   Threaded-WASI JavaScript ownership follows the same rule across host realms:
   every public async operation receives a native RAII token, and a restart
   waits off the JavaScript thread for the previous Tokio generation to retire.
   No realm-local "first owner" may stand in for process-global ownership.
   Bindings from the preceding implicit-owner protocol fail closed instead of
   attempting to coordinate that single owner through realm-local JavaScript
   state.
   JavaScript close single-flight state is published before invoking cleanup,
   so synchronous re-entry joins the original lifecycle attempt rather than
   creating a second owner-release or native-close sequence.
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
   async-task owns their destruction. Every retained successful output carries
   its producing generation until extracted by a poll, so blocking and immediate
   results receive the same boundary when a completed value remains unpolled.
   Owned `block_on` futures use that containment while they are driven and
   while a rejected runtime acquisition destroys them.
   Handle retirement precedes dependency notification, and the buffered result
   destructor and arbitrary dependency waker have separate unwind boundaries,
   preventing a hostile pair from becoming a double panic.
   Contained owned-waker delivery borrows with `wake_by_ref`, then destroys the
   waker under a second boundary; a wake panic and a `RawWaker` destructor panic
   therefore cannot combine while one consuming `Waker::wake` frame unwinds.
   Normal dependency `set`, `clear`, and conditional-clear notifications use
   that same central panic boundary after committing their state transition.
   Waiter clone, replacement, wake, retirement, and final destruction are also
   generation-scoped and panic-contained, and no waiter destructor runs while
   the dependency mutex or dependency-stack `RefCell` is borrowed. Cooperative
   driver unwind cleanup independently forwards owner handoffs, flushes local
   slot work, and compensates absorbed queue wakes. Detaching a task clears its
   retained waiter before handing ownership to async-task, so a parent cannot
   remain reachable solely through an abandoned dependency registration.
   Internal module-loader execution and supervision are one accepted task, so
   panic, shutdown cancellation, or rejected submission becomes exactly one
   build diagnostic and completion accounting cannot hang.

9. **The supported compatibility path does not change.** Native and
   `wasm32-wasip1-threads` builds without `async-runtime` retain napi-rs's
   Tokio executor and Rolldown's previous behavior. Threadless
   `wasm32-wasip1` rejects the Tokio-only feature combination at compile time:
   napi-rs can construct a current-thread runtime there but rejects every
   built-in async submission, so such an artifact cannot run Rolldown.

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
deferred workerd loader, and ship no worker scripts. Every managed workerd
instance factory must install both CurrentThread task and timer hosts before
returning. A synchronous host-turn scheduling failure must clear its coalescing
state so a later dispatch can retry, and initialization cleanup failures must
remain visible even when the primary thrown value is a primitive.

## Related

- [implementation.md](./implementation.md) - the scheduler and integration map
- [bundler-data-lifecycle](../bundler-data-lifecycle/implementation.md) -
  deferred drops and rebuild ownership
