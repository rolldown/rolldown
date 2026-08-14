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
- Returns a `RolldownWatcher` immediately. The first build is deferred to `process.nextTick` so the caller can attach event listeners first. This matches Rollup's pattern: the constructor calls `process.nextTick(() => this.run())` where `run()` is private.

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
  | { code: 'ERROR'; error: Error; result: RolldownWatchBuild };
```

Event listeners are normally **awaited** before proceeding — blocking semantics matching Rollup.
The coordinator may stop waiting for an `event`, `change`, or `restart` listener only when a close
request wins the close-aware dispatch race described below. The JavaScript callback itself keeps
running, and `watcher.close()` still waits for the complete close sequence.

### Rust API

```rust
let watcher = Watcher::new(configs, handler, &watcher_config)?;
watcher.run();       // spawns the coordinator (non-blocking)
watcher.close().await?;  // sends Close, awaits completion
```

Follows the same `new → run → close` pattern as `DevEngine`. `new()` creates the coordinator future but doesn't spawn it. `run()` spawns it on the tokio runtime. `close()` sets the shared close signal, sends a fire-and-forget `Close` message, and awaits the shared completion future. `wait_for_close()` gives consumers a reliable way to await the watcher's completion without closing it.

### Known Divergences from Rollup

| Aspect               | Rollup                     | Rolldown               | Reason                                                              |
| -------------------- | -------------------------- | ---------------------- | ------------------------------------------------------------------- |
| Bundler per output   | One build, multiple writes | One bundler per output | Architecture constraint — Rolldown's bundler owns the full pipeline |
| `buildStart` calls   | Once per config            | Once per output        | Consequence of one-bundler-per-output                               |
| Module graph sharing | Shared across outputs      | Separate per output    | May change in the future                                            |
| `restart` event      | Per config change          | Per rebuild cycle      | Rolldown emits `restart` once per rebuild cycle                     |

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
```

**Ownership rules:**

- `Watcher` only holds lifecycle state (`tx`, the close signal, and `coordinator_state`) — lightweight, no bundler access.
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
Any ──(Close)──→ Closing → Closed
```

**No explicit Building state.** The coordinator's event loop blocks during build (it `await`s). Fs events buffer in the mpsc channel. After build, `drain_buffered_events()` via `try_recv()` picks them up.

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
            - update_watch_files() again with any render-phase files
         c. handler.on_event(BUNDLE_END or ERROR)
      6. handler.on_event(END)
      7. drain_buffered_events() → process events that arrived during build
```

### Close

```
watcher.close() sends WatcherMsg::Close (fire-and-forget)
  → sets the close flag and notifies any in-progress consumer callback wait
  → awaits shared coordinator future (wait_for_close)
  → handle_close():
      1. State → Closing
      2. task.call_hook_close_watcher() for each task (plugin hook, awaited)
      3. task.close() for each task (bundler cleanup)
      4. handler.on_close() (awaited)
      5. State → Closed
      6. coordinator future completes → all wait_for_close() callers resolve
```

Consumer callbacks are normally blocking, but the coordinator waits for them together with the
dedicated close signal. If an `event`, `change`, or `restart` listener calls and awaits
`watcher.close()`, the close signal wins the wait and the coordinator drops only its Rust-side wait
for that callback. The JavaScript callback and its promise continue running. The coordinator then
runs the normal close hooks, closes every bundler, and emits `close`; only after that does the
original `watcher.close()` promise resolve. This breaks the self-wait cycle without weakening the
meaning of a resolved close promise.

### Error Recovery

Build errors do **not** stop the watcher. On error, `event('ERROR')` is emitted with the error details and a `result` handle. The watcher continues watching — when the user fixes the error and saves, a rebuild triggers.

## Plugin Hooks

All watch-related hooks are **blocking** — the coordinator awaits their completion. This matches Rollup.

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
    fn on_event(&self, event: WatchEvent) -> impl Future<Output = ()> + Send;
    fn on_change(&self, path: &str, kind: WatcherChangeKind) -> impl Future<Output = ()> + Send;
    fn on_restart(&self) -> impl Future<Output = ()> + Send;
    fn on_close(&self) -> impl Future<Output = ()> + Send;
}
```

All methods are awaited during normal operation, ensuring Rollup-compatible sequential semantics.
The `on_event`, `on_change`, and `on_restart` waits are close-aware so an awaited listener can close
its own watcher without deadlocking. `on_close` remains fully awaited as part of the close sequence.

## NAPI Bridge

### Event Handler

`NapiWatcherEventHandler` implements `WatcherEventHandler`, bridging all 4 trait methods to a single JS callback via `ThreadsafeFunction`. Each method wraps its data in a `BindingWatcherEvent` variant and calls `listener.await_call()`, which awaits the JS Promise. During normal dispatch the coordinator therefore blocks until the JS handlers finish; if the close signal wins, the close-aware dispatch wrapper drops only this Rust-side wait as described above.

```rust
struct NapiWatcherEventHandler {
    listener: Arc<MaybeAsyncJsCallback<FnArgs<(BindingWatcherEvent,)>>>,
}

impl WatcherEventHandler for NapiWatcherEventHandler {
    async fn on_event(&self, event: WatchEvent) {
        let binding_event = BindingWatcherEvent::from_watch_event(event);
        self.listener.await_call(FnArgs { data: (binding_event,) }).await;
    }
    // same pattern for on_change (from_change), on_restart, on_close
}
```

`BindingWatcherEvent` wraps an internal enum (`BundleEvent | Change | Restart | Close`) with NAPI-exposed accessor methods (`eventKind()`, `bundleEventKind()`, `bundleEndData()`, etc.) for JS consumption.

### Event Loop Keepalive

`ThreadsafeFunction` uses `Weak = true` (unref'd), so it doesn't prevent Node.js from exiting. `Watcher::wait_for_close()` returns a `Shared<Future>` that resolves when the coordinator finishes — idempotent, so multiple callers (or late callers after completion) all resolve immediately. The NAPI binding exposes this as `waitForClose()` — the pending JS Promise keeps the event loop alive. This replaces the old `setInterval(() => {}, 1e9)` hack.

```
constructor(options, listener)  // creates Watcher with handler, ready to run
run()   → inner.run()           // spawns coordinator (non-blocking)
        → inner.waitForClose()  // pending Promise keeps Node alive
close() → inner.close()         // sends Close msg, awaits shared future
                                // waitForClose() resolves, event loop free to exit
```

### Binding as Thin Wrapper

`BindingWatcher` is intentionally a thin wrapper — it holds a `rolldown_watcher::Watcher` and delegates directly. No state machine, no locking, no logic beyond type conversion. All lifecycle management lives in the Rust core. The constructor takes both `options` and `listener`, creates the `NapiWatcherEventHandler`, and passes it to `Watcher::new()`. Each NAPI method (`run`, `waitForClose`, `close`) is a direct delegation to the inner watcher.

### Event Emitter

`WatcherEmitter` uses a simple `Map<string, Function[]>` for listener storage (on/off). Async `emit()` dispatches handlers sequentially (`for...of` + `await`) so side effects from earlier handlers (e.g. `result.close()` triggering `closeBundle`) are visible to later handlers. No external dependency needed.

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

Environment variables set during watch mode:

```
ROLLUP_WATCH=true    // Rollup compatibility
ROLLDOWN_WATCH=true  // Rolldown-specific
```

## Migration Status

Tracks progress from old watcher → new `rolldown_watcher`. Items link to [#6482](https://github.com/rolldown/rolldown/issues/6482) and related issues.

### NAPI + TypeScript Bridge

- [ ] Surface setup errors (e.g. `options` hook) as `ERROR` events, not unhandled rejections ([#6482](https://github.com/rolldown/rolldown/issues/6482))

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
