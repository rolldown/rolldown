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
  Full configuration, partial merge/validation/commit, start, shutdown, and
  submission use the same mutex. A racing submission cannot recreate the
  backend after shutdown, and concurrent partial updates cannot read the same
  stale options snapshot and overwrite one another.
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
  async-task's abort-on-panicking-drop boundary. Task outputs remain in an
  equivalent wrapper until a live `JoinHandle` extracts them, covering detached
  completion as well as cancellation.
- `CurrentThreadExecutor` uses a reentrancy-safe FIFO runnable queue. In a host
  embedding, a wake requests a fresh host turn before polling: futures such as
  `futures::Shared` invoke outer wakers while holding internal locks, so polling
  inline from the scheduler callback can re-enter the same future and
  self-deadlock. The Node binding registers one weak threadsafe-function-backed
  task driver per environment; an accepted dispatch is coalesced until the host
  calls `drive_current_thread_tasks`. This registration is also present in the
  browser build because fresh-turn polling is a future/scheduler requirement,
  independent of Node timers. Wakes are enqueue-only even when no host is
  registered; pure Rust use makes progress through an explicit `block_on` or
  `drive_current_thread_tasks` call. A host turn polls at most 64 runnables
  before redispatching, so a self-waking task cannot monopolize the JavaScript
  event loop. Blocking work executes inline.
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
  Every blocking job has a stable executor-scoped id copied into its
  `JoinHandle`. Pending dependencies propagate through async task handles, so a
  saturated blocking owner can lend its lane to the exact job its nested
  `block_on` awaits, never an earlier detached sibling. Dependency contexts form
  a thread-local stack: polling unrelated scheduler work pushes that task's own
  context above the driving `block_on`, so its blocking waits cannot leak into
  the owner's over-cap lineage. Dependency transitions and abandoned handles
  clear and wake parent contexts.
- Drain and cooperative loops force a blocking turn after 16 consecutive
  runnable polls when the blocking FIFO has capacity. After the cooperative
  LIFO budget is exhausted, one shared-FIFO pop is mandatory even if the next
  awaited-future poll refills the local slot. The timer timekeeper uses a sticky
  runnable-only scheduler role, including through nested `block_on`, so a
  stalled blocking closure cannot stop timer service. Parked-driver registration
  records whether a parker may consume blocking work; blocking submissions and
  exit miss compensation skip the runnable-only timekeeper. A completing
  blocking-capable driver consumes one admitted blocking-only residue itself
  when handing it to a queued Rayon drainer could leave no physical lane.
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
  with the same isolation outside the queue lock and under the retiring
  generation identity. Convenience APIs that own a rejected future/closure
  register its contained destruction while holding the lifecycle lock;
  shutdown/restart cannot finish the transition until those registrations
  retire. Shutdown timer wakes are isolated too. After diagnostics are
  extracted, caught panic payloads are dropped under a second `catch_unwind`;
  only a nested panic payload from a hostile payload destructor is forgotten.
  The binding host-timer adapter applies the same two-stage boundary before
  returning through napi's environment-cleanup C ABI.
  The full blocking
  result-delivery boundary is also contained: dropping a panic-on-drop result
  after its join handle detached cannot bypass blocking-slot or drainer
  retirement and strand shutdown.
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
`configureAsyncRuntime` converts its optional fields to a
`RuntimeOptionsPatch`; the controller merges that patch into the latest
committed options, validates the complete candidate, and commits it in one
critical section. Omitted fields are preserved, concurrent calls apply in lock
order without stale-snapshot overwrites, and validation failure commits
nothing.

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
queue. If the operating system refuses to create the maintenance thread,
deferred destruction falls back to synchronous, panic-contained drops because
moving destruction off the caller is an optimization rather than a correctness
requirement.

### Timers and native watch mode

`rolldown_utils::time::sleep_until` routes watcher debounce timers to Tokio on
the default build and to the shared runtime otherwise. `MultiThreadExecutor`
uses an executor-owned timer heap and timekeeper role. `CurrentThreadExecutor`
uses the host `TimerDriver` registered by `packages/rolldown/src/timer-host.ts`,
which delegates to paired `setTimeout`/`clearTimeout` callbacks in each
importing environment. The Rust relay records whether the JS schedule callback
has returned before sending cancellation, preventing cancel from overtaking
timeout creation. Cancellation clears the timeout and resolves the schedule
Promise so the detached relay task retires immediately. Each CurrentThread
generation also retains the armed host wakers; shutdown closes that registry,
marks every sleep fired, wakes active `block_on` calls, and makes later polls
resolve while their host-side timers are cancelled.
MultiThread timer wakes, including shutdown drain-fire, are individually
wrapped with `catch_unwind`; a user-supplied `RawWaker` cannot unwind the
timekeeper or strand shutdown. Replaced and cancelled heap wakers are moved out
under the heap mutex, then destroyed with panic containment after the lock is
released, so a waker destructor may safely re-enter timer cancellation.
CurrentThread host-driver wakes have the same containment, including env
cleanup eviction and panic-payload destruction, so a custom `RawWaker` cannot
unwind through the NAPI cleanup hook or prevent later pending timers from being
drained. Timer-driver liveness callbacks and sweep hooks are also
panic-contained, matching the runnable-host registry: a panicking liveness
probe is treated as a dead driver and selection falls back to another live
host. Timer-driver callbacks and driver destruction run without the registry
mutex held; selection probes a snapshot and retries if
concurrent registry mutation makes it stale.

CurrentThread runnable-host registration follows the same newest-live-driver
model. Driver liveness, dispatch, and sweep callbacks run outside the registry
mutex and are panic-contained. If every environment temporarily disappears,
runnables remain queued for the next registration, an explicit hostless drive,
or shutdown cancellation; wake callers never poll inline. A newly registered
host supersedes any pending dispatch because an accepted weak
threadsafe-function call may have been discarded when its previous environment
died; duplicate host callbacks are harmless because the executor serializes
queue draining.
If installing an environment cleanup hook fails, registration is rolled back
immediately so no driver survives without a teardown owner.
Shutdown closes the queue before dropping pending runnables, so cancellation
retires generation guards without waiting for a host callback. A stale callback
from an older generation is also harmless: it either finds no running
CurrentThread executor or services the current generation as an extra host
turn.

Replayable bundle/dev/watch close state retains the original error chain rather
than flattening it to text. At the NAPI boundary, a nested `napi::Error` is
cloned through napi-rs's shared exception reference, preserving the original JS
error object and its message/stack/properties for concurrent and late close
callers. The pinned napi-rs revision also aborts environment tasks only after
releasing its task-registry mutex, because abort synchronously wakes and drops
registrations that re-enter that registry during final env teardown.
TypeScript close coordinators memoize terminal native and listener results but
clear the outer single-flight promise after retryable worker or runtime-release
failures. Worker stop closures retain only workers whose termination rejected,
so a later close retries unfinished cleanup without terminating successful
workers again. Watch close-listener reentrancy is scoped through
`AsyncLocalStorage`: the listener's own `close()` receives the completed native
phase, while unrelated callers continue awaiting the full close lifecycle and
observe its listener/runtime result.

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

The native count and `packages/rolldown/src/runtime-lifecycle.ts` are one
protocol. The native `ASYNC_RUNTIME_LEASES` count starts at one while
`WasiRuntimeLeaseManager` starts with `#initialLeaseAvailable = true`: its first
JS lease consumes the implicit native owner without calling
`startAsyncRuntime`, then releases it with `shutdownAsyncRuntime`. After that
count reaches zero, every later JS lease calls `startAsyncRuntime` before it can
release. Build, scan, watch, and dev objects each own one lease for their whole
lifecycle. Standalone binding-backed promise utilities (`parse`,
`parseAstAsync`, `transform`, `minify`, isolated declarations, module-runner
transforms, and asynchronous resolver methods) own one lease per invocation.
Overlapping calls therefore retain independent owners until their own promises
settle, and a call after the final release restarts the stopped runtime. The
added direct-binding wrappers are selected only for threaded WASI; native
isolated-declaration, module-runner, and `ResolverFactory` identities remain the
binding exports. Native and threadless artifacts receive no-op leases.

Package copies in one JavaScript realm share the manager through a realm-global
weak registry keyed by the loaded binding's `startAsyncRuntime` function
identity. Copies backed by the same binding therefore cannot each consume its
single implicit owner, while distinct bindings remain independent. A failed
release stays owned by its lease and can be retried by the same close call; if
that caller abandons the failure, the next acquisition retries every retained
release before starting another owner. Changing the native initial count to
zero requires changing the JS first-acquire path in the same change; otherwise
the first release is a native no-op and leaves the runtime running without a
tracked owner.

Parallel-plugin workers are supervised from construction through shutdown, not
only until their bootstrap message. Delayed worker `error` events and
unexpected exits are retained as close failures instead of becoming uncaught
parent-process events. A supervisor that has already exited does not physically
terminate again, but rejects one cleanup attempt with its retained fault so the
existing retryable-cleanup protocol preserves ownership; the next attempt
clears that logical owner. Bootstrap pools await every startup attempt before
taking their cleanup snapshot, so a late-registering sibling cannot escape
termination after another sibling fails.

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
