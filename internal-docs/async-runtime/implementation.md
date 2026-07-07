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
Rolldown returns it untouched when the controller is stopped. The optional
`AsyncRuntime::spawn_blocking` hook follows the same ownership rule for its
closure. Accepted tasks and closures retain napi-rs's cancel-on-drop guards.
Generated-task submission and custom runtime lifecycle operations delegate to
the registered implementation; `start` and `shutdown` report failures through
`napi::Result`. `AsyncRuntime` is an unsafe implementation contract because
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
than return with callable unmapped code. A failed module load still rolls back
without retention. The backend object is reused across zero-environment
shutdown/start cycles and its `Drop` is not guaranteed to run, so
`AsyncRuntime::shutdown` must still release and quiesce all active resources.
The `unsafe impl AsyncRuntime` comment in
`crates/rolldown_binding/src/async_runtime.rs` records both halves of this
contract. Rolldown's pinned napi-rs revision must include that retention
implementation before this code can ship; the unsafe-trait change deliberately
makes an older revision fail to compile instead of producing an unpinned
binary.

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
  unwound.
- The owned Rust `block_on` helper acquires its generation work scope before
  moving the input future into the driver. Acquisition failure registers
  rejected destruction under the controller lifecycle mutex, then destroys the
  future behind the same panic boundary and reentrancy context used by
  convenience spawn rejection. Successful driving wraps the future in
  `ContainedFuture`, so cancellation, an early stop, or a poll panic cannot
  combine with a panicking future destructor and abort the native process.
  The borrowed NAPI backend hook remains covered independently by napi-rs's
  synchronous runtime-use gate and `SafeDrop` wrapper.
- `CurrentThreadExecutor` uses a reentrancy-safe FIFO runnable queue. In a host
  embedding, a wake requests a fresh host turn before polling: futures such as
  `futures::Shared` invoke outer wakers while holding internal locks, so polling
  inline from the scheduler callback can re-enter the same future and
  self-deadlock. The Node binding registers one weak threadsafe-function-backed
  task driver per environment; an accepted dispatch is coalesced until the host
  calls `drive_current_thread_tasks`. This registration is also present in the
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
  the exact blocking dependency published by the awaited future has priority
  over ordinary FIFO work on that turn. Once shutdown is observed after a
  pending poll, it takes precedence over queue draining and stored self-wake
  permits. A nonzero pending dispatch coalesces later wakes before they allocate
  another process-global identity; callers that race after both observing zero
  still resolve through the exact-capability compare-exchange. Every accepted
  host dispatch carries a globally unique, nonzero capability.
  Dispatch publication and host-turn admission are serialized by the
  scheduler-idle mutex. Admission consumes the pending capability and publishes
  the draining role before releasing that mutex, so a wake cannot observe the
  intermediate zero-capability state and publish a duplicate host turn.
  The controller atomically consumes that exact capability and claims the
  executor's RAII host-turn role while holding the lifecycle mutex; shutdown
  cannot transition from that generation's `Running` state without either
  preceding admission or observing the claimed role. If the JavaScript
  scheduler throws before it can queue the fresh turn,
  `cancel_current_thread_task_dispatch` clears only the matching capability. It
  never polls from the failing callback. Instead it enqueues at most one exact
  replacement notification through the host driver. Successful drive or host
  replacement clears that recovery capability. If the replacement is also
  cancelled, or no live host accepts it, the executor temporarily rejects
  scheduler publications and cancels the affected runnable and blocking queues
  under the generation's existing contained-drop rules. This settles the
  original operation without an unrelated wake and then reopens the executor
  for a later independent dispatch chain. Every invocation of the host
  dispatcher owns a generation-scoped scheduler role from before the callback
  starts until it returns. This covers initial queue publication, recovery,
  host replacement, and bounded-turn continuation even when shutdown cancels
  the queued work and retires its generation registration concurrently.
  A bounded host turn releases queue-drain exclusivity before requesting its
  continuation, but retains a separate active host-turn role until that host
  dispatch call returns. These roles keep every `Runnable::run`, async-task's
  destruction of detached completed outputs, and all dispatch publication
  inside generation quiescence; CurrentThread shutdown waits for both before
  publishing `Stopped`.
  CurrentThread exposes one physical blocking lane. Uncontended closures and
  same-frame nested calls execute inline. On native builds, contention from a
  different driver creates a stable indexed blocking job and returns its
  `JoinHandle` instead of sleeping inside the task poll. Each native
  CurrentThread `block_on` frame publishes the same dependency context used by
  MultiThread: if its awaited async lineage reaches that queued job, the
  lexically ambient owner frame claims and runs exactly that job without
  incrementing the active-lane metric. It cannot consume an unrelated queued
  sibling. Ordinary queued work is serviced FIFO only after the physical lane
  is released; release also wakes explicit drivers, and a host turn may claim a
  serviceable released lane. Threadless builds never have a foreign concurrent
  driver, so their uncontended and same-stack paths remain fully inline.
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
  hidden reserve. Blocking start/completion counters count every executed
  closure, including exact-dependency work, while active/high-water counters
  count admitted lanes and therefore remain bounded by `max_blocking_tasks`.
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
  `TaskDependency` stores its live dependency and retained waiter in one mutex
  so identity cannot tear across separate atomics. Set, clear, conditional
  clear, claim, and waiter replacement commit under that mutex, then wake or
  destroy moved-out wakers after unlocking. Waker clone, replacement drop, wake,
  retirement, and final destruction run under the dependency generation and
  independent panic boundaries. A lock-free `has_current` hint lets the common
  dependency-free task poll skip that mutex. Propagation creates a fresh local
  liveness link with a non-owning parent edge to the source publication while
  every hop shares one one-shot exact-job claim. Link cancellation and final
  claim validation/commit serialize through that shared claim, giving concurrent
  withdrawal and lending one winner. Poll start invalidates and removes the
  task's prior local link before polling user code: ancestor chains become stale
  immediately, but cancelling an intermediate hop cannot cancel the child's
  source claim. Non-owning edges and iterative liveness traversal keep deeply
  nested join chains from recursively traversing or destroying the Rust stack.
  Direct blocking-handle cleanup matches the stable job id, independent of owner
  enrichment. Task detachment clears the retained waiter before
  async-task receives detached ownership. Parked-driver entries
  publish that dependency for owner-aware handoff, but registry removal moves
  the entry out before dropping its `Arc<TaskDependency>` so waiter destruction
  can re-enter scheduler code without holding the registry mutex.
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
  ordinary cap is saturated, then falls back to normal blocking admission.
  After the cooperative LIFO budget is exhausted, one shared-FIFO pop is
  mandatory even if the next awaited-future poll refills the local slot. The
  timer timekeeper uses a sticky
  runnable-only scheduler role, including through nested `block_on`; the nested
  cooperative loop fires due timers before every runnable turn, so a permanently
  hot stream cannot suspend the outer timekeeper loop and starve the timer heap.
  A stalled blocking closure cannot stop timer service either. Parked-driver
  registration records whether a parker may consume blocking work; blocking
  submissions and blocking exit compensation skip the runnable-only
  timekeeper. Exit compensation treats runnable and blocking residue as
  independent obligations: it first hands runnable work to an available driver,
  then, while the generation remains running, a completing blocking-capable
  driver consumes one admitted blocking job itself even when runnable work was
  also present. Handing only the blocking job to a queued Rayon drainer could
  leave no physical lane. Once stop is observable, compensation may still wake
  abort-generated runnables so their generators retire, but it never starts
  queued blocking work. MultiThread shutdown closes and drains the blocking
  FIFO and publishes the stop flag while holding the FIFO mutex; normal and
  compensation admission check that same state under the mutex, so either a job
  is claimed before shutdown or it is cancelled.
  Exact owner lending is performed by the cooperative driver that already owns
  the live dependency lineage. When normal blocking admission is saturated, one
  idle pass checks that dependency, reserves its exact active owner frame, and
  removes only its indexed job. The dependency job executes on the otherwise
  idle cooperative lane under a fresh nested owner frame. No worker-specific
  broadcast or global dependency scan is submitted. One unrelated Rayon worker
  can therefore remain parked indefinitely without blocking later dependencies
  or scheduler-idle retirement, and large dependency topologies require one
  bounded claim attempt per serviced dependency rather than O(N^2) cloned
  snapshots multiplied by the worker count. Every reservation release, including
  a failed exact claim or an already-removed job, wakes at most one parked
  blocking-capable driver whose published live dependency belongs to that exact
  owner. A newer unrelated or untagged parker cannot absorb the handoff, so
  multiple dependencies of one owner rearm linearly without a global wake batch.
  The selected parker stores the owner identity separately from its ordinary wake
  permit. It attempts that handoff before unrelated queue work; a withdrawn or
  replaced publication forwards the identity to the next live same-owner parker,
  an already-reserved owner leaves it to the active transfer's completion, and a
  driver exit forwards any unconsumed identity.
  Park registration is followed by a fresh availability check for the driver's
  lexically ambient exact owner as well as the normal queue recheck. A
  reservation released just before registration therefore causes another claim
  attempt instead of a missed handoff and permanent park.
- `JoinHandle` normalizes async-task, blocking-job, and immediate results and
  detaches async tasks on drop to match Tokio. Scheduler shutdown instead
  aborts accepted async tasks and resolves retained handles with `JoinError`.
  Successful values remain generation-tagged until polling transfers ownership
  to the caller. Dropping a blocking or immediate handle is panic-contained
  because its receiver/result may already own a completed user value whose
  destructor unwinds. Task detachment or receiver destruction completes before
  dependency notification, and the arbitrary dependency waker is invoked behind
  a separate containment boundary so two hostile callbacks cannot double-panic.
- MultiThread shutdown waits in three stages: accepted work retirement,
  drainer/timekeeper exit, then physical Rayon worker exit. Only after all
  three stages does the controller publish `Stopped` and wake a waiting
  `start`. A lifecycle call made from a task poll, blocking closure, or Rayon
  worker of the generation being stopped returns an error rather than waiting
  on itself. Queued blocking closures are dropped one at a time behind
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
  an operation may therefore still report a live gauge; lifecycle quiescence or
  polling for guard retirement is required before asserting zero. A reset
  generation is part of the deadlock-detector fingerprint, preventing repeated
  counter values across a reset from being mistaken for no progress.
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
- `configureAsyncRuntime({ flavor, workerThreads, maxBlockingTasks })`, exported
  from `rolldown/experimental`

Configuration must happen before the first async binding call.
`configureAsyncRuntime` converts its optional fields to a
`RuntimeOptionsPatch`; the controller merges that patch into the latest
committed options, validates the complete candidate, and commits it in one
critical section. Omitted fields are preserved, concurrent calls apply in lock
order without stale-snapshot overwrites, and validation failure commits
nothing.

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
generated loader before it is copied into the package so its file-backed worker
removes inherited string-input flags such as `--input-type`, `--eval`, and
`--print` from `process.execArgv`. Those flags describe the parent process's
input source and can make Node reject `wasi-worker.mjs`; other runtime flags
remain inherited. The patch deliberately checks the expected napi-rs template
and fails the build on template drift.

This API is feature-gated. `configureAsyncRuntime`, `getAsyncRuntimeConfig`, and
`getAsyncRuntimeMetrics` are exported on every build, but only the
`async-runtime` build honors them. On the default `tokio-runtime` build
`configureAsyncRuntime` throws a feature-disabled error (built without the
`async-runtime` feature), `getAsyncRuntimeConfig` reports values derived from the
environment variables and built-in defaults, and `getAsyncRuntimeMetrics` always
returns zeroed counters.

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
the build pending forever.

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
uses an executor-owned timer heap and timekeeper role. `CurrentThreadExecutor`
uses the host `TimerDriver` registered by `packages/rolldown/src/timer-host.ts`,
which delegates to paired `setTimeout`/`clearTimeout` callbacks in each
importing environment. The Rust relay records whether the JS schedule callback
has returned before sending cancellation, preventing cancel from overtaking
timeout creation. Cancellation clears the timeout and resolves the schedule
Promise so the detached relay task retires immediately. A host-provided
`clearTimeout` is allowed to throw, but that exception is contained inside the
JavaScript cancel callback because it crosses N-API through a non-catching
TSFN. The schedule Promise is still resolved in `finally`, so a host cleanup
failure cannot escape as a fatal exception or strand the Rust relay during
shutdown. Each CurrentThread generation also retains the armed host wakers;
shutdown closes that registry, marks every sleep fired, wakes active `block_on`
calls, and makes later polls resolve while their host-side timers are
cancelled.
MultiThread timer wakes, including shutdown drain-fire, are individually
wrapped with `catch_unwind`; a user-supplied `RawWaker` cannot unwind the
timekeeper or strand shutdown. Replaced and cancelled heap wakers are moved out
under the heap mutex, then destroyed with panic containment after the lock is
released, so a waker destructor may safely re-enter timer cancellation.
CurrentThread host-driver wakes have the same containment, including env
cleanup eviction and panic-payload destruction, so a custom `RawWaker` cannot
unwind through the NAPI cleanup hook or prevent later pending timers from being
drained. Relay failures evict or wake their affected timers before emitting
best-effort diagnostics, and diagnostic formatting/output is independently
panic-contained, so a closed stderr or hostile formatter cannot strand a
sleep. Timer-driver liveness callbacks and sweep hooks are also panic-contained,
matching the runnable-host registry: a panicking liveness probe is treated as a
dead driver and selection falls back to another live host. Timer-driver
callbacks and driver destruction run without the registry mutex held;
selection probes a snapshot and retries if
concurrent registry mutation makes it stale.

CurrentThread runnable-host registration follows the same newest-live-driver
model. Driver liveness, dispatch, and sweep callbacks run outside the registry
mutex and are panic-contained. If every environment temporarily disappears,
runnables remain queued for the next registration, an explicit hostless drive,
or shutdown cancellation; wake callers never poll inline. A newly registered
host supersedes any pending dispatch because an accepted weak
threadsafe-function call may have been discarded when its previous environment
died; duplicate host callbacks are harmless because the executor serializes
queue draining. Dispatches carry a globally unique, non-wrapping `u64`
capability as an exact high/low `u32` pair, avoiding both JavaScript number
precision loss and a higher minimum N-API version. The exported drive callback
must present that same capability; the controller consumes it from the current
executor and claims the drain role while still holding the lifecycle mutex.
This linearizes host-turn admission with `Running -> Stopping`.
If installing an environment cleanup hook fails, registration is rolled back
immediately so no driver survives without a teardown owner.
Shutdown closes the queue before dropping pending runnables, so cancellation
retires generation guards without waiting for an unadmitted host callback. A
callback admitted before shutdown keeps the scheduler role set until it
retires, so shutdown waits even if host execution is delayed after admission.
A stale callback from an older generation, or a superseded callback from the
same generation, is a no-op and never clears or drains replacement dispatch
state.

Replayable bundle/dev/watch close state retains the original error chain rather
than flattening it to text. At the NAPI boundary, a nested `napi::Error` is
cloned through napi-rs's shared exception reference, preserving the original JS
error object and its message/stack/properties for concurrent and late close
callers. The pinned napi-rs revision also aborts environment tasks only after
releasing its task-registry mutex, because abort synchronously wakes and drops
registrations that re-enter that registry during final env teardown.
TypeScript close coordinators memoize terminal native and listener results but
clear the outer single-flight promise after retryable worker or runtime-release
failures. The single-flight promise is published through a deferred microtask
before the cleanup attempt is invoked, so a synchronously reentrant `close()`
observes and returns the original promise instead of starting a second attempt.
Worker stop closures retain only workers whose termination rejected,
so a later close retries unfinished cleanup without terminating successful
workers again. `RolldownBuild` keeps the latest operation's worker pool alive
when its native build promise rejects because that operation's native
`BundleHandle` still owns `closeBundle`; superseded pools may terminate once a
new native handle has synchronously replaced them. The convenience `build()`
API performs one bounded retry when its hidden bundle still owns worker or
runtime cleanup; a persistent failure is registered with the shared
retryable-cleanup owner instead of being discarded with the hidden bundle.
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
async context API, so only the exact callback invocation synchronously on the
stack receives the acknowledgement. Unrelated close callers always await full
cleanup and observe its failure; a browser callback must request close before
its first async suspension. Plugin normalization extends this scope to
user-defined thenables: reading and synchronously invoking each `then` method is
performed inside `CloseCallbackScope`, as are synchronous `outputOptions`
hooks. A thenable that requests close before returning therefore receives the
same acknowledgement instead of deadlocking normalization against native close.
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
instead of monopolizing the microtask queue, and a user-controlled getter is
never observed twice merely to determine whether the callback is asynchronous.

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
owners for a still-running runtime.

Restart is awaitable because napi's combined custom/Tokio runtime deliberately
does not overlap Tokio generations. `AcquireAsyncRuntimeTask` runs as N-API
async work, snapshots napi-rs's retirement waiter, and waits on its condition
variable off the JavaScript thread. A fresh waiter is used if another lifecycle
transition creates a newer retirement before start linearizes. The waiter
reports retirement-worker creation or runtime-drop failures as terminal errors
instead of waiting forever, and rejects waiting from the generation that is
retiring. The binding installs one cancellation hub per N-API environment.
Environment teardown cancels that environment's pending waiters and wakes tasks
blocked behind another native transition; it never cancels retirement itself.

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
without an owner.
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
the raw addon only inside a worker. A pending shared-scheduler task clones its
real waker to an external native thread. After the worker is terminated and its
environment cleanup has returned, the parent releases that thread; it calls
`wake_by_ref`, drops the waker, and publishes completion. The parent process
never imports the addon, so survival cannot be explained by another live
environment retaining the image. The probe adds no module-count hooks or
lifecycle locks.

The WASI CI lane runs `packages/rolldown/tests/wasi-runtime-lifecycle.mjs`
against the generated threaded artifact. It covers overlapping public owners,
restart after the final release, repeated immediate token reacquisition while
Tokio's previous generation retires, cancellation of a worker environment whose
acquisition is blocked behind that retirement, operation and
binding-construction failures, worker realms, a real dev-engine
run/close/restart, fail-closed watch and parallel-plugin capability detection,
and duplicate JavaScript package copies that resolve one shared binding. The
watch case verifies `ERROR`/`END`, repeated close, and that plugin option hooks
never run. Parallel JavaScript plugins are rejected by both the public factory
and option consumption on WASI because the Rust binding does not consume their
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
clears that logical owner. Bootstrap pools await every startup attempt before
taking their cleanup snapshot, so a late-registering sibling cannot escape
termination after another sibling fails. Bootstrap failures are normalized to
a cloneable `ParallelPluginBootstrapError` before crossing `postMessage`. If
the control-port send itself fails, the worker closes/unrefs that port and
throws the cloneable diagnostic from a microtask, ensuring the supervisor sees
an `error` or terminal exit even when unhandled promise rejections are
configured to warn instead of terminating the worker. Once a pool is
initialized, every remaining option-access, warning, binding-conversion, and
callback-wrapping step runs inside the same cleanup boundary so a synchronous
setup failure cannot abandon those workers.

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
