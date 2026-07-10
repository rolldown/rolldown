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
Rolldown explicitly enables both `napi/async-runtime` and `napi/async` in this
profile. OXC's NAPI crates currently enable Tokio too, but unload safety must
not depend on that transitive feature accident.

In that combined profile, napi-rs's established public free `spawn`,
`spawn_blocking`, `block_on`, and runtime-entry helpers retain their Tokio API
and behavior. The explicit `spawn_on_custom_runtime` and
`spawn_blocking_on_custom_runtime` helpers route through the registered
implementation in both pure and combined `async-runtime` builds, with stable
signatures under Cargo feature unification. Rolldown's own task creation uses
`rolldown_utils::futures`, so it reaches the shared scheduler directly;
arbitrary transitive calls to napi-rs's Tokio helper names must not be assumed
to use Rolldown's scheduler or bounded blocking lane.

Promise resolution, panic rejection, and cancellation handles remain owned by
napi-rs. `AsyncRuntime::spawn` transfers an opaque `AsyncRuntimeTask` and
Rolldown returns it untouched with an `AsyncRuntimeRejection` carrying the
controller's exact admission diagnostic when the controller is stopped or
backend construction fails. The optional `AsyncRuntime::spawn_blocking` hook
follows the same ownership and diagnostic rule for its closure. Accepted tasks
and closures retain napi-rs's cancel-on-drop guards.
Generated-task submission and custom runtime lifecycle operations delegate to
the registered implementation; `start` and `shutdown` report failures through
`napi::Result`. The borrowed `AsyncRuntime::block_on` hook delegates to
Rolldown's fallible driver and returns its admission or interrupted-drive
diagnostic through `napi::Result`; napi-rs then preserves it in
`try_block_on_custom_runtime` instead of replacing it with a generic runtime
failure. `AsyncRuntime` is an unsafe implementation contract because
Node may unload an addon image immediately after environment cleanup:
`shutdown` must leave no backend-owned thread, task, closure, destructor, or
callback able to execute addon code after it returns. Rolldown closes
generation admission, waits for every accepted work guard, drops the executor,
and waits for all native workers to exit, but that proves only runtime-owned
quiescence. An externally cloned `Waker` can outlive task/future retirement and
retain wake, clone, and drop vtables in the addon image.

napi-rs supplies the remaining guarantee independently of Tokio. A custom
backend may register during module initialization, but napi-rs commits
permanent native-image retention only after every module export and the
per-environment support machinery initialize successfully. It pins the image
with `GetModuleHandleExW(PIN | FROM_ADDRESS)` on Windows or a validated leaked
`dlopen` reference on supported Unix; unsupported native loaders abort rather
than return with callable unmapped code. A failure before exports or
per-environment support can expose native callbacks rolls back without
retention. Later initialization failures conservatively retain the image when
native values may already have escaped. The backend object is reused across
zero-environment shutdown/start cycles and its `Drop` is not guaranteed to run, so
`AsyncRuntime::shutdown` must still release and quiesce all active resources.
The `unsafe impl AsyncRuntime` comment in
`crates/rolldown_binding/src/async_runtime.rs` records both halves of this
contract. Rolldown's pinned napi-rs revision must include that retention
implementation before this code can ship; the unsafe-trait change deliberately
makes an older revision fail to compile instead of producing an unpinned
binary.

The integration currently pins the coordinated v3-manifest compatibility
revision `9cd62b6ad6d92d6ebd19d9b5e4fcf4461d687d91` for `napi`,
`napi-derive`, and `napi-derive-backend`. `BindingMagicString` uses concrete
`MagicString<'static>` storage backed by `Cow::Owned`, so the public N-API
class cannot borrow a constructor call frame and moving the input `String`
does not add a second source allocation.

### Rolldown scheduler

`crates/rolldown_utils/src/async_runtime.rs` owns the lazy global controller.

- The controller lifecycle is
  `Initial -> Running -> Stopping -> Stopped` for real generations, with a
  separate `StoppedBeforeFirstUse` state for zero-work environment teardown.
  Initial work may lazily create the backend. napi invokes `start` during addon
  registration, so the initial `start` leaves `Initial` unchanged to preserve
  the documented pre-first-async-call configuration window. Shutting down
  `Initial` creates no backend. It first records
  `StoppingWithoutBackend(StoppedBeforeFirstUse)`, waits for rejected
  convenience-submission destructors, then publishes `StoppedBeforeFirstUse`.
  Shutdown from an already stopped lifecycle uses the same non-restartable
  transition with its corresponding final state. The next lifecycle `start`
  returns stopped-before-first-use to lazy `Initial`, so repeated zero-work
  start/shutdown cycles do not freeze configuration. If first-backend
  construction fails, no retry may create a generation until rejected user
  state has been destroyed. An external `start` waits even though the lifecycle
  is still `Initial`, while reentrant lifecycle calls from that destructor
  return an error instead of self-deadlocking. Shutdown changes
  `Running` to `Stopping` under the controller mutex before closing the
  generation. Submissions while stopped return their task or closure untouched.
  A real-generation restart waits while `Stopping`, then creates the next
  `Running` generation, and configuration remains frozen once any real backend
  has existed. Full configuration, partial merge/validation/commit, start,
  shutdown, and submission use the same mutex. A racing submission cannot
  recreate the backend after shutdown, and concurrent partial updates cannot
  read the same stale options snapshot and overwrite one another.
  While holding the lifecycle mutex, the controller takes the generation's
  stop-publication mutex, publishes stop, and only then mutates `Running` to
  `Stopping`. The MultiThread final verdict closes its admission gate and takes
  the same publication mutex for its stop and scheduler-state rechecks. A
  verdict that wins keeps the lifecycle in `Running`; shutdown that wins must
  be observed as stopping. Publication is released before
  `GenerationWork::close_and_abort`, so a synchronous abort wake may wait on
  the verdict gate without retaining either lifecycle lock.
- Each backend owns a generation work registry. Async tasks register an abort
  handle and all accepted operations register a retirement guard while the
  controller mutex is held. A generation stop registry also owns every active
  `block_on` parker. Shutdown closes and drain-fires timers, wakes those parkers,
  atomically closes the work registry, aborts accepted async work, closes and
  drains the queued blocking FIFO, and waits for every guard to retire.
  Async-task scheduler closures and heap sleeps hold weak executor references,
  so completed or cancelled work cannot keep an old pool alive accidentally.
  The complete async-task generator is wrapped in a generation-scoped
  `ManuallyDrop` future whose destructor catches and quarantines panics before
  async-task's abort-on-panicking-drop boundary. Every successful output
  retained by a `JoinHandle` uses an equivalent generation-scoped wrapper until
  a live poll extracts it. This includes async-task completion, MultiThread and
  queued CurrentThread blocking receivers, and immediate CurrentThread results.
  Dropping an unpolled handle therefore destroys its output under the producing
  generation even after restart. Controller entry compares that active
  generation with the current running/stopping generation and rejects stale
  submission, start, and shutdown attempts. MultiThread and queued
  CurrentThread blocking submissions use a corresponding registered-function
  wrapper: when controller admission wins but executor queue closure wins
  before publication, captured user state is destroyed on the submitting
  thread before its generation work registration retires. Every scheduler-owned
  blocking execution boundary also holds an outer generation guard through
  result delivery and caught
  panic-payload destruction. This covers normal FIFO service and exact
  owner-lane lending in both executors, after an inner function wrapper has
  unwound. The public fallible `try_spawn` API returns
  `(RuntimeConfigError, future)` when admission fails. Lifecycle owners use it
  when consuming a one-shot coordinator future so shutdown races can restore
  and retry that exact future after `start`. `try_spawn_blocking` returns
  `(RuntimeConfigError, closure)` under the same rule. The convenience `spawn`
  and `spawn_blocking` APIs contain destruction of rejected work and return an
  immediate failed handle carrying the same error text for callers that do not
  need recovery.
- The owned Rust `block_on` helper acquires its generation work scope before
  moving the input future into the driver. Acquisition failure registers
  rejected destruction under the controller lifecycle mutex, then destroys the
  future behind the same panic boundary and reentrancy context used by
  convenience spawn rejection. Successful driving wraps the future in
  `ContainedFuture`, so cancellation, an early stop, or a poll panic cannot
  combine with a panicking future destructor and abort the native process.
  The borrowed `try_block_on_dyn` helper returns admission failure without
  polling and reports a concurrent stop after the driver releases the future;
  the caller retains the same pinned value and may retry it after `start`.
  The NAPI backend hook maps that error directly into `napi::Result` and remains
  covered independently by napi-rs's synchronous runtime-use gate and
  `SafeDrop` wrapper.
- `CurrentThreadExecutor` uses a reentrancy-safe FIFO runnable queue. In a host
  embedding, a wake requests a fresh host turn before polling: futures such as
  `futures::Shared` invoke outer wakers while holding internal locks, so polling
  inline from the scheduler callback can re-enter the same future and
  self-deadlock. The Node binding registers one weak native
  threadsafe-function-backed task driver per environment. It creates the
  threadsafe function with a null JavaScript function and a custom native
  `call_js_cb`, so the event loop still supplies a fresh turn while delivery
  capabilities never enter JavaScript. The executor publishes one exact
  internal dispatch capability, and the registry gives each live host a
  distinct opaque delivery capability for that registration and attempt. The
  registry stores hosts in a keyed linked order, indexes each live delivery
  capability directly, and keeps an exact reference count on each internal
  dispatch. Broadcast planning, acknowledgement, failure, and completion are
  therefore linear in the number of hosts without losing registration-order
  fallback or stale-generation rejection. The
  first responsive delivery resolves to and consumes the internal capability;
  delayed, unregistered, or retired delivery capabilities fail lookup before
  executor admission. This registration is also present in the
  browser build because fresh-turn polling is a future/scheduler requirement,
  independent of Node timers. Wakes are enqueue-only even when no host is
  registered. After publishing a runnable, the queue walks the generation's
  parker registry and wakes every active explicit `block_on` driver without
  allocating a snapshot or polling from the wake caller. The fanout is bounded
  by concurrent explicit drivers and prevents a newer driver blocked inside
  `poll` from absorbing the only queue wake while an older driver sleeps. Pure
  Rust use otherwise makes progress through an explicit `block_on` or
  `drive_current_thread_tasks` call. A host turn polls at most 64
  runnables before redispatching, so a self-waking task cannot monopolize the
  JavaScript event loop, and both host turns and explicit `block_on` drivers
  force a blocking turn after 16 consecutive runnable polls. For `block_on`,
  every exact blocking dependency published by the awaited future during one
  poll remains eligible, with one bounded claim attempt taking priority over
  ordinary FIFO work on that turn. Every runnable claim takes the scheduler
  mutex and then the generation stop-publication mutex before checking stop,
  terminal dispatch cancellation, and the queue. Shutdown publication and
  terminal cancellation therefore either follow a completed claim or prevent
  it, including for a host turn admitted before the publication; both guards are
  released before `Runnable::run`. Every explicit driver also checks generation
  stop before polling its own input future, so a shutdown wake cannot cause one
  final poll before stop wins over queue draining and stored self-wake permits.
  A nonzero pending dispatch coalesces later wakes before they allocate another
  process-global identity; callers that race after both observing zero still
  resolve through the exact-capability compare-exchange. Every host delivery
  also carries a globally unique, nonzero capability, separate from the internal
  dispatch.
  Each registry entry retains at most one queued/in-flight delivery and one
  latest pending internal capability. Repeated broadcasts to a blocked host
  replace that pending value instead of adding threadsafe-function queue
  entries. Native callback completion or failure frees the physical slot and
  submits at most the latest pending capability if another host has not already
  serviced it. Completion does not invoke the host driver directly; it asks the
  executor to republish the current capability so the follow-up owns the normal
  generation-scoped dispatch-call role and shutdown can serialize or reject it.
  The registry increments a per-dispatch publication count before liveness
  probing or invoking any host and decrements it only after the complete
  broadcast. A synchronously completed accepted failure therefore remains
  recorded while later provisional hosts are still deciding. The final
  publication returns `Unavailable` only when no attempt or reservation was
  accepted, and `Failed` when all accepted references retired without service.
  The registry publication count has an unwind guard. Before mutating a host
  entry, publication reserves every required delivery capability and all output
  vector capacity while the registry remains locked. An identity-exhaustion
  panic therefore releases the exact provisional count without leaving an
  undispatched physical slot or pending capability behind, and a later
  publication can retry the same internal dispatch.
  Rejection, unregister, and liveness sweep return terminal failures for any
  other coalesced dispatch they retire, so a host cannot strand a newer
  executor capability while rejecting its current physical call. A concurrent
  re-publication of an already-cancelling dispatch also returns `Failed`, never
  `Unavailable`, so it cannot clear the executor capability ahead of recovery.
  Dispatch publication and host-turn admission are serialized by the
  scheduler-idle mutex. The scheduler also records each in-flight internal
  capability publication. A new host request for the same capability sets one
  coalesced republish bit instead of starting an independent dispatcher call.
  After the host call returns, the publication owner reacquires the mutex and
  either performs that rebroadcast or retires an `Unavailable` capability while
  publication start is still excluded. A `Failed` completion with an armed
  rebroadcast first resets only the exact registry dispatch from `Cancelling`
  to a fresh pending state after its publication count and host references are
  both zero. Registry failure notifications carry both the internal dispatch
  and the current failure epoch; every successful reset advances that epoch.
  Delayed delivery completion enters the executor with that exact capability.
  While holding the scheduler-idle mutex, the executor briefly locks the
  registry to verify that the same epoch is still `Cancelling`, releases the
  registry lock, and only then mutates executor publication state. The registry
  never calls back into the scheduler while holding its mutex, preserving the
  scheduler-before-registry lock order without retaining either registry state
  or user destructors across the transition. A stale completion can therefore
  neither clear nor terminally cancel a capability after a later host request
  joined and successfully rearmed its owner. Without an armed rebroadcast, the
  owner is removed in the same scheduler-locked transition that validates and
  consumes the failed capability and either reserves its sole replacement or
  claims terminal cancellation. Publication admission cannot observe a
  still-pending, ownerless failed capability between those decisions.
  The failed-dispatch action frame, whether reached from synchronous
  publication failure or later delivery failure, owns an exact
  registry-cancellation guard while it runs that action. Guard cleanup removes
  the registry entry only if both dispatch and epoch still match. If publishing
  the reserved replacement unwinds, contained guard cleanup retires only the
  old `Cancelling` epoch; the replacement remains pending under its original
  identity and can be retried by a later host request.
  The dispatch-call owner catches callback unwind, retains the first panic
  payload in a scoped owner, and services any already-armed rebroadcast before
  resuming that panic. Scheduler resolution, registry validation, replacement
  reservation, terminal cancellation, and every nested replacement publication
  run under a second unwind boundary while that owner remains outside the
  unwind frame. A later panic is safely discarded through the contained
  payload-drop path, including when its destructor panics, and the first payload
  remains authoritative. The scoped owner uses the same contained path if it
  must be abandoned before resume, so a hostile first payload is never dropped
  during a later unwind. Without an armed bit, or once stop or terminal
  cancellation begins, the owner removes its exact publication entry without
  retrying, so a later registration may retry pending work and shutdown cannot
  wait on an ownerless publication. Different capabilities may still publish
  concurrently when a callback consumes an older capability and a bounded-turn
  continuation starts before the older broadcast returns.
  Admission rechecks generation stop while holding the mutex, then consumes the
  pending capability and publishes the draining role before releasing it, so a
  stale stopped-generation callback cannot claim a turn and a wake cannot
  observe the intermediate zero-capability state and publish a duplicate host
  turn. Failed-dispatch recovery uses the same serialization: consuming the
  failed capability, tagging and reserving its sole replacement, and claiming
  that replacement's dispatch-call role are one scheduler-locked transition.
  The controller claims the exact delivery, atomically consumes its mapped
  internal capability, and claims the executor's RAII host-turn role while
  holding the lifecycle mutex; shutdown cannot transition from that
  generation's `Running` state without either preceding admission or observing
  the claimed role. The native `call_js_cb` calls
  `drive_current_thread_tasks`, which returns an opaque callback lease only
  after that exact claim and admission have completed. The callback
  acknowledges or fails the delivery, destroys its queued payload, and only
  then drops the lease so shutdown and restart cannot overlap the tail of an
  old-generation callback. An env-null teardown callback or stale delivery
  reports failure and releases its payload without touching JavaScript.

  `reserveCurrentThreadHostRegistration()` allocates a nonzero split `u64`
  capability without installing a host. The package validates both words
  before passing them to exactly one task- or timer-host registration.
  Registration atomically consumes the reservation and returns `void`, so
  malformed result conversion cannot hide the only rollback authority after
  native side effects. Either unregister function removes an unconsumed
  reservation as well as its matching installed host. A consumed capability
  cannot be reused for another host.
  `registerCurrentThreadTaskHost()` accepts the reserved high/low words plus an
  optional `dispatch?: never` misuse sentinel. The Rust boundary rejects a
  supplied callback synchronously before consuming the reservation or creating
  or invoking a callback. The binding does not export
  `driveCurrentThreadRuntimeTasks` or
  `cancelCurrentThreadRuntimeTaskDispatch`. Asynchronous host returns are
  therefore impossible: no user callback runs, so no Promise/thenable,
  constructor getter, species lookup, or rejection-observation mechanism is
  part of delivery.
  `getCurrentThreadTaskHostContractVersion()` exposes the native ABI version for
  this contract in every runtime profile. `timer-host.ts` first reads the
  binding's normalized runtime capabilities. Every native shared-runtime
  environment installs both hosts proactively, including an import-time
  MultiThread profile, because the still-lazy runtime may be synchronously
  configured to CurrentThread after the side-effect module is cached. Tokio
  builds skip host installation. Before invoking either registration, the
  package validates contract version 4 and the complete reservation,
  registration, liveness, and disposal surface. A binding from the prior
  callback protocol lacks the capability reporter and version export, so it
  still fails with a package/binding mismatch before its callback-accepting
  function can run.
  A truly legacy binding that exposes neither the capability reporter nor any
  async-runtime host export remains a no-op for compatibility. Once a reporter
  identifies a shared-runtime build, a missing host contract fails closed with
  `ERR_ROLLDOWN_BINDING_MISMATCH`. The package also keeps an environment-local
  weak installation registry keyed by the concrete task-host registration
  function. Re-evaluating a generated or cache-busted `timer-host` chunk backed
  by the same binding therefore reuses its task and timer hosts, while another
  native image or WASI instance still receives independent registrations.

  The native threadsafe function has maximum queue size one and is unreferenced,
  so it neither exceeds the registry's physical single-flight slot nor keeps a
  worker event loop alive. Its finalizer owns a weak reference to host state.
  The mutex-protected raw TSFN slot owns exactly the `initial_thread_count = 1`
  acquisition. Environment cleanup marks the host closing, unregisters it, then
  takes and normally releases that owner even if registry eviction panics.
  Explicit registration rollback or a non-closing sweep takes the same owner
  with abort mode. A call returning raw `napi_closing` takes the slot without
  releasing it: Node has already decremented that caller's acquisition and the
  pointer is no longer valid. The finalizer likewise only invalidates the slot
  because it is already running inside Node's destruction path. The custom
  callback always releases a queued delivery payload, including when Node
  invokes it with a null environment during teardown. Finalization, cleanup,
  dispatch closure, and sweep are idempotent and cannot unregister a replacement
  generation or retire the initial owner twice.

  A failure removes only that host attempt. If
  another live delivery or coalesced pending slot still references the internal
  dispatch, recovery is suppressed. The last unserviced failure marks the
  dispatch cancelling before releasing the registry lock, preventing a racing
  registration from attaching to the failed capability, then enqueues at most
  one exact replacement notification. The executor reserves that replacement
  before releasing its scheduler-idle mutex, so a concurrent new registration
  joins the tagged recovery capability through its coalesced republish bit
  instead of publishing an untagged dispatch. Every such host attempt reuses
  the same replacement identity. A failed broadcast honors one newly armed bit
  by reopening and retrying that exact replacement; a subsequent failure
  without another newly armed request retires its publication owner and claims
  terminal cancellation atomically under the same mutex. Terminal cancellation
  temporarily rejects scheduler publications and cancels the affected runnable
  and blocking queues outside the mutex under the generation's existing
  contained-drop rules. This settles the original operation without an
  unrelated wake and then reopens the executor for a later independent dispatch
  chain. Every invocation of the host dispatcher owns a generation-scoped
  scheduler role from before the callback starts until it returns or its
  original panic resumes. One owner may perform multiple coalesced broadcasts
  of the same internal capability under that role. This covers initial queue
  publication, recovery, host replacement, and bounded-turn continuation even
  when shutdown cancels the queued work and retires its generation registration
  concurrently.
  A bounded host turn releases queue-drain exclusivity before requesting its
  continuation, but retains a separate active host-turn role until that host
  dispatch call returns. These roles keep every `Runnable::run`, async-task's
  destruction of detached completed outputs, and all dispatch publication
  inside generation quiescence; CurrentThread shutdown waits for both before
  publishing `Stopped`.
  CurrentThread exposes one physical blocking lane. Uncontended closures and
  same-frame nested calls execute inline. On native builds, contention from a
  different driver creates a stable indexed blocking job and returns its
  `JoinHandle` instead of sleeping inside the task poll. Every CurrentThread
  blocking submission and queued-lane claim rechecks generation stop while
  holding the blocking-admission mutex. Because shutdown publishes stop before
  taking that mutex, a closure either claims or queues for the lane before stop
  publication, or is rejected without executing. Terminal host-dispatch
  cancellation publishes its cancellation flag before taking the same mutex;
  FIFO and exact-owner claims recheck that flag under the lock so cancellation
  cannot race a queued closure into execution. Each native CurrentThread
  `block_on` frame publishes the same dependency context used by MultiThread: if
  its awaited async lineage reaches that queued job, the lexically ambient owner
  frame claims and runs exactly that job without incrementing the active-lane
  metric. It cannot consume an unrelated queued sibling. Ordinary queued work
  is serviced FIFO only after the physical lane is released; release also wakes
  explicit drivers. Outside an admitted host turn, the releasing caller drains
  the FIFO to preserve hostless progress. Inside an admitted host turn, release
  returns to the bounded host-turn driver instead, so queued blocking work
  cannot bypass the 64-unit yield budget; a later host turn continues any
  residue. Threadless builds never have a foreign
  concurrent driver, so their uncontended and same-stack paths remain fully
  inline.

- `MultiThreadExecutor` schedules bounded queue-drain jobs on a custom Rayon
  pool. The same pool is inherited by nested `par_iter` calls. Rayon worker
  start hooks classify every nested worker for cooperative `block_on`; a
  separate driver marker limits the per-worker LIFO slot to scheduler frames
  that will actually drain it. The custom spawn handler retains every native
  worker `JoinHandle`; after pool destruction, shutdown joins those handles so
  physical retirement includes OS-thread termination and thread-local
  destructors, not only Rayon's worker exit hook.
- A second FIFO holds blocking closures. `active_blocking` limits how many
  Rayon workers may block at once. Validation reserves one worker from
  blocking admission. MultiThread promotes a requested worker count of one to
  two, clamps it to the 256-worker production ceiling and
  `rayon::max_num_threads()`, and only then derives
  `max_blocking_tasks <= worker_threads - 1`. The Rayon pool creates exactly
  that validated count; configuration and metrics therefore report physically
  realizable workers, with no hidden reserve. Blocking start/completion counters
  count every executed closure, including exact-dependency work, while
  active/high-water counters count admitted lanes and therefore remain bounded
  by `max_blocking_tasks`.
  Every blocking job has a stable executor-scoped id, and its dependency pairs
  that id with the exact `BlockingOwnerToken` frame whose admitted lane may be
  reused. Pending dependencies propagate through async task handles, acquiring
  the ambient owner frame when they enter an owner's lineage, so a saturated
  owner can lend only to the exact job its nested `block_on` awaits, never an
  earlier detached sibling or another owner's job. Dependency contexts form a
  thread-local stack: polling unrelated scheduler work pushes that task's own
  context above the driving `block_on`, so its blocking waits cannot leak into
  the owner's over-cap lineage. Exact lending requires the matching
  `BlockingOwnerToken` to remain lexically ambient on the cooperative driver.
  The active-owner registry proves only that this exact frame is still active
  and unreserved; its cardinality never infers ancestry for a stolen Rayon job
  or scheduler runnable that has lost the thread-local token.
  `TaskDependency` stores the ordered collection of live dependency
  publications observed during the current poll and its retained waiter in one
  mutex so identity cannot tear across separate atomics. A later pending handle
  in the same poll appends instead of replacing an earlier dependency. Every
  append receives a contiguous, non-reused local sequence id and updates an
  unowned FIFO or exact-owner FIFO plus an indexed job-to-sequence set. Owned
  and propagated publications both create a fresh local liveness link, so
  append is O(1) and does not perform a duplicate scan that could make one poll
  quadratic. Set, clear, conditional clear, claim, and waiter replacement
  commit under that mutex, then wake or destroy moved-out wakers after
  unlocking. The dependency TLS stack also releases its `RefCell` borrow before
  those operations can invoke a waker, so reentrant wake code may enter another
  dependency context. Waker clone, replacement drop, wake, retirement, and
  final destruction run under the dependency generation and independent panic
  boundaries. A lock-free `has_current` hint lets the common dependency-free
  task poll skip that mutex. Propagation creates a fresh local liveness link
  with a non-owning parent edge to each source publication while every hop for
  that publication shares one one-shot exact-job claim.
  Link cancellation and final claim validation/commit serialize through that
  shared claim, giving concurrent withdrawal and lending one winner. Poll start
  invalidates and removes all of the task's prior local links before polling
  user code: ancestor chains become stale immediately, but cancelling an
  intermediate hop cannot cancel the child's source claim. Non-owning edges and
  iterative liveness traversal keep deeply nested join chains from recursively
  traversing or destroying the Rust stack.
  Direct blocking-handle cleanup matches the stable job id, independent of owner
  enrichment. Task detachment snapshots the child's live publications, clears
  its retained waiter before async-task receives detached ownership, then hashes
  the exact job/claim/immediate-parent identities and retires all matching parent
  publications in one parent scan. The batch removes matching job-index entries,
  uses the existing monotonic prefix/FIFO reclamation for owner indexes, and
  consumes at most one parent waiter, avoiding quadratic cleanup when one poll
  observes many child dependencies. Parked-driver entries publish that
  dependency for owner-aware handoff. Selection snapshots only parker and
  dependency `Arc`s while holding the registry mutex, evaluates the targeted
  live-owner predicate after unlocking, then reacquires the registry only to
  grant the selected handoff. Registry removal moves the entry out before
  dropping its `Arc<TaskDependency>` so waiter destruction can re-enter
  scheduler code without holding the registry mutex.
  The blocking FIFO is a queue of stable ids plus an indexed job map. Normal
  admission skips tombstoned ids amortized O(1); exact lending removes the job
  from the map in O(1) after atomically claiming the live dependency. Owner-lane
  availability uses unique reservation identities, so a delayed stale drop
  cannot release a newer transfer of the same frame. Every scheduler identity
  that participates in stale-handle rejection or indexed lookup uses checked
  allocation and fails before `u64` reuse: generations, per-generation task
  keys, executors, blocking jobs and owner frames, lending reservations, host
  driver registrations, and timers. Metrics reset generations likewise check
  both seqlock increments before publishing the odd reset state, so exhaustion
  cannot leave readers spinning or recreate an old progress fingerprint.
- Drain and cooperative loops force a blocking turn after 16 consecutive
  runnable polls. A cooperative `block_on` attempts its live exact owner
  dependency first at that boundary, including an owner-lane transfer while the
  ordinary cap is saturated, then falls back to normal blocking admission. On
  every other cooperative turn, runnable work keeps priority, but if none is
  available the same exact dependency is attempted before ordinary blocking
  FIFO admission. This prevents an unrelated closure from consuming the last
  ordinary slot and waiting synchronously for the still-queued dependency.
  After the cooperative LIFO budget is exhausted, one shared-FIFO pop is
  mandatory even if the next awaited-future poll refills the local slot. One
  dedicated non-Rayon timer thread owns deadline waiting for the generation. It
  executes no runnable or blocking work, so long-lived timers preserve every
  configured Rayon worker and a stalled blocking closure cannot stop timer
  service. Cooperative drivers still bound their own parks by the earliest
  timer and may race the dedicated thread to remove a due heap entry; the heap
  mutex gives exactly one side the waker. Exit compensation treats runnable and
  blocking residue as
  independent obligations: it first hands runnable work to an available driver,
  then, while the generation remains running, a completing blocking-capable
  driver consumes one admitted blocking job itself even when runnable work was
  also present. Handing only the blocking job to a queued Rayon drainer could
  leave no physical lane. Once stop is observable, compensation may still wake
  abort-generated runnables so their generators retire, but it never starts
  queued blocking work. MultiThread shutdown publishes stop before taking the
  blocking FIFO mutex, then closes and drains the queue. Normal and compensation
  admission recheck stop and closure under that mutex, so either a job claim
  linearizes before stop or the job is cancelled.
  Runnable claims use the same stop-publication mutex for both the shared FIFO
  and the worker-local LIFO slot. They release it before running or destroying
  the claimed value. A claim that observes stop performs a generation-scoped,
  panic-contained runnable drop instead of `Runnable::run`, and decrements the
  queued gauge without publishing a poll or active-runnable sample.
  Exact owner lending is performed by the cooperative driver that already owns
  the live dependency lineage. When normal blocking admission is saturated, one
  idle pass checks that dependency, reserves its exact active owner frame, and
  removes only its indexed job. Before consuming the dependency, the exact claim
  rechecks generation stop and queue closure under the blocking FIFO mutex. If
  shutdown published stop before that queue claim, the job remains queued for
  cancellation and the owner reservation is released without starting user
  work. The dependency job otherwise executes on the idle cooperative lane under
  a fresh nested owner frame. No worker-specific broadcast or global dependency
  scan is submitted. One unrelated Rayon worker can therefore remain parked
  indefinitely without blocking later dependencies or scheduler-idle
  retirement. Targeted selection clones only the chosen publication and
  live-owner checks clone none. Exact handoff predicates inspect only that
  owner's FIFO; availability predicates inspect that FIFO and the unowned FIFO.
  Each predicate lazily discards a stale sequence at most once, so interspersed
  stale and wrong-owner publications cannot be rescanned by repeated owner
  probes. Selection compares the two live FIFO heads and thus preserves global
  publication order. Binding the older unowned head moves its sequence to the
  front of the exact-owner FIFO, immediately making a previous negative
  exact-owner predicate stale. Claims use the selected sequence for O(1) entry
  lookup and remove its owner/job indexes; claimed or cancelled entries remain
  sequence-preserving tombstones until prefix reclamation or the next poll
  reset. Repeatedly servicing live publications interspersed with cancelled and
  wrong-owner entries therefore remains amortized linear instead of rescanning
  the shared collection or rebuilding progressively smaller snapshots. Every
  reservation release, including a failed exact claim or an already-removed
  job, wakes at most one parked blocking-capable driver whose published live
  dependency belongs to that exact owner. A newer unrelated or untagged parker
  cannot absorb the handoff, so multiple dependencies of one owner rearm
  linearly without a global wake batch.
  The selected parker stores the owner identity separately from its ordinary wake
  permit. It attempts that handoff before unrelated queue work; a withdrawn or
  replaced publication forwards the identity to the next live same-owner parker,
  an already-reserved owner leaves it to the active transfer's completion, and a
  driver exit forwards any unconsumed identity.
  Park registration is followed by a fresh availability check for the driver's
  lexically ambient exact owner as well as the normal queue recheck. A
  reservation released just before registration therefore causes another claim
  attempt instead of a missed handoff and permanent park.
  A cooperative-driver unwind guard performs the same exit duties on every
  return and user-future panic: deregister the parker, forward an unconsumed
  owner handoff, flush the worker's LIFO slot, and compensate any absorbed queue
  wake. Each duty has an independent panic boundary so one hostile cleanup
  cannot suppress the remaining obligations.
- MultiThread's opt-in park-deadline verdict has a separate admission protocol
  from the relaxed metrics fingerprint. Runnable scheduling, blocking
  scheduling, a LIFO-slot flush, timer registration, and timer firing enter a
  sequentially consistent active admission before changing scheduler-visible
  state. Runnable claims from the shared FIFO or worker-local LIFO slot and
  ordinary or exact blocking claims likewise acquire admission before removal
  and retain it until `runnable_started` or `blocking_started` advances the
  progress fingerprint. Timer firing keeps admission from heap removal through
  waker invocation, including dedicated-timekeeper, cooperative, shutdown, and
  heap drop paths. A timer blocked behind a closed verdict gate therefore
  remains visible in the heap, while removed work cannot become invisible
  before its replacement wake or start accounting is published. Successful
  publication advances a monotonic epoch before the active count retires. After
  all legacy
  permit/queue/fingerprint/timer checks, the final verdict closes an atomic gate.
  If an earlier admission is active, the verdict reopens the gate and retries.
  Otherwise later admissions wait while the driver takes the stop-publication
  mutex and rechecks the permit, both queues, the publication epoch, metrics,
  timers, and generation stop. Exact-owner reservation release and every
  executor-level targeted handoff or forwarding path additionally take a
  detector-only publication mutex before exposing lane availability, retaining
  it through handoff selection and unpark. The verdict takes that mutex after
  stop publication and rechecks its parker's pending handoff plus the live
  dependency's exact owner-lane availability. A release after deadline
  deregistration therefore remains visible even when there was no parker to
  wake, while a selected-but-not-yet-unparked handoff cannot pass the final
  synchronized boundary invisibly.
  Shutdown orders lifecycle mutex before stop publication; the verdict orders
  its gate before stop publication before exact-owner publication. Owner
  publication is outermost to owner-registry, dependency, parked-driver, and
  parker locks, and no reverse path retains one of those locks while acquiring
  it. Direct executor shutdown publishes stop before taking the blocking queue,
  matching the verdict's stop-publication-before-queue order. The driver
  explicitly releases all three guards before `panic_any`; Rust runs panic
  hooks before unwind, and those hooks may synchronously submit scheduler work,
  release an owner lane, or enter lifecycle code. These protocols are entered
  only when `park_deadline` is configured; the default runtime takes disabled
  branches and performs no admission accounting or publication locking.
- `JoinHandle` normalizes async-task, blocking-job, and immediate results and
  detaches async tasks on drop to match Tokio. Scheduler shutdown instead
  aborts accepted async tasks and resolves retained handles with `JoinError`.
  Async-task awaiter registration is given a cached panic-contained proxy,
  never the caller's waker directly. The same proxy is registered for blocking
  dependency propagation, remains stable while the caller's `Waker::will_wake`
  identity is unchanged, and contains both wake and final source-waker
  destruction before either can reach async-task's abort-on-panic boundary.
  Each cached source snapshots the runtime generation active when that caller
  waker is registered, including an explicit no-generation state. Cloning,
  identity comparison, wake, cache replacement, completion cleanup, and handle
  destruction restore that snapshot, so a retained first-generation handle
  cannot run an old `RawWaker` destructor while a replacement generation is
  active.
  Successful values remain generation-tagged until polling transfers ownership
  to the caller. Dropping a blocking or immediate handle is panic-contained
  because its receiver/result may already own a completed user value whose
  destructor unwinds. Task detachment or receiver destruction completes before
  dependency notification, and the arbitrary dependency waker is invoked behind
  a separate containment boundary so two hostile callbacks cannot double-panic.
- MultiThread shutdown waits in three stages: accepted work retirement,
  drainer and dedicated-timekeeper exit, then joined Rayon-worker termination.
  The timer thread is marked with a separate lifecycle identity, joined before
  restart, and never classified as a cooperative pool worker. The final Rayon
  join barrier includes worker TLS destructors, which run after Rayon's exit
  hook. `ON_POOL_WORKER` remains set until OS-thread TLS teardown completes, so
  lifecycle reentry from a retiring worker's TLS destructor is recognized as
  self-wait and rejected. Only after all three stages does the controller
  publish `Stopped` and wake a waiting `start`. A lifecycle call made from a
  task poll, blocking closure, or Rayon worker of the generation being stopped
  returns an error rather than waiting on itself. Queued blocking closures are
  dropped one at a time behind
  `catch_unwind`; a submission that races queue closure is rejected and dropped
  with the same isolation outside the queue lock and under the retiring
  generation identity. Convenience APIs that own a rejected future/closure
  register its contained destruction while holding the lifecycle lock;
  shutdown/restart cannot finish the transition until those registrations
  retire. Public `block_on` performs both the driver call and destruction of its
  erased future inside the same registered generation scope. A failed scope
  acquisition likewise keeps first-backend retry and lifecycle transitions
  closed until contained input destruction retires. CurrentThread blocking
  calls keep their work and generation guards through panic conversion and
  payload destruction. Shutdown timer wakes are isolated too. After
  diagnostics are extracted, caught panic payloads are dropped under a second
  `catch_unwind`; only a nested panic payload from a hostile payload destructor
  is forgotten.
  The binding host-timer adapter applies the same two-stage boundary before
  returning through napi's environment-cleanup C ABI.
  The full blocking result-delivery boundary is also contained: both a send
  after its join handle detached and dropping a receiver with a completed value
  already buffered isolate panic-on-drop results from scheduler retirement.
- Atomic metrics expose task, poll, queue-depth, active-worker, panic, and
  blocking-concurrency counters. Reset clears cumulative event counters only;
  live gauges and lifetime high-water marks remain intact because active guards
  may still need to decrement them. Result delivery can wake a joiner or resolve
  an N-API promise from inside the final runnable/blocking poll, before the
  enclosing active guard retires. An immediate metrics snapshot after awaiting
  an operation may therefore still report a live gauge, and that in-flight work
  may publish completion or poll events after a reset. Lifecycle quiescence or
  polling for guard retirement is required before asserting zero for gauges;
  post-reset event counters describe events published after the reset point. A
  reset generation is part of the deadlock-detector fingerprint, preventing
  repeated counter values across a reset from being mistaken for no progress.
  Snapshot construction clamps each loaded lifetime high-water value to at
  least its corresponding loaded live gauge. This preserves the public
  high-water invariant during the small writer window between incrementing a
  live gauge and publishing its atomic maximum. The N-API
  surface exports counters as JavaScript numbers through the full exact integer
  range (`Number.MAX_SAFE_INTEGER`) instead of saturating at `u32::MAX`; values
  beyond that range clamp at the last exactly representable integer.
- Deadline arithmetic is checked. A configured park duration too large to add
  to `Instant::now()` becomes an unbounded but wakeable condvar wait, and an
  overflowing CurrentThread host-timer grace keeps that timer live. This makes
  arbitrary nonzero `ROLLDOWN_PARK_DEADLINE_MS` values non-panicking.

The binding adapter and JS-facing configuration live in
`crates/rolldown_binding/src/async_runtime.rs`. Configuration sources are:

- `ROLLDOWN_RUNTIME=single|current-thread|multi|multi-thread`
- `ROLLDOWN_WORKER_THREADS`
- `ROLLDOWN_MAX_BLOCKING_THREADS` (retained as the compatibility environment
  variable name; it now caps jobs within the fixed pool)
- `ROLLDOWN_PARK_DEADLINE_MS` (opt-in deadlock detection)
- `configureAsyncRuntime({ flavor, workerThreads, maxBlockingTasks })`, exported
  from `rolldown/experimental`

Configuration must happen before the first async binding call.
`configureAsyncRuntime` converts its optional fields to a
`RuntimeOptionsPatch`; the controller merges that patch into the latest
committed options, validates the complete candidate, and commits it in one
critical section. Omitted fields are preserved, concurrent calls apply in lock
order without stale-snapshot overwrites, and validation failure commits
nothing.

Native `ROLLDOWN_*` worker counts clamp to 256 before either backend can
construct physical workers. Native Tokio's separate blocking-thread limit
clamps to 512, and module initialization checks the combined count before Tokio
performs its internal addition. Native Tokio module initialization registers a
napi-rs custom-runtime factory that captures the process-wide resolved snapshot;
every environment load and explicit lifecycle restart therefore rebuilds Tokio
with the same worker and blocking limits reported by diagnostics. The
environment is not re-read on reload. The JavaScript configuration boundary
rejects `workerThreads` or `maxBlockingTasks` above 256; it does not silently
clamp an out-of-range explicit value into the accepted range. Accepted values
still undergo normal topology validation: CurrentThread becomes `(1, 1)`,
MultiThread promotes one worker to two, applies Rayon's platform cap, and
limits blocking admission to `worker_threads - 1`. The core shared-runtime
validation repeats the worker ceiling so direct Rust callers cannot bypass the
resource bound. Native defaults also take the smaller of physical and
process-available CPU counts before applying backend scaling, so container CPU
limits do not inherit the host's full physical topology. Threaded WASI uses the
generated emnapi loader's separate
`NAPI_RS_ASYNC_WORK_POOL_SIZE`/`UV_THREADPOOL_SIZE` pipeline and its 1024-worker
cap.

The CLI parses and applies `--environment` before importing the timer host or
any binding-backed command module. Runtime environment variables supplied by
that flag are therefore visible to the binding's module-initialization
snapshot, with the same semantics as variables inherited from the parent
process.

On shared WebAssembly builds, the resolver always reports and configures
`CurrentThread`. `ROLLDOWN_RUNTIME=multi` is accepted as an inherited
environment value but normalized before the module-init `configure` call;
otherwise loading a threadless WASI artifact would panic while registering the
addon.

The threaded-WASI binding must link Rust's `crt1-reactor.o` and export
`_initialize`. napi-build locates that startup object relative to Cargo's
`RUSTC`, but task runners may expose either a bare command or the rustup proxy
instead of the real toolchain executable. `packages/rolldown/build-binding.ts`
therefore resolves the active compiler through `rustup which rustc`, with
`rustc --print sysroot` as the non-rustup fallback, before invoking the WASI
build. After napi-rs returns, the same script validates every emitted threaded
artifact as a reactor: `_initialize` must be a function export and `_start`
must be absent. Omitting the reactor can leave package import synchronously
executing malformed startup code, which cannot be interrupted by a JavaScript
promise timeout.

napi-rs generates the Node threaded-WASI loader. The build script patches that
generated loader before it is copied into the package. Each Node or browser
loader evaluation creates an isolated emnapi context instead of reusing the
realm-global default. The postprocessor imports `createContext` directly from
the generated binding's existing `@emnapi/runtime` dependency so this also
works with the published `@napi-rs/wasm-runtime` 1.1.6 facade. Its `destroy()` wrapper calls
`napi_prepare_wasm_env_cleanup()` once after successful preparation, retries a
failed preparation, and does not repeat successful preparation when the
underlying destroy operation is retried. This ordering lets pending N-API
promises reject before emnapi disables JavaScript callbacks and lets a later
loader evaluation replace a destroyed context.

The Node loader's file-backed worker initially removes inherited string-input
flags such as `--input-type`, `--eval`, and `--print` from `process.execArgv`.
If Node rejects additional parent-only flags with
`ERR_WORKER_INVALID_EXEC_ARGV`, worker construction removes only the arguments
named by that error and retries. Accepted runtime flags remain inherited. The
patch deliberately checks the expected napi-rs template and fails the build on
template drift.

The same generated Node loader is the authority for threaded-WASI async-work
pool reporting. It normalizes the selected environment value, caps it at 1024,
passes that integer to emnapi, and writes its canonical decimal form into the
copied WASI environment before instantiation. The Rust reporter therefore sees
the exact pool value for supported package loading. A custom loader that skips
this patch is outside that exact-reporting contract and must canonicalize its
environment consistently if it wants matching diagnostics.

`packages/rolldown/build-binding.ts` snapshots the exact generated binding
surface before invoking napi-rs, including the root `browser.js` facade. A
failure in Rolldown's post-build patching or validation restores every
overwritten generated file and removes only files created by that invocation.
The root facade is managed explicitly rather than by a broad JavaScript-file
pattern, so unrelated sources remain outside the transaction.

The handwritten public facade lives in
`packages/rolldown/src/api/async-runtime.ts`. It exposes `AsyncRuntime*` names,
documents initialization ordering, artifact support, metrics reset semantics,
and WASI restrictions, and keeps deprecated `BindingRuntime*` type aliases for
compatibility. Each facade function validates that its generated binding export
is callable and reports `ERR_ROLLDOWN_BINDING_MISMATCH` when a stale optional
binding is loaded, instead of leaking an unhelpful `is not a function` error.
The generated N-API declarations remain an internal transport detail.

This API is feature-gated. `configureAsyncRuntime`, `getAsyncRuntimeConfig`, and
`getAsyncRuntimeMetrics` are exported on every build, but only the
`async-runtime` build honors configuration. On the default `tokio-runtime`
build `configureAsyncRuntime` throws a feature-disabled error (built without
the `async-runtime` feature), `getAsyncRuntimeConfig` reports values derived
from the environment variables and built-in defaults, and
`getAsyncRuntimeMetrics` always returns zeroed counters.

Tokio resolution distinguishes all three target families so the pure table
remains exhaustive and unit-testable. Native uses the bounded Rolldown-built
multi-thread runtime. `wasm32-wasip1-threads` mirrors the generated loader's
emnapi pool. The table models threadless `wasm32-wasip1` as napi-rs's single
current-thread lane, but `lib.rs` rejects that Tokio-only feature combination
at compile time because napi-rs rejects every built-in async task there.
Threadless artifacts must enable `async-runtime`. The minimal profile is
`--no-default-features --features async-runtime`; leaving default features
enabled and adding `--features async-runtime` is also supported because the
shared backend takes precedence over `tokio-runtime`. CI compiles both profiles.

The dedicated native async-runtime test build also enables
`runtime-submission-failure-test`. Its raw-binding-only stop/start probes shut
down the real scheduler so one `Env::spawn_future` submission rejects before a
retry executes the already-memoized close future. The same fixture verifies
that `BindingWatcher.run()` returns a rejected Promise while stopped, retains
its coordinator, and starts it exactly once after restart. These exports are
absent from production artifacts.

`getRuntimeCapabilities()` also exposes stable public-workflow gates.
`devSupported` follows the effective runtime flavor and is false on
`CurrentThread`; `watchSupported` is false on every WebAssembly artifact. The
TypeScript `runtime-support.ts` layer maps those binding facts to named public
features and throws `ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE` before entering
unsupported setup paths. Missing capability booleans from an older reporter are
normalized from the stable `threads` and `wasi` fields before either support
queries or error construction. If the reporter itself is absent, generated
loaders expose `__rolldownBindingTarget`; compatibility maps `native`, `wasi`,
and `wasi-threads` to conservative complete capability records instead of
assuming every legacy artifact is native. Reports with any other missing,
invalid, or internally inconsistent field fail with
`ERR_ROLLDOWN_BINDING_MISMATCH`; when loader metadata is available, its target
must also agree with the reporter. This prevents malformed threaded-WASI
reports from silently taking the native no-lease path or enabling unsupported
worker-backed features. The layer is intentionally extensible so stacked host
integrations can add richer workflow support without changing the low-level
binding contract. Parallel-plugin descriptor consumption has an additional
synchronous preflight at the public build, rolldown, scan, and dev boundaries
and at `createBundlerOptions`. The latter repeats the preflight immediately
after synchronous `outputOptions` hooks, before normalizing hook-injected
plugins. Each pass recursively inspects already-materialized plugin arrays
without assimilating neighboring thenables, so a fabricated or older-package
descriptor on an unsupported artifact fails before the next asynchronous setup
boundary, worker registry, runtime lease, or binding construction. Ordinary
object plugins do not trigger that gate.

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
the build pending forever. Detached normal- and external-module tasks tolerate
their message receiver disappearing while they await JavaScript hooks: worker
environment teardown owns cancellation, so a late completion or hook error
must retire quietly instead of panicking on a closed loader channel. The real
worker fixture holds both callback kinds, terminates the environment, forces a
full scheduler stop/restart, and verifies that the main realm can build again.

### Deferred destruction

`crates/rolldown/src/utils/defer_drop.rs` owns one process-global serial
maintenance worker. Heavy link-stage values are sent there after generation,
and every build entry calls `drain()` before starting new scan/link/render work.
The worker is deliberately outside Rayon: a rebuild may call `drain()` from a
pool worker while every other execution lane is unavailable, so inheriting
that Rayon registry could deadlock the build against its own maintenance
queue. If the operating system refuses to create the maintenance thread,
deferred destruction falls back to synchronous, panic-contained drops because
moving destruction off the caller is an optimization rather than a correctness
requirement. Worker execution and both synchronous fallback paths use a
two-stage unwind boundary: a deferred destructor panic is caught, then its
panic payload is destroyed behind a second boundary. A hostile payload can
therefore neither kill the worker nor discard queued jobs whose pending counts
would otherwise remain registered forever. The pending count is retired only
after both boundaries complete, so the next build cannot begin while a caught
panic payload is still being destroyed.

### Timers and native watch mode

`rolldown_utils::time::sleep_until` routes watcher debounce timers to Tokio on
the default build and to the shared runtime otherwise. `MultiThreadExecutor`
uses an executor-owned timer heap and one lazily created, lifecycle-managed
non-Rayon timekeeper thread. An unpolled sleep creates no thread; the first heap
registration starts it before inserting the timer entry or queue node. A
thread-creation failure therefore leaves `Sleep::registered` false with no
retained heap waker, and the same sleep may retry without duplicate state. The
thread waits only on the heap's private parker, never runs scheduler queues, and
is joined before the generation can restart. `ensure_started` observes heap
closure before waiting for the handle mutex, recognizes its own timekeeper
thread id, and removes a finished handle before joining it outside that mutex.
TLS teardown can therefore re-enter timer startup without joining itself or
blocking another caller behind a join held under the handle lock.
The deadline queue is an indexed binary min-heap. Each timer id maps to its
current node, so cancellation removes arbitrary non-earliest deadlines in
`O(log n)` instead of accumulating lazy tombstones and paying one unbounded
cleanup pass when a distant earliest timer finally expires.
An arbitrary timer waker may own and release the final direct executor
reference from that timekeeper thread. Heap closure still makes the service
loop exit, but this self-release path detaches its own join handle instead of
attempting to join the current thread.
When the MultiThread park-deadline detector is enabled, the heap shares the
executor's verdict admission state. Registration publishes the heap entry under
admission, and every firing or shutdown-drain path retains admission until all
removed wakers have run. This closes the otherwise invisible interval between
removing a due timer and publishing the runnable produced by its wake.
`CurrentThreadExecutor`
uses the host `TimerDriver` registered by `packages/rolldown/src/timer-host.ts`,
which delegates to paired `setTimeout`/`clearTimeout` callbacks in each
importing environment. Every sleep arms the same timer identity and absolute
deadline on every live driver. The first responsive host wakes the sleep; its
completion cancels all losing arms. A live but CPU-starved environment can
therefore delay only its own redundant arm. Each sleep keys cancellation
ownership by driver registration and reconciles it against a keyed live set,
keeping re-poll work linear while preserving environment eviction and
per-driver contained cancellation. The timer-driver registry likewise stores
registrations in an id-keyed map with a linked oldest-to-newest order. Explicit
environment cleanup and the idempotent unregister performed by a swept host use
expected constant-time lookups, while stable live selection performs several
linear traversals and still invokes liveness, sweep, and destructor callbacks
outside its lock. Dropping a populated local registry likewise destroys each
remaining driver behind an independent panic boundary. Every
polled sleep also publishes
one stable, coalescing repoll signal in its `TimerDriverRegistry`. Driver
registration publishes the new driver first, snapshots those signals, releases
all registry locks, and then wakes them. A concurrently first-polled sleep
therefore either appears in that snapshot or observes the new driver in its
subsequent all-live selection. Re-polling preserves the existing timer id and
absolute deadline, refreshes old arms, adds the new arm, and keeps one pending
entry until completion or cancellation closes the signal and cancels every
losing driver. Registration work is linear in the number of pending sleeps and
coalesces repeated wake requests; no driver callback, arbitrary task waker, or
waker destructor runs under the driver or signal registry locks. The binding
also invokes this core registration without holding its exact-registration
mutex. It publishes the returned id and JavaScript-facing lookup atomically
against eviction afterward, or unregisters the id if a synchronous repoll
already found the new host dead.
Each accepted timer registration owns a
`HostTimerRelay` and its reserved relay id. The relay moves atomically from
waiting-for-arm to armed, cancelled-before-arm, or cancel-sent. Cancellation
before the schedule TSFN runs leaves a cancelled-before-arm tombstone instead
of sending `clearTimeout` too early. Delivery of the schedule callback is the
arm boundary regardless of its result: JavaScript may create the timeout and
then throw or return an invalid non-Promise value. Delivery consumes any
pre-arm tombstone, and every delivered error queues cancellation before the
error is published to the relay task or the pending timer is removed and
woken. Promise rejection follows the same fail-closed ordering. If the relay
task has already dropped its one-shot receiver when delivery arrives, the
sender also cancels the potentially armed timeout. The atomic relay state makes
these competing cancellation paths exact-once, including explicit sleep
cancellation and host eviction.

The relay id remains reserved while any pending entry, queued schedule
delivery, relay task, or queued cancellation payload can still refer to it.
This prevents a late schedule or cancellation callback from aliasing a newer
timer after id wraparound. Cancellation clears the timeout and resolves the
schedule Promise so the detached relay task retires immediately. Timer APIs and
the handle's disposal methods are captured for each schedule and reused across
long-timeout chunks. If the matching `clearTimeout` throws, cancellation tries
the handle's captured `Symbol.dispose`/`close` methods. If those also fail, it
unrefs the handle so even the maximum Node timeout cannot retain the process;
the stale callback is a no-op because its active identity was already removed.
Recovered and fail-safe cancellation errors are reported as structured errors
without escaping the cancellation callback. The JavaScript callback has an
outer containment boundary that rejects the schedule Promise on an unexpected
diagnostic-path failure, and the binding submits cancellation through napi-rs's
catching return-value TSFN variant so even a directly registered callback throw
becomes a Rust diagnostic instead of `napi_fatal_exception`. If no cancellation
or unref mechanism succeeds, the schedule Promise rejects so Rust's bounded
live host failure policy receives the diagnostic instead of stranding the
relay.
If runtime shutdown rejects a newly
created detached relay task before submission, registration removes and wakes
the pending timer before dropping the rejected future and releasing its relay
id. A re-entrant poll therefore cannot observe a stranded entry or reuse the id
while the rejected relay is still observable. Each CurrentThread generation
also retains the armed host wakers; shutdown closes that registry, marks every
sleep fired, wakes active `block_on` calls, and makes later polls resolve while
their host-side timers are cancelled.
MultiThread timer wakes, including shutdown drain-fire, are individually
wrapped with `catch_unwind`; a user-supplied `RawWaker` cannot unwind the
timekeeper or strand shutdown. Owned wake delivery uses `wake_by_ref`, then
drops the waker under a separate containment boundary, so a wake panic and a
destructor panic cannot double-panic in one consuming call. Replaced and
cancelled heap wakers are moved out under the heap mutex, then destroyed with
panic containment after the lock is released, so a waker destructor may safely
re-enter timer cancellation.
CurrentThread host-driver wakes have the same containment, including env
cleanup eviction and panic-payload destruction, so a custom `RawWaker` cannot
unwind through the NAPI cleanup hook or prevent later pending timers from being
drained. Relay failures evict or wake their affected timers before emitting
best-effort diagnostics, and diagnostic formatting/output is independently
panic-contained, so a closed stderr or hostile formatter cannot strand a
timer-host registry. Timer-driver liveness callbacks and sweep hooks are
also panic-contained: a panicking liveness probe is treated as a dead driver
while the stable live snapshot remains armed. Timer-driver callbacks and driver
destruction run without the registry mutex held; selection probes a snapshot
and retries if concurrent registry mutation makes it stale. A reentrant
liveness probe can unregister multiple later drivers and leave the probe
snapshot as their final owner, so each snapshot reference is destroyed behind
its own panic boundary after the registry lock is released. The live result
uses the same contained snapshot ownership, so capability checks,
newest-driver selection, and other consumers cannot aggregate-drop multiple
final driver references after concurrent unregister.
Each CurrentThread sleep records cancellation ownership for the complete live
driver snapshot before invoking any driver's `register` callback. Existing arms
remain in place throughout re-selection, and every new arm enters the owned set
before the first callback runs. An earlier driver may therefore unregister a
later driver and unwind without making that later driver's snapshot reference
its final owner. Snapshot references are also destroyed independently behind
panic containment. A waker-clone or register unwind cannot orphan a previously
armed or partially registered host timer, or double-panic through an unrelated
driver destructor. Completion, stale-driver retirement, and `Sleep` drop
isolate every driver's `cancel` callback and final destruction independently;
one hostile driver cannot skip cancellation of later arms or double-panic an
outer unwind. First-poll repoll and registry wakers are both cloned before
either is published. If the second clone unwinds, the first clone is destroyed
behind a separate containment boundary before the original panic resumes.
The binding rechecks host liveness while holding the pending-relay map mutex.
Eviction publishes the dead latch before taking that same mutex to drain, so a
stale driver snapshot either inserts before the drain and is woken by it, or is
rejected after the drain without publishing a stranded relay. Bulk eviction
also retires each relay through its own contained cancellation and wake
boundaries, and the environment cleanup hook contains the complete eviction, so
one hostile cancellation cannot skip later timers or unwind through N-API.
Every waker passed to an arbitrary timer-driver `register` callback is an
`Arc<Wake>` proxy. Proxy clone never clones the hostile underlying `RawWaker`;
proxy wake and wake-by-reference invoke the underlying wake-by-reference behind
containment, and final proxy destruction drops the underlying waker behind a
separate boundary. A register callback can therefore unwind before retaining
its argument without combining that unwind with a hostile waker destructor.

CurrentThread runnable-host registration uses the same all-live race. Driver
liveness, dispatch, and sweep callbacks run outside the registry mutex and are
panic-contained. Its liveness-probe snapshot likewise destroys each driver
reference independently: reentrant removal of multiple hosts cannot combine
their final destructors into a process-aborting unwind. Live selection and each
planned delivery retain drivers through contained owners. Delivery identities
are planned under the registry mutex, then paired newest-first with those
owners after unlock; normal completion, rejection, callback reentry, and
unwind therefore destroy every driver reference independently without running
a driver destructor under the registry mutex. One internal dispatch is
represented by one distinct delivery per host, and the first responsive
callback atomically consumes the mapped internal capability. If every
environment temporarily disappears, runnables remain queued for the next
registration, an explicit hostless drive, or shutdown cancellation; wake
callers never poll inline. A newly registered host joins the existing dispatch
through a fresh delivery instead of superseding attempts already accepted by
other hosts.

Both internal dispatches and host deliveries use globally unique, non-wrapping
`u64` identities. The opaque delivery remains in the native threadsafe-function
payload. Its custom callback presents that delivery to the registry, which
verifies its registration and attempt, maps it to the internal dispatch, and
lets the controller consume the internal capability while still holding the
lifecycle mutex. This linearizes host-turn admission with
`Running -> Stopping`. Successful admission marks the internal dispatch
serviced and clears matching coalesced pending slots on losing hosts. Their
already queued physical callbacks remain single-flight and return as stale
no-ops.

Unregister removes the host's in-flight and pending registry references. A
still-accepted removed reference is recorded as a failure and starts recovery
only when no other accepting reference for that dispatch remains. A
re-registration receives a new host id and every delivery receives a new
attempt id, so late drive, acknowledgement, or failure calls cannot mutate the
replacement entry. Shutdown clears all pending and per-dispatch bookkeeping but
retains each physical in-flight slot until its native callback completes or its
host unregisters; restarted work coalesces behind that slot rather than
creating a second queued callback.
If installing an environment cleanup hook fails, registration is rolled back
immediately so no driver survives without a teardown owner.
Shutdown closes the queue before dropping pending runnables, so cancellation
retires generation guards without waiting for an unadmitted host callback. A
native callback admitted before shutdown keeps the scheduler role set until it
retires, so shutdown waits even if host execution is delayed after admission.
A stale native callback from an older generation, or a superseded callback from
the same generation, is a no-op and never clears or drains replacement dispatch
state.

Replayable bundle/dev/watch close state retains the original error chain rather
than flattening it to text. A nested `napi::Error` is cloned through napi-rs's
shared exception reference, preserving the original JS error object and its
message/stack/properties for concurrent and late close callers, including WASI
promise rejections. The pinned napi-rs revision also aborts environment tasks
only after releasing its task-registry mutex, because abort synchronously wakes
and drops registrations that re-enter that registry during final env teardown.
Binding close methods return terminal hook and devtools failures as structured
results; a rejected N-API promise is reserved for retryable transport/runtime
failure. TypeScript close coordinators memoize terminal native and listener
results but clear the outer single-flight promise after a transport rejection
or retryable worker/runtime-release failure. Watcher runner outcomes are
observed immediately and record settled errors inside their original
fulfillment/rejection continuation, so a same-turn native-close rejection
cannot overtake diagnostic publication. A successful native close awaits a
still-pending runner before releasing owned resources. Bundle, scan, and dev cleanup
retain the latest workers and runtime lease until native close has delivered a
terminal result. Close attempts identify terminal diagnostics separately from
owned cleanup failures. Internal bounded retries project out already-delivered
terminal diagnostics and retry only retained workers or runtime leases; public
late close calls still replay the memoized terminal result. Watcher cleanup
also tags automatic close attempts: a worker fault discarded by internal
cleanup is retained as an undelivered terminal diagnostic, then replayed by the
first later public attempt while only the still-owned worker is retried. A
public call that joins the automatic attempt receives the fault there and
prevents a duplicate replay on its cleanup retry. The single-flight
promise is published through a deferred microtask
before the cleanup attempt is invoked, so a synchronously reentrant `close()`
observes and returns the original promise instead of starting a second attempt.
Worker stop closures retain only workers whose termination rejected,
so a later close retries unfinished cleanup without terminating successful
workers again. Parallel-plugin pool startup invokes every initializer before
observing any result because each production initializer constructs and
registers its worker synchronously before awaiting bootstrap. The first
bootstrap rejection then requests cleanup of every registered sibling
immediately; it does not wait for another bootstrap Promise that may never
settle. Each supervisor attaches a rejection observer when its bootstrap
Promise is created, before any caller can delay `waitForBootstrap()`, so an
immediate worker `error` or `exit` cannot become a process-level unhandled
rejection. Immediate sibling failures are still collected in thread order,
and later rejections remain observed while physical worker termination
completes.
Terminating a still-bootstrapping supervised worker rejects its bootstrap wait
immediately, so the cancelled initializer releases its retained async frame
even when the worker's exit event arrives in the stopping phase. Physical
`Worker.terminate()` waits on a pool barrier: every constructed worker must
emit an intermediate readiness message over a transferred bootstrap
`MessagePort` after its static binding imports and before plugin initialization,
or exit. The private channel keeps inherited `--import` and loader traffic on
`parentPort` from forging readiness or a terminal bootstrap result. Failed
bootstraps remain referenced until that barrier and physical cleanup complete.
This prevents one worker's cleanup from interrupting napi-rs module
registration in a sibling while preserving termination of a plugin initializer
that never settles.
`RolldownBuild` keeps the latest operation's worker pool alive
when its native build promise rejects because that operation's native
`BundleHandle` still owns `closeBundle`; superseded pools may terminate once a
new native handle has synchronously replaced them. The convenience `build()`
API and `scan()` perform an immediate bounded native-close retry while
retaining their workers and runtime lease. If ownership remains, they schedule
one final retry on a later event-loop turn and await it inside the public
operation. Native-close closures never enter abandoned setup recovery: a
`closeBundle` hook may start nested option setup, which itself waits for setup
cleanup and would otherwise form a native-close/setup cycle. Setup-only
worker/runtime closures remain in the setup recovery registry. Terminal
diagnostics from every native-close attempt are merged by identity and
multiplicity before the operation rejects, so no detached attempt can discover
a diagnostic after settlement. A persistent final transport failure remains
bounded and retains explicit retry ownership without scheduling hidden work.
Error ownership uses invalidatable per-cleanup claims; releasing ownership
severs every retained error-to-cleanup closure immediately. An attempted setup
recovery always reports its rejection even if that attempt released its final
resource.
Watch close-listener reentrancy is scoped through `AsyncLocalStorage`: the
listener's own `close()` receives the completed native phase, while unrelated
callers continue awaiting the full close lifecycle and observe its
listener/runtime result.
`RolldownBuild` and `DevEngine` apply the same owner-scoped rule to every
normalized callback passed into their native objects. A close requested from a
plugin hook, output callback, log callback, or dev callback starts the normal
memoized close lifecycle but returns an immediate acknowledgement to that
callback, allowing native work to release the callback before close waits for
quiescence. External and later close callers still receive the full cleanup
result, including replayed terminal errors and retryable ownership. Node uses
`AsyncLocalStorage` to distinguish the exact async callback. Each context also
carries an active invocation bit that is cleared when the callback settles, so
timers or promises created by that callback cannot retain reentrant-close
privilege after native code has stopped awaiting it. Browser builds have no
async context API. They retain an owner identity until every callback result for
that owner settles, so a build or dev callback may request close, or reject a
failure-close admission cycle, after an async suspension. This cannot
distinguish an unrelated same-owner caller while a callback is active; that
caller may receive the immediate close acknowledgement or admission rejection
instead of the full result. Different owners remain isolated, and later callers
observe the memoized cleanup result after the active callbacks settle. Plugin
normalization extends this scope to user-defined thenables: reading and
synchronously invoking each `then` method is performed inside
`CloseCallbackScope`, as are synchronous `outputOptions` hooks. A thenable that
requests close before returning therefore receives the same acknowledgement
instead of deadlocking normalization against native close.
Each resolution is first boxed in an opaque non-thenable value. Nested thenables
are therefore processed by a later explicit flatten pass under a fresh scope,
instead of being recursively assimilated by the native Promise after the
browser scope has unwound. The boxed promise is awaited outside the synchronous
invocation boundary, so browser hosts do not accidentally grant reentrant-close
privilege to unrelated later microtasks. Each branch retains its own thenable
resolution ancestry, so self-resolving and mutually recursive thenables reject
with a `TypeError` while the same thenable may still appear in independent
plugin-array branches. Array flattening uses the same path-local ancestry rule,
so malformed circular plugin arrays reject without recursive stack overflow
while shared arrays in independent branches remain valid. Callback-return
thenables likewise capture their `then` method once before deferred
assimilation, box each nested resolution, and retain path-local ancestry.
Self-resolving and mutually recursive callback results therefore reject
instead of monopolizing the microtask queue. A data-property thenable may remove
or replace its own `then` before resolving itself, matching native Promise
semantics. Nested `then` accessors are read exactly once under the callback
scope. A non-function result settles as a plain value, a function is invoked
under the same scope, and a thrown getter value is preserved as the rejection
reason.

Watch build results refine that callback scope with an opaque plugin-driver
identity. Each pending close awaited by a close callback records a scoped
dependency edge. Before awaiting another result, Rolldown checks whether the
new edge would complete a cycle, so an A -> B -> A chain receives an immediate
acknowledgement even though N-API may enter B's `closeBundle` callback through a
fresh async context. Acyclic closes continue to await and replay their native
terminal outcome. Active async-context ancestors still handle nested
same-result calls directly. Browser builds retain active watch close identities,
reference-counted per scope, until each callback result settles. A same-result
close after an async suspension therefore remains reentrant without granting
that privilege to another result. Browser hosts still cannot distinguish an
unrelated same-result caller during that active hook window; such a caller
receives the acknowledgement, while calls after the hook settles receive the
memoized terminal result normally.

Native watch mode is supported on both runtime flavors. Public `dev()` checks
`devSupported` before reading callbacks, running plugin hooks, creating workers,
acquiring a runtime lease, or constructing `BindingDevEngine`. Public `watch()`
creates its emitter first, checks `watchSupported` before calling
`createWatcher`, and routes failure through `failSetup`; callers therefore
observe `ERROR` followed by `END`, and `close()` remains usable without any
worker, lease, or native watcher having been created. WASI watch remains
unsupported because entering the native initial build can park the JavaScript
host thread before debounce timers are involved.

### Threaded WASI runtime ownership

Threaded WASI starts with zero Rolldown owners. Every public asynchronous
operation calls the binding's `acquireAsyncRuntime()` export and receives one
`BindingAsyncRuntimeLease` native object. The lease owns exactly one count until
its idempotent `release()` succeeds; its native finalizer is the backstop if
promise delivery, JavaScript setup, or user cleanup abandons the object.
There is no implicit owner shared between JavaScript realms: workers and the
main realm therefore cannot independently claim the same process-global count.

The native manager serializes `Stopped -> Starting -> Running` and
`Running -> Stopping -> Stopped` transitions with a mutex and condition
variable, but drops the mutex before invoking napi lifecycle hooks. Concurrent
acquisitions share one start transition and then retain independent counts.
Only the final lease release calls napi shutdown. Failed start leaves zero
owners; failed shutdown keeps the final lease owned so the same JavaScript
cleanup can retry. Releasing an already released token is a no-op, and
concurrent finalization cannot underflow the count. Environment cancellation
and owner publication are one atomic decision: after a successful start, the
acquisition compare-exchanges its cancellation state from pending to committed
before incrementing the owner count. If cleanup wins that race, the manager
enters `Stopping`, rolls the just-started runtime back, and never exposes a
lease. A rollback failure retains one abandoned lease owner in
`ShutdownFailed`, preserving a recoverable retry path instead of reporting zero
owners for a still-running runtime. One acquisition can first recover such an
abandoned owner and then lose the commit race after starting the replacement
generation, so its shutdown action remains reusable for that second rollback
instead of leaving the manager stuck in `Stopping`.

Restart is awaitable because napi's combined custom/Tokio runtime deliberately
does not overlap Tokio generations. `AcquireAsyncRuntimeTask` runs as N-API
async work, snapshots napi-rs's retirement waiter, and waits on its condition
variable off the JavaScript thread. A fresh waiter is used if another lifecycle
transition creates a newer retirement before start linearizes. The waiter
reports retirement-worker creation or runtime-drop failures as terminal errors
instead of waiting forever, and rejects waiting from the generation that is
retiring. A non-last environment cleanup briefly publishes a napi lifecycle
transition without creating a Tokio retirement generation. If explicit start
meets that transition, the binding retries through a cancellable exponential
condition-variable backoff capped at 16ms instead of hot-spinning an emnapi
async-work thread. The binding installs one cancellation hub per N-API
environment. Environment teardown cancels that environment's pending waiters
and wakes both retirement and transition-backoff waits; it never cancels
retirement itself.

The task returns the native lease token as its output rather than resolving a
bare `Promise<void>`. Ownership therefore remains in Rust across async-work
completion and JavaScript object conversion. If delivery fails, normal Rust or
N-API finalization releases the token. The legacy `startAsyncRuntime` and
`shutdownAsyncRuntime` exports retain a separate manual-owner count for
threaded-WASI compatibility, so an unmatched manual shutdown cannot decrement
a public object's token. On native and threadless-WASI artifacts they remain
successful no-ops for compatibility; automatic N-API environment lifecycle
owns those runtimes. Callable builtin hooks rely exclusively on the outer
native operation token; retaining a manual owner inside their async block would
make environment-teardown cancellation attempt a lifecycle transition from
inside the runtime operation guard.

`packages/rolldown/src/runtime-lifecycle.ts` exposes the awaitable lease
protocol. Build, scan, watch, and dev objects await one lease before native
construction and retain it for their whole lifecycle. Standalone
binding-backed promise utilities (`parse`, `parseAstAsync`, `transform`,
`minify`, isolated declarations, module-runner transforms, callable builtin
hooks, and asynchronous resolver methods) await one lease per invocation.
Overlapping calls therefore own independent native tokens until their own
promises settle.

The TypeScript lease decision is snapshotted once when a package copy loads.
Generated bindings always provide `getRuntimeCapabilities`; incomplete focused
test mocks and legacy development shims that omit it conservatively take the
native no-op lease path instead of throwing during module initialization.
Bindings from the preceding threaded-WASI protocol report
`target: 'wasi-threads'` but do not export `acquireAsyncRuntime`. The TypeScript
layer detects their `startAsyncRuntime`/`shutdownAsyncRuntime` pair and fails
lease acquisition closed. JavaScript realms do not share `globalThis`, so no
realm-local registry can safely consume the protocol's one implicit native
owner. Modern native-token bindings can safely fall back to independent local
managers because every acquisition receives a distinct native token.
A threaded-WASI binding that exposes neither protocol fails acquisition with a
package/binding version-mismatch diagnostic instead of entering native work
without an owner. Both this missing-protocol path and the rejected legacy
implicit-owner path carry `ERR_ROLLDOWN_BINDING_MISMATCH`.
Each acquired value is validated for a callable `release()` method, captured
once with its original receiver, before JavaScript records lease ownership.
Malformed package/binding combinations therefore fail with
`ERR_ROLLDOWN_BINDING_MISMATCH` instead of allowing native work to proceed with
an unreleasable token.
Older capability reports also lack `devSupported`; the public workflow layer
derives it from `threads`, while a shim with no reporter keeps the historical
native MultiThread feature set.

Package copies in one JavaScript realm share a manager through a realm-global
weak registry keyed by the loaded binding's `acquireAsyncRuntime` function
identity. This coalesces failed-release recovery without serializing independent
native token requests; the native manager owns lifecycle transition ordering.
Correctness no longer depends on realm-global state: every realm obtains real
native tokens. Each JavaScript release retries one transient native shutdown
failure before surfacing it, so setup and utility calls without a reusable close
object cannot strand every other realm after a one-shot failure. A persistent
failure stays owned by its lease and can be retried by the same close call; if
that caller abandons the failure, the next acquisition in the same realm retries
retained releases before requesting another token. Native and threadless
artifacts use no-op JavaScript leases, preserving direct binding identities where
no threaded-WASI ownership is required.

The native async-runtime integration suite builds a test-only probe and loads
the raw addon only inside a worker after the public package installs that
environment's normal hosts. A pending shared-scheduler task clones its real
waker to an external native thread. The unreferenced task host allows the worker
to exit naturally; after environment cleanup has returned, the parent releases
that thread, which calls `wake_by_ref`, drops the waker, and publishes
completion. No test-only unregister or forced `Worker.terminate()` masks host
ownership. The parent process never imports the addon, so survival cannot be
explained by another live environment retaining the image. The probe adds no
module-count hooks or lifecycle locks.

The WASI CI lane runs `packages/rolldown/tests/wasi-runtime-lifecycle.mjs`
against the generated threaded artifact. It covers isolated loader contexts,
pending-promise settlement during context cleanup, same-realm reload after
destruction, selective inherited-worker-argument retry, overlapping public owners,
restart after the final release, repeated immediate token reacquisition while
Tokio's previous generation retires, cancellation of a worker environment
whose acquisition is blocked behind retirement, operation and
binding-construction failures, worker realms, a real dev-engine run/close/restart,
fail-closed watch and parallel-plugin capability detection, and duplicate
JavaScript package copies that resolve one shared binding. A user-created Node
worker loads a separate Wasm memory, so it cannot cover the same-image
non-last-environment transition and is not claimed as that regression. The watch
case verifies `ERROR`/`END`, repeated close, and that plugin option hooks never
run. Parallel JavaScript plugins are rejected by both the public factory and
option consumption on WASI because the Rust binding does not consume their
worker registry on wasm targets.
The consumption guard covers descriptors created directly or by an older
package copy and runs before plugin promise assimilation, options hooks,
registry allocation, runtime acquisition, or native construction.
`rolldown()` checks the result of its input-options hook again before lease
acquisition, so a hook cannot inject an unsupported descriptor and leave an
otherwise unusable bundle owner behind. The synchronous descriptor walk tracks
visited arrays, which keeps malformed cyclic plugin lists bounded while still
finding a materialized descriptor elsewhere in the graph. A parent-process
watchdog runs the suite in a child process so a synchronous WASI loader stall
cannot consume the entire CI job without a bounded failure.

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
travel over a transferred `MessagePort`, isolated from inherited preload and loader messages on
`parentPort`; those private messages and worker exit release the
physical-termination barrier because each proves native-addon registration has
finished. Bootstrap failures are normalized to a cloneable
`ParallelPluginBootstrapError` before crossing `postMessage`. If the
control-port send itself fails, the worker closes/unrefs that port and throws
the cloneable diagnostic from a microtask, ensuring the supervisor sees an
`error` or terminal exit even when unhandled promise rejections are configured
to warn instead of terminating the worker. Once a pool is initialized, every
remaining option-access, warning, binding-conversion, and callback-wrapping
step runs inside the same cleanup boundary so a synchronous setup failure
cannot abandon those workers.

### Non-threaded WASI

The current-thread executor is the runtime half of the non-threaded
`wasm32-wasip1` build. Packaging, generated loaders, and the emnapi
memory-growth backport are handled in the dependent browser/WASI change.
That managed workerd entry must register both the runnable task host and timer
host for every independently created instance, including callers of the root
instance factory rather than only the package convenience wrapper. Its task
host clears `pending` if `setTimeout` or another host scheduler throws
synchronously, allowing a later runtime wake to retry dispatch. If
initialization fails and context destruction also fails, object errors retain
cleanup through `cause`; primitive primary failures are combined with cleanup
errors in an aggregate so the unrecoverable cleanup failure is not hidden.

The dependent browser/WASI change will publish the two flavors as distinct
artifact sets:

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
