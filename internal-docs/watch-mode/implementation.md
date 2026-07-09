# Watch Mode — Implementation

> The design principles and open questions live in [design.md](./design.md).
> This file is the implementation reference.

## Summary

Watch mode monitors source files and automatically rebuilds when changes are detected. The `rolldown_watcher` crate is the foundation, using a clean actor-based architecture. This doc is the authoritative reference for implementing and evolving watch mode.

## API Contract

### TypeScript API (Rollup-aligned)

```typescript
function watch(input: WatchOptions | WatchOptions[]): RolldownWatcher;
```

- Accepts a single config or an array of configs.
- Each config may have multiple `output` entries. Internally, **each output creates a separate bundler** (a `WatchTask`).
- An omitted output and an explicit empty `output: []` both normalize to one
  default output, matching Rollup.
- Config-scoped setup and teardown stay singular for in-process plugins despite
  those per-output bundlers: `options`, `closeWatcher`, and
  `watch.onInvalidate` run once per input config. The first output task owns
  those watch lifecycle callbacks; follower output tasks omit them. Build,
  output, and `watchChange` hooks still run in each output task because their
  plugin contexts are not shared. Parallel plugins still own one worker pool
  per output task, so each pool receives its own `closeWatcher` before worker
  termination.
- Returns a `RolldownWatcher` immediately. The first build is deferred to a zero-delay host timer so the caller can attach event listeners first in Node.js and browsers. `WatcherEmitter.close()` is bound through a deferred handler, so a close in the same creation tick waits for asynchronous option/plugin setup instead of becoming a no-op.

```typescript
interface RolldownWatcher {
  on<E extends keyof RolldownWatcherEventMap>(
    event: E,
    listener: (...args: RolldownWatcherEventMap[E]) => MaybePromise<void>,
  ): this;
  off<E extends keyof RolldownWatcherEventMap>(
    event: E,
    listener: (...args: RolldownWatcherEventMap[E]) => MaybePromise<void>,
  ): this;
  clear<E extends keyof RolldownWatcherEventMap>(event: E): void;
  close(): Promise<void>;
}

type RolldownWatcherEventMap = {
  event: [data: RolldownWatcherEvent];
  change: [id: string, change: { event: ChangeEvent }];
  restart: [];
  close: [];
};

type ChangeEvent = 'create' | 'update' | 'delete';

type RolldownWatcherEvent =
  | { code: 'START' }
  | { code: 'BUNDLE_START' }
  | { code: 'BUNDLE_END'; duration: number; output: readonly string[]; result: RolldownWatchBuild }
  | { code: 'END' }
  | { code: 'ERROR'; error: Error; result: RolldownWatchBuild | null };
```

Event listeners are normally **awaited** before proceeding — blocking semantics matching Rollup.
The coordinator may stop waiting for an `event`, `change`, or `restart` listener only when a close
request wins the close-aware dispatch race described below. The JavaScript callback itself keeps
running, and `watcher.close()` still waits for the complete close sequence. If an awaited listener
rejects before close wins, the coordinator fails closed: it records the original callback error,
runs the normal watcher cleanup sequence, and replays that terminal failure from every current or
later `watcher.close()` call.

### Rust API

```rust
let watcher = Watcher::new(configs, handler, &watcher_config)?;
watcher.run()?;      // submits the coordinator (non-blocking)
watcher.close().await?;  // sends Close, awaits completion
```

Follows the same `new → run → close` pattern as `DevEngine`. `new()` creates the coordinator future but doesn't spawn it. `run()` submits it on the selected runtime and returns `Result<(), WatcherStartError>`. Submission is fallible: if shutdown rejects the task, the exact boxed coordinator future is restored to the not-yet-started state and the typed error is returned, so a call after runtime restart can retry it instead of silently creating a watcher with no coordinator. `close()` first publishes the shared close signal, then performs the same fallible start and awaits the shared completion future; a rejected close-time submission is surfaced instead of reporting a false successful close. The future carries a cloneable terminal close result, so concurrent and later `close()` callers observe the same success or failure. Publishing close before a not-yet-started coordinator prevents a creation-tick close from racing into the initial build. `wait_for_close()` is intentionally completion-only: it keeps Node alive but does not surface the close error through an otherwise ignored promise.

### Known Divergences from Rollup

| Aspect                                       | Rollup                            | Rolldown                             | Reason                                                                                      |
| -------------------------------------------- | --------------------------------- | ------------------------------------ | ------------------------------------------------------------------------------------------- |
| Bundler per output                           | One build, multiple writes        | One bundler per output               | Architecture constraint — Rolldown's bundler owns the full pipeline                         |
| `options` hook lifecycle                     | Once per config build/rebuild     | Once during watcher setup            | Native watch tasks retain materialized binding options and plugin workers across rebuilds   |
| `outputOptions` hook lifecycle               | Once per output per build/rebuild | Once per output during watcher setup | Output options are normalized before constructing each persistent native bundler            |
| `buildStart` calls                           | Once per config                   | Once per output                      | Consequence of one-bundler-per-output                                                       |
| `watchChange` calls                          | Once per config                   | Once per output                      | Each output task owns a distinct live plugin context                                        |
| `onInvalidate` for follower-only watch files | Once per config                   | Not currently emitted                | The first output owns the config callback; native output tasks do not yet share a watch set |
| Parallel `closeWatcher` calls                | Once per config                   | Once per output                      | Each output task owns a separate worker pool                                                |
| Module graph sharing                         | Shared across outputs             | Separate per output                  | May change in the future                                                                    |
| `restart` event                              | Per config change                 | Per rebuild cycle                    | Rolldown emits `restart` once per rebuild cycle                                             |

Rollup recreates a graph from the task's merged config for every build, so its
`options` hooks can replace plugins and other input options on every rebuild,
and each subsequent write reruns `outputOptions`. Rolldown creates persistent
native bundlers and parallel-plugin worker pools during asynchronous watcher
setup. Re-running those hooks would require a rebuild-time NAPI reconfiguration
protocol that atomically replaces native options, plugin drivers, worker pools,
and close ownership; invoking the hooks alone would expose values the active
bundler does not use.

For multiple outputs, in-process `onInvalidate` is attached only to the first
output task to avoid duplicate callbacks for files shared by every output.
Because each native task currently owns an independent file watcher and there
is no config-group event identity, attaching the callback to every task cannot
reliably distinguish one shared filesystem event from separate invalidations.
As a result, a file watched only by a follower output does not currently invoke
the config callback. Correct parity requires grouping output tasks behind one
config-level watch set/event stream rather than timing-based deduplication.

## Architecture

### Actor Pattern

```
Watcher (public API)
  ├── tx: mpsc::Sender ──→ WatchCoordinator (actor, owns everything)
  └── close_notify ──────→ wakes the coordinator while it awaits a consumer callback
                               ├── handler: H (WatcherEventHandler impl)
                               ├── state: WatcherState
                               └── tasks: IndexVec<WatchTaskIdx, WatchTask>
                                    ├── WatchTask 0
                                    │   ├── bundler: Arc<TokioMutex<Bundler>>
                                    │   ├── fs_watcher: DynFsWatcher (owned, per-task)
                                    │   ├── watched_files: FxDashSet<ArcStr>
                                    │   └── needs_rebuild: bool
                                    └── WatchTask N ...

Data flow:
  DynFsWatcher ──(TaskFsEventHandler: maps notify events → FileChangeEvent)──→ WatcherMsg::FileChanges ──→ WatchCoordinator
  WatchCoordinator ──→ dispatch_event / dispatch_change / dispatch_restart
                         └── await_handler_or_close()
                               ├── handler.on_*().await ──→ Consumer (NAPI/Rust)
                               └── close_notify ─────────→ stop the callback wait and run handle_close()
                   └── registration failure → close hidden attempt → delayed bounded task retry
                                               └── exhaustion → handle_close() with replayable error
```

**Ownership rules:**

- `Watcher` only holds lifecycle state (`tx`, the close signal, and `coordinator_state`) — lightweight, no bundler access. The state retains the boxed coordinator future until runtime submission succeeds, and restores it on rejection. `publish_close()` sets the atomic flag, notification, and actor message without spawning; N-API calls it synchronously before returning the close promise so a JavaScript listener cannot return into a new build before close is visible. The async `close()` future enters through the selected runtime, starts a not-yet-running coordinator, and awaits its shared result.
- `WatchCoordinator` owns ALL mutable state. No external mutation.
- Each `WatchTask` owns its `DynFsWatcher`. Per-task watchers mean isolated watch sets and simpler ownership.
- Bundler is `Arc<TokioMutex<>>` because event data structs carry a clone for consumer access (e.g. `BUNDLE_END.result`).

### Three-Layer Stack

```
TypeScript API (packages/rolldown/src/api/watch/)
  ├── watch-emitter.ts   — WatcherEmitter: on/off/clear, dispatches to listeners
  ├── watcher.ts         — createWatcher: options → BindingWatcher, wire close
  └── index.ts           — watch() public function
       ↓
NAPI Bindings (crates/rolldown_binding/src/watcher.rs)
  ├── BindingWatcher     — wraps rolldown_watcher::Watcher
  └── NapiWatcherEventHandler — implements WatcherEventHandler, bridges to JS
       ↓
Rust Core (crates/rolldown_watcher/)
  └── Watcher → WatchCoordinator → WatchTask[] → Bundler
```

### Crate Layout

```
rolldown_watcher/
├── lib.rs                     // Public exports
├── watcher.rs                 // Watcher (public API) + WatcherConfig
├── watch_coordinator.rs       // WatchCoordinator (actor + event loop)
├── watch_task.rs              // WatchTask (bundler + fs watcher) + WatchTaskIdx + BuildOutcome
├── task_fs_event_handler.rs   // TaskFsEventHandler (notify → FileChangeEvent mapping)
├── handler.rs                 // WatcherEventHandler async trait
├── event.rs                   // WatchEvent, BundleStartEventData, BundleEndEventData, WatchErrorEventData
├── file_change_event.rs       // FileChangeEvent (path + kind)
├── watcher_state.rs           // WatcherState enum + transitions
└── watcher_msg.rs             // WatcherMsg enum (FileChanges, Close)
```

## State Machine

```
Idle ──(FsEvent)──→ Debouncing
Debouncing ──(more FsEvents)──→ Debouncing (extend deadline, coalesce changes)
Debouncing ──(timeout)──→ run rebuild sequence → drain buffered → Idle or Debouncing
Task build ──(watch registration failure)──→ delayed retry (25ms, 100ms, 250ms)
Delayed retry ──(success)──→ finish the same public build event cycle
Delayed retry ──(exhausted)──→ Closing with replayable registration error
Any ──(Close)──→ Closing → Closed
```

**No explicit Building state.** The coordinator's event loop blocks during build (it `await`s). Fs events buffer in the mpsc channel. After build, `drain_buffered_events()` via `try_recv()` picks them up.
Registration backoff is likewise a coordinator sub-phase rather than a `WatcherState` variant. It
continues to receive file-change and close messages while waiting, but the retry deadline is fixed
so incoming changes cannot turn a bounded recovery into an indefinite wait.

```rust
enum WatcherState {
    Idle,
    Debouncing { changes: FxIndexMap<String, WatcherChangeKind>, deadline: Instant },
    Closing,
    Closed,
}
```

**Debounce coalescing:** When multiple events arrive for the same path during the debounce window, the change kinds are consolidated rather than using simple last-write-wins. See "Kind Consolidation" below for details. The deadline resets on each new event.

## Debouncing

### Two Layers, One Default

There are two possible debounce layers:

1. **Coordinator-level** (`WatcherState::Debouncing`) — batches file changes across files before triggering a rebuild. Controlled by `buildDelay`. This is the primary mechanism.
2. **Fs-watcher-level** (`notify-debouncer-full`) — deduplicates rapid OS-level events for the same file (e.g. editors that write multiple times per save). Available in `rolldown_fs_watcher` but not used by the watcher.

Only coordinator-level debounce is active by default. This matches Rollup, which implements its own `setTimeout`/`clearTimeout` debounce on top of chokidar (chokidar has no debounce option — only `awaitWriteFinish` for write completion detection).

### Rollup's Approach

Rollup's `buildDelay` option (default: **0ms**) controls a simple timer-reset pattern:

```javascript
// Each file change resets the timer
if (this.buildTimeout) clearTimeout(this.buildTimeout);
this.buildTimeout = setTimeout(() => {
  // emit all accumulated changes, trigger single rebuild
}, this.buildDelay);
```

Changes accumulate in an `invalidatedIds` Map during the delay window — both per-file deduplication and cross-file batching happen in one mechanism. Rollup also applies an `eventsRewrites` table for smarter coalescing (create+delete=null, delete+create=update, etc.).

### Rolldown's Approach

The `WatcherState::Debouncing` state does the same thing with `tokio::select!` and a deadline reset:

- File change → `Idle` becomes `Debouncing { changes, deadline }`
- More changes → deadline resets, changes accumulate with kind consolidation per path
- Deadline fires → if changes are non-empty, passed to `run_build_sequence()`; if empty (all cancelled out by kind consolidation), silently return to Idle
- If a queued file-change message and the deadline are both ready, the coordinator
  consumes the message first and extends the deadline. This prevents polling
  jitter at the debounce boundary from producing an avoidable intermediate build.

#### Kind Consolidation

Like Rollup's `eventsRewrites` table, rolldown consolidates change kinds when multiple events arrive for the same path during a debounce window (`merge_change_kind` in `watcher_state.rs`):

| Existing | New    | Result    | Rationale                                                          |
| -------- | ------ | --------- | ------------------------------------------------------------------ |
| Create   | Update | Create    | File is still new — modification doesn't change that               |
| Create   | Delete | _removed_ | File never existed from the observer's perspective                 |
| Delete   | Create | Update    | File was recreated — net effect is a modification                  |
| _other_  | _any_  | new kind  | Latest kind wins (e.g. Update+Update→Update, Update+Delete→Delete) |

This matters because plugins receive the `WatcherChangeKind` in `watchChange` hooks and may behave differently based on whether a file was created vs. modified.

The fs-watcher layer (`notify-debouncer-full`) is available as an option for users who need OS-level event deduplication (noisy editors, network drives), exposed through `watch.watcher` options (`usePolling` / `pollInterval`). Using both layers adds latency and makes timing harder to reason about, so it's not the default.

### Default Delay

Rollup defaults `buildDelay` to 0ms. The new `rolldown_watcher` defaults to 0ms (`DEFAULT_DEBOUNCE_MS`), matching Rollup.

## Event Lifecycle

### Initial Build

```
Watcher spawns coordinator
  → run_initial_build()
  → on_event(START)
  → per task: on_event(BUNDLE_START) → build → on_event(BUNDLE_END or ERROR)
  → on_event(END)
  → enter event loop (Idle)
```

### File Change → Rebuild

```
File change detected by per-task FsWatcher
  → TaskFsEventHandler sends WatcherMsg::FileChanges
  → process_fs_event():
      - Maps notify EventKind → WatcherChangeKind (Create/Update/Delete)
      - task.invalidate(path) → sets needs_rebuild = true
      - task.call_on_invalidate(path) → fires immediately, before debounce
      - State: Idle → Debouncing, or extends deadline
  → Debounce timer fires (tokio::select!)
  → run_build_sequence(changes):
      1. handler.on_change(path, kind) for each change
      2. task.call_watch_change(path, kind) for each task × each change
      3. handler.on_restart()
      4. handler.on_event(START)
      5. For each task with needs_rebuild:
         a. handler.on_event(BUNDLE_START)
         b. task.build():
            - bundler.with_cached_bundle_experimental(FullBuild, |bundle| { ... })
              1. bundle.scan_modules() → discover module graph
              2. bundle.get_watch_files() → register FS watches (before render, before checking scan result for error recovery)
              3. bundle.bundle_write() or bundle.bundle_generate() (if skip_write)
            - each full build owns independent plugin-driver and emitted-file
              state; a retained earlier result stays valid until its own close
            - update_watch_files() again with any render-phase files
              - every candidate addition is attempted and every `paths_mut()`
                transaction is committed, including after an individual add
                fails or when every path is already registered; the macOS
                FSEvents backend stops delivery when the transaction opens and
                restarts on commit
              - successfully added paths are published only after commit
                succeeds; add and commit diagnostics are aggregated
            - if either registration operation fails, close the unreported
              bundle attempt and retry the task after 25ms, 100ms, then 250ms
              without emitting another `BUNDLE_START`
            - after the third retry fails, stop the coordinator and replay the
              registration failure through `watcher.close()`
         c. handler.on_event(BUNDLE_END or ERROR)
      6. handler.on_event(END)
      7. drain_buffered_events() → process events that arrived during build
```

### Close

```
WatchCoordinator::run()
  → polls run_loop() behind AssertUnwindSafe + FutureExt::catch_unwind
  → normal stop, WatcherMsg::Close, callback-interrupted close, and event-loop panic
    all converge on one handle_close() call
  → a caught panic is recorded first; hostile panic payload destruction is contained
  → handle_close():
      1. State → Closing
      2. task.call_hook_close_watcher() for each task (plugin hook, awaited)
         - if no build created a plugin driver, Bundler creates a temporary
           driver for closeWatcher and discards it without closeBundle
         - errors and Rust panics are recorded; remaining tasks still run
      3. task.close() for each task (final BundleHandle close)
         - errors and Rust panics are recorded; remaining tasks still run
      4. handler.on_close() (awaited; a panic is recorded)
      5. State → Closed
      6. coordinator future completes with the aggregated native result
         - event-loop callback/plugin failures, panics, and cleanup failures share one stable result
         → close() callers resolve/reject identically
         → wait_for_close() callers resolve as a liveness signal

watcher.close() sets the close flag before starting a not-yet-started coordinator,
starts it if necessary, notifies any in-progress consumer callback wait, sends the
fire-and-forget WatcherMsg::Close, and awaits that shared coordinator future.
```

Consumer callbacks are normally blocking, but the coordinator waits for them together with the
dedicated close signal. If an `event`, `change`, or `restart` listener calls and awaits
`watcher.close()`, the close signal wins the wait and the coordinator drops only its Rust-side wait
for that callback. The JavaScript callback and its promise continue running. The coordinator then
runs the normal close hooks, closes every bundler, and emits `close`; only after that does the
original `watcher.close()` promise resolve. This breaks the self-wait cycle without weakening the
meaning of a resolved close promise.

The JavaScript wrapper has two memoized phases. `nativeClosePromise` awaits
`BindingWatcher.close()` and therefore all Rust hooks/coordinator cleanup.
The binding returns a structured result containing both the native close
failures and the opaque close identities of every bundle handle owned by the
coordinator. Rust keeps each close failure separately,
including every diagnostic in a batched failure, and the binding converts each
entry independently. JavaScript exceptions retain their original object
identity on supported N-API hosts. The TypeScript wrapper flattens those
entries into the same outer close coordinator as retained-result, worker,
listener, and runtime-release failures, so one stable terminal `AggregateError`
contains every attempted shutdown phase instead of collapsing all native
failures into one child error.
Structured native failures are terminal results, so the outer `closePromise`
continues by closing every superseded `BUNDLE_END` / `ERROR` result, awaiting
every parallel-plugin worker termination, and dispatching a stable snapshot of
`close` listeners concurrently. A rejected N-API close promise is different:
the binding may already have published native close without delivering its
shared result. JavaScript clears only the transport promise, retains result
handles, workers, listeners, and the runtime lease, and retries the idempotent
native close on the next public close attempt. Teardown continues only after a
structured result establishes native ownership.
Close dispatch awaits every listener with all-settled semantics, aggregates all
listener failures, and clears the complete listener map both before dispatch
and after it settles so listeners added during terminal dispatch cannot survive
shutdown. Each event carries
its internal watch-task index: JavaScript keeps one native-owned current result
per task and moves only the preceding result into its retained-close registry.
Native result closing is therefore the current-result backstop, while the
JavaScript registry ensures every older retained result also runs `closeBundle`
while its workers are alive without reporting a current-result failure twice.
There is a host-microtask window after the `BUNDLE_START` adapter commits a
provisional result transfer but before Rust resumes and constructs the
replacement bundle. Native close identities are the ownership authority for
that window: JavaScript excludes any matching superseded entry from its drain,
because the coordinator has already attempted that exact handle.
An explicitly closed result unregisters itself as soon as its full native close
fulfills, so long-running watchers do not retain successful historical bundle
handles. Failed results stay registered until watcher shutdown can surface
their terminal error. Terminal watcher shutdown accounts for every superseded
result independently even when multiple hooks reuse the same JavaScript error
object, then clears both current and superseded registries before worker and
listener teardown so the public emitter does not retain closed bundle handles.
Native, result, worker, and listener failures are aggregated after all phases
have been attempted (a single failure is rethrown unchanged). The native
`close` event callback merely starts/observes this outer lifecycle and returns
immediately, so the Rust coordinator never waits on a listener that waits on
itself. On Node.js, `AsyncLocalStorage` identifies close-listener continuations:
a reentrant `watcher.close()` returns the already-settled native phase. Close
listener invocations link to their async-context parent, and lookup walks active
ancestors, so mutually closing watcher listeners A -> B -> A acknowledge A
without changing unrelated callers. Outside callers receive the outer promise
and therefore observe listener completion or rejection. Browser hosts do not
expose an equivalent async context. There, each emitter keeps its native-phase
fallback active for the entire close-listener promise so calls after an `await`
cannot self-deadlock, including mutually closing watchers. Calls started before
listener dispatch still hold the outer promise, but an unrelated same-watcher
call made while the listener is active is indistinguishable and receives the
native phase.

`BUNDLE_END` and `ERROR` results adapt their `close()` method through the same
`CloseCallbackScope` used to wrap plugin callbacks. The underlying
`BindingWatcherBundler.close()` is always invoked, preserving `BundleHandle`'s
shared terminal future. Full rebuilds never pre-clear the preceding result:
each build has independent clearable emitted-file state, so an older result's
plugin context remains usable and its eventual close cannot invalidate a newer
build. The internal `BUNDLE_START` payload carries its watch-task index; before
the public event is emitted, the TypeScript adapter moves that task's current
result into a provisional pending-build slot. If the public listener finishes,
the result becomes superseded before native bundle construction starts. If the
listener rejects or starts watcher close, the provisional transfer is canceled
synchronously because native still owns that handle. The binding publishes the
native close request before returning the close promise, so the coordinator sees
close before it can continue from `BUNDLE_START` into bundle construction. Once
bundle construction has replaced the handle, native closes the hidden
replacement and JavaScript closes the previously emitted result. Each plugin
driver has an opaque close identity exposed only through the internal binding
context and native event payload; the public watch result remains close-only.
If `closeBundle` re-enters a result backed by that same driver anywhere in the
active async-context invocation chain, the hook receives an immediate
acknowledgement so it can return. Awaiting another result records a process-wide
dependency edge; an edge that would complete a nested A -> B -> A cycle is
acknowledged even when results belong to different watchers and N-API entered
the nested hook through a fresh async context. Each edge belongs to the source
callback invocation and is removed when either that invocation or the target
close settles, so fire-and-forget closes cannot create stale cycle evidence.
Other cross-result closes await and replay their own terminal result, including
the original JavaScript error object. Node async context distinguishes external
same-result callers, so concurrent and later callers retain the full-result
behavior. Browser hosts retain each active identity until the hook promise
settles, allowing reentry after `await`; an unrelated same-result call during
that window is indistinguishable and receives the acknowledgement, while later
calls replay the full terminal result.

Asynchronous setup failures (for example an `options` hook rejection) are
reported as `ERROR` with `result: null`, followed by `END`, matching Rollup's
pre-build error shape. The emitter is then bound to a terminal close lifecycle,
so a same-tick `close()` cannot remain pending. External close calls are gated
on completion of the terminal report and therefore cannot resolve or dispatch
`close` before `ERROR` and `END` listeners finish. A close called from inside an
`ERROR` or `END` listener starts that same lifecycle but receives a reentrant
nonblocking result, breaking the listener/report self-wait. Close listeners run
only after terminal reporting, so they may await an `END` observation without
forming the inverse wait cycle. Node uses async context to identify those
listener continuations; browser hosts use the same scoped fallback as normal
close-listener reentrancy. A rejected `ERROR` or `END` listener is retained as
part of the terminal setup-close result, so every concurrent or later external
`close()` call replays the same listener failure instead of only logging it.

Setup also uses all-settled option initialization and terminates workers from
every successfully initialized output if another output or native watcher
construction fails. Rejected option initialization can also retain
worker-cleanup ownership; the JavaScript setup path adopts those closures
together with fulfilled-option workers and the runtime lease under one
retryable cleanup owner. Cleanup is retried once immediately. If it still
fails, the owner remains in the shared pending-cleanup registry so later
parallel-plugin initialization can recover the workers or lease instead of
discarding them with the setup error. The registry and retry coalescing live in
the platform-neutral `utils/retryable-cleanup.ts`; keeping them separate from
worker startup prevents browser watch builds from retaining Node worker-thread
code.

### Error Recovery

Build errors do **not** stop the watcher. On error, `event('ERROR')` is emitted with the error details and a `result` handle. The watcher continues watching — when the user fixes the error and saves, a rebuild triggers.

## Plugin Hooks

All watch-related hooks are **blocking** — the coordinator awaits their completion. This matches Rollup.
`watchChange` is also fail-closed: a rejected hook stops the rebuild sequence, enters the normal
cleanup path, and becomes part of the replayable watcher close result instead of being logged and
discarded.

| Hook                         | When                           | Purpose                                                |
| ---------------------------- | ------------------------------ | ------------------------------------------------------ |
| `watchChange(id, { event })` | After debounce, before rebuild | Let plugins react to file changes (cache invalidation) |
| `closeWatcher()`             | During watcher close           | Let plugins clean up resources                         |

Plugin context additions in watch mode:

- `this.meta.watchMode` — `true` when running in watch mode
- `this.addWatchFile(id)` — Add a file to the watch set (not in module graph)

### onInvalidate Callback

Configured via `WatcherOptions`, fires **immediately** on file change (before debounce completes). Unlike `watchChange`, this fires per-event, not per-build-cycle.

## File Watching

- After each build, `bundler.watch_files()` returns the current set.
- `WatchTask::update_watch_files()` diffs against the current set — new files are added to the per-task `DynFsWatcher`.
- `include`/`exclude` patterns filter which files are watched (via `pattern_filter`).
- Files are watched **non-recursively** (individual file watches).
- Batch operations: `fs_watcher.paths_mut()` returns a guard for batching adds, committed via `.commit()`.
- Opening a path transaction may pause event delivery until commit. Every eligible candidate is
  attempted and every transaction is committed, including when an individual `PathsMut::add`
  fails or the batch is empty. If commit succeeds, only paths whose additions succeeded enter
  `watched_files`; if commit fails, none of the staged paths are published. Add and commit
  diagnostics are aggregated when both occur.
- Any add or commit failure aborts that build attempt; neither is logged and skipped. The
  coordinator closes the unreported bundle handle and retries the whole task with
  25ms/100ms/250ms backoff inside the same public `BUNDLE_START` cycle. Close remains interruptible
  during backoff. Exhaustion fails closed and the registration diagnostics become part of the
  stable result replayed by every `watcher.close()`.
- `BUNDLE_END.output` follows Rollup's `path.resolve(output.file || output.dir)` behavior. Relative
  paths are resolved against the normalized bundler `cwd`, and `.` / `..` components are removed
  lexically without requiring the output path to exist.

### Split-Phase Build for Watch Mode

`Bundler::write()` runs scan → render → write atomically. But the watcher needs to register FS watches for discovered files BETWEEN scan and write — otherwise changes made during render hooks (e.g. `renderStart` modifying a file) are missed because the FS watcher isn't watching yet.

The watcher uses `Bundler::with_cached_bundle_experimental()` to get `&mut Bundle` access, allowing manual orchestration of the build phases:

1. **Scan** — `bundle.scan_modules()` discovers module graph and populates watch files
2. **Watch registration** — `bundle.get_watch_files()` → register FS watches BEFORE render hooks fire.
   This happens before checking the scan result — so files are watched even on scan error.
   This is critical for error recovery: if a user introduces a syntax error, the watcher must
   still be watching the broken file so that saving a fix triggers a rebuild.
3. **Write/Generate** — `bundle_write()` or `bundle_generate()` (if `skip_write`)

This matches the legacy watcher's approach (`with_cached_bundle`), where `watch_files()` was called between scan and write phases.

### Missing File Recovery

When an import resolves to a non-existent file, the build errors. Watch mode relies on the resolver cache being cleared before each rebuild (`bundler.clear_resolver_cache()`). The expected recovery workflow is: create the missing file, then manually edit a watched file (e.g. noop edit to the importer) to trigger a rebuild. The resolver re-evaluates the import with a fresh cache and succeeds. This matches Rollup's behavior — Rollup only watches successfully loaded modules.

### Notify Event Mapping

```
notify::EventKind::Create(_)                              → WatcherChangeKind::Create
notify::EventKind::Modify(Name(RenameMode::To))           → WatcherChangeKind::Create
notify::EventKind::Modify(Name(RenameMode::Both))         → per-path (see below)
notify::EventKind::Modify(Name(RenameMode::From))         → WatcherChangeKind::Delete
notify::EventKind::Remove(_)                              → WatcherChangeKind::Delete
notify::EventKind::Modify(_)  (other)                     → WatcherChangeKind::Update
notify::EventKind::Access(_)                              → None (ignored — prevents infinite rebuild loops on Linux)
```

**Rename handling:** Linux inotify can emit `Modify(Name(Both))` when both source and destination are known in a single rename event. This event carries two paths `[from, to]`. The event handler splits it into two `FileChangeEvent`s: `Delete` for the source path and `Create` for the destination path. This preserves both signals — the delete ensures stale cache entries are invalidated, and the create triggers missing-dir rebuilds. `RenameMode::To` and `RenameMode::From` are the single-path equivalents.

**Access filtering:** The build process reads watched source files, which on Linux triggers `IN_OPEN`/`IN_CLOSE_NOWRITE` events. Without filtering, these cause infinite rebuild loops.

### Path Identity

The watch set stores paths as raw `ArcStr` strings. The `notify` crate reports events with OS-native paths. If these don't match exactly, `is_watched_file()` fails silently. The current `#[cfg(windows)]` backslash fallback is a symptom.

**Recommendation:** Use `PathBuf` for the watched file set instead of `ArcStr`. This handles trailing slashes, double separators, `.` segments, and Windows `\` vs `/` — all common mismatch sources between resolver output and notify events.

See [module-id.md](../module-id/implementation.md) for the full analysis of path identity across the bundler, `PathBuf` comparison behavior, and Rollup's approach.

## WatcherEventHandler Trait

The single extension point for consumers. NAPI implements it to bridge to JS; Rust consumers implement directly.

```rust
pub trait WatcherEventHandler: Send + Sync {
    fn on_event(&self, event: WatchEvent) -> impl Future<Output = anyhow::Result<()>> + Send;
    fn on_change(&self, path: &str, kind: WatcherChangeKind) -> impl Future<Output = anyhow::Result<()>> + Send;
    fn on_restart(&self) -> impl Future<Output = anyhow::Result<()>> + Send;
    fn on_close(&self) -> impl Future<Output = anyhow::Result<()>> + Send;
}
```

All methods are awaited during normal operation, ensuring Rollup-compatible sequential semantics.
The `on_event`, `on_change`, and `on_restart` waits are close-aware so an awaited listener can close
its own watcher without deadlocking. An error returned before close wins terminates the event loop
and is aggregated into its replayable close result. `on_close` remains fully awaited as part of the
close sequence, and its returned error is aggregated with earlier lifecycle failures.

## NAPI Bridge

### Event Handler

`NapiWatcherEventHandler` implements `WatcherEventHandler`, bridging all 4 trait methods to a single JS callback via `ThreadsafeFunction`. Each method wraps its data in a `BindingWatcherEvent` variant and calls `listener.await_call()`, which awaits the JS Promise. During normal dispatch the coordinator therefore blocks until the JS handlers finish; if the close signal wins, the close-aware dispatch wrapper drops only this Rust-side wait as described above.

```rust
struct NapiWatcherEventHandler {
    listener: Arc<MaybeAsyncJsCallback<FnArgs<(BindingWatcherEvent,)>>>,
}

impl WatcherEventHandler for NapiWatcherEventHandler {
    async fn on_event(&self, event: WatchEvent) -> anyhow::Result<()> {
        let binding_event = BindingWatcherEvent::from_watch_event(event);
        self.listener.await_call(FnArgs { data: (binding_event,) }).await?;
        Ok(())
    }
    // same pattern for on_change (from_change), on_restart, on_close
}
```

`BindingWatcherEvent` wraps an internal enum (`BundleEvent | Change | Restart | Close`) with NAPI-exposed accessor methods (`eventKind()`, `bundleEventKind()`, `bundleStartData()`, `bundleEndData()`, etc.) for JS consumption. `BUNDLE_START` carries the internal watch-task index used for result ownership transfer. Bundle result payloads carry the same task index and plugin-driver close identity beside the close-only `BindingWatcherBundler`; the TypeScript adapter consumes all internal metadata before emitting the public event.

### Event Loop Keepalive

`ThreadsafeFunction` uses `Weak = true` (unref'd), so it doesn't prevent Node.js from exiting. The coordinator is a `Shared<Future>` carrying the terminal native close result. `Watcher::close()` replays that result, while `Watcher::wait_for_close()` deliberately discards it and resolves when the coordinator finishes. The NAPI binding exposes the latter as `waitForClose()` — the pending JS Promise keeps the event loop alive without creating an unhandled rejection. This replaces the old `setInterval(() => {}, 1e9)` hack.
The TypeScript runner still awaits that completion-only promise: an unexpected
N-API transport rejection is recorded as a watcher run failure and enters the
normal fail-closed cleanup path instead of becoming an unhandled rejection.
The `run()` / `waitForClose()` outcome is observed from creation and stores its
settled diagnostics in the same fulfillment or rejection continuation. A
native close failure in the same microtask turn therefore cannot overtake the
bookkeeping and omit the transport error. A successful native close waits for
any still-pending runner outcome before releasing workers or the runtime lease;
a retryable native transport failure reports only already-settled runner
diagnostics and lets the next close attempt await the rest.

```
constructor(options, listener)  // creates Watcher with handler, ready to run
run()   → inner.run()           // submits or rejects; never silently succeeds
        → inner.waitForClose()  // pending Promise keeps Node alive
close() → inner.close()         // sends Close msg, awaits shared future
                                // waitForClose() resolves, event loop free to exit
```

### Binding as Thin Wrapper

`BindingWatcher` owns no independent lifecycle state. It converts the flattened
output configs into native configs, applies the maximum `buildDelay`, selects
the first config with explicit watcher-backend settings for the shared native
watcher, and creates the `NapiWatcherEventHandler`. `run()` and
`waitForClose()` delegate directly. Shared-runtime builds attempt `run()`
before entering a N-API future, allowing a stopped scheduler to return an
already-rejected JavaScript Promise while retaining the native coordinator for
an explicit retry. Tokio builds perform the same checked call inside the N-API
runtime context they require. `close()` publishes close synchronously,
then returns a structured result containing every native close failure and the
close identities owned by the native coordinator.

The coordinator distinguishes a public `close()` from cleanup started
automatically after `run()` rejects or native `CLOSE` arrives. If an automatic
attempt encounters a retryable worker termination failure, its internally
discarded promise retains that diagnostic separately from worker ownership. A
later public close retries only the still-owned workers and replays the
previously undelivered fault. If a public caller joins the automatic attempt
before it settles, that promise delivers the fault directly and the later
cleanup retry does not replay it a second time.

### Event Emitter

`WatcherEmitter` uses a simple `Map<string, Function[]>` for listener storage
(on/off). Every dispatch snapshots the applicable listener array before calling
user code, so listeners added or removed during an event affect only later
events. Normal async `emit()` remains sequential (`for...of` + `await`) so side
effects from earlier handlers (e.g. `result.close()` triggering `closeBundle`)
are visible to later handlers. Terminal `emitClose()` uses the all-settled
behavior described above. The emitter also owns a deferred close-handler
Promise so `close()` is valid before `createWatcher()` finishes asynchronous
plugin setup. The bound `Watcher` remains the authority for native/full-phase
memoization. Close-listener reentrancy uses `AsyncLocalStorage` on Node.js and a
per-emitter active-listener fallback in browser builds; the browser fallback
intentionally prioritizes no deadlock over distinguishing unrelated calls
during the listener promise. No external dependency is needed.

Setup failures are reported as `ERROR` then `END` before an external same-tick
`close()` can finish. Errors from another JavaScript realm retain their original
object identity; non-Error thrown values are wrapped with `cause`, including a
non-coercible fallback that cannot disrupt terminal reporting.

### Event Mapping

Lives in `watcher.ts` (`createEventCallback()` — a standalone function), not in the emitter. The callback is created before the `BindingWatcher` constructor and passed to it alongside options. Maps `BindingWatcherEvent` → Rollup-compatible event objects. Error events carry structured `Vec<BuildDiagnostic>` data from Rust; the binding preserves these diagnostics, and the JS layer converts them via `aggregateBindingErrorsIntoJsError()` before exposing them on Rollup-style event objects.

### End-to-End Flow

```
WatchCoordinator.run_build_sequence()
  → dispatch_event(WatchEvent::BundleEnd(data))
    → await_handler_or_close(handler.on_event(...))
      ├── callback branch:
      │     → NapiWatcherEventHandler.on_event()
      │       → BindingWatcherEvent::from_watch_event(event)
      │       → listener.await_call(binding_event).await → ThreadsafeFunction calls JS
      │     → JS: createEventCallback() receives BindingWatcherEvent
      │       → Maps to RolldownWatcherEvent { code: 'BUNDLE_END', ... }
      │       → emitter.emit('event', mapped_event) → sequential for...of await
      │     → await_call resolves → coordinator continues
      └── close branch:
            → close_notify resolves → dispatch returns close requested
            → coordinator runs handle_close() and completes the close sequence
```

## Configuration

```typescript
interface WatcherOptions {
  skipWrite?: boolean; // Skip bundle.write(). Default: false
  buildDelay?: number; // Debounce ms. Default: 0
  watcher?: {
    usePolling?: boolean; // Use polling backend. Default: false
    pollInterval?: number; // Polling interval ms. Default: 100
  };
  notify?: { ... }; // Deprecated — use `watcher` instead
  include?: StringOrRegExp | StringOrRegExp[];
  exclude?: StringOrRegExp | StringOrRegExp[];
  onInvalidate?: (id: string) => void;
  clearScreen?: boolean; // Clear screen on rebuild. Default: true
}
```

Across enabled input configs, the native coordinator uses the largest
`buildDelay`, as Rollup does. Backend/polling settings configure one shared
native watcher, so the first config that explicitly provides such settings is
authoritative. The multiple-watcher-option warning counts input configs, not
their expanded output tasks; multiple outputs from one config therefore do not
produce a false warning.

Environment variables set during watch mode:

```
ROLLUP_WATCH=true    // Rollup compatibility
ROLLDOWN_WATCH=true  // Rolldown-specific
```

## Migration Status

Tracks progress from old watcher → new `rolldown_watcher`. Items link to [#6482](https://github.com/rolldown/rolldown/issues/6482) and related issues.

### NAPI + TypeScript Bridge

- [x] Surface setup errors (e.g. `options` hook) as `ERROR` events with `result: null`, then `END`, without leaking partially initialized parallel-plugin workers ([#6482](https://github.com/rolldown/rolldown/issues/6482))

### Cleanup

- [ ] Remove `reset_closed_for_watch_mode()` hack — see [rust-bundler.md](../rust-bundler/implementation.md) for the `Bundle.close()` design that replaces it
- [ ] Rename `WatcherChangeKind` → `FileChangeEventKind` (type stays in `rolldown_common`)
- [ ] CLI `--watch` mode working with new watcher ([#7759](https://github.com/rolldown/rolldown/issues/7759))

### Missing Features

- [x] Resolver cache invalidation between rebuilds ([#6482](https://github.com/rolldown/rolldown/issues/6482)) — `clear_resolver_cache()` called at start of each rebuild
- [ ] File unwatching — `update_watch_files()` only adds, never removes. Watch set grows monotonically

### Future

- [ ] Non-blocking builds — spawn builds instead of inline `await` (see Unresolved Questions)
- [ ] Incremental builds — `WatchTask::build()` currently does full rebuild via `bundler.write()`
- [ ] Parallel task builds within a single coordinator
- [ ] Bulk-change threshold optimization — For bulk changes (e.g. `git checkout` producing 1000+ file events), we could skip per-file `on_change`/`watchChange` hooks and just do a full rebuild. Rollup doesn't do this — it always calls per-file hooks regardless of volume. This is a potential future optimization if per-file hook overhead becomes a performance issue.

## Related

- [design.md](./design.md) — watch-mode design principles and open questions
- [rust-bundler](../rust-bundler/implementation.md) — Core Bundler struct and `Bundle.close()` design
- [rust-classic-bundler](../rust-classic-bundler/implementation.md) — Rollup API compatibility wrapper
- [module-id](../module-id/implementation.md) — Module ID, path identity, and normalization
- [#6482](https://github.com/rolldown/rolldown/issues/6482) — Watch mode issue collection (tracks all known bugs)
- `crates/rolldown_watcher/` — Implementation
- `crates/rolldown_fs_watcher/` — File system watching abstraction over `notify`
- `crates/rolldown_dev/` — Dev mode, uses same actor pattern for reference
- `packages/rolldown/src/api/watch/` — TypeScript API layer
