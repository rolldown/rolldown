# Watch Mode

## Summary

Watch mode monitors source files and automatically rebuilds when changes are detected. The `rolldown_watcher` crate is the foundation, using a clean actor-based architecture. This doc is the authoritative reference for implementing and evolving watch mode.

## Design Principles

- **JS API aligns with Rollup** — The TypeScript surface (events, options, plugin hooks, lifecycle ordering) should match Rollup's behavior unless there's a technical reason not to. Divergences are documented explicitly.
- **Rust code follows Rust idioms** — The Rust core should feel native: ownership-driven, enum state machines, trait-based extensibility, no unnecessary `Arc`/`Mutex` beyond what the architecture requires.
- **Consistent naming across the stack** — Rollup defines the canonical event/concept names (e.g. `BUNDLE_START`/`BUNDLE_END`). The Rust side should use the same terminology so there's a clean 1:1 mapping and no mental translation at the NAPI boundary.

## API Contract

### TypeScript API (Rollup-aligned)

```typescript
function watch(input: WatchOptions | WatchOptions[]): RolldownWatcher;
```

- Accepts a single config or an array of configs.
- Each config may have multiple `output` entries. Internally, **each output creates a separate bundler** (a `WatchTask`).
- Returns a `RolldownWatcher` immediately. The first build is deferred to `process.nextTick` so the caller can attach event listeners first.

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

All event listeners are **awaited** before proceeding — blocking semantics matching Rollup.

### Rust API

```rust
let watcher = Watcher::new(config, handler, &watcher_config)?;
let watcher = Watcher::with_multiple_bundler_configs(configs, handler, &watcher_config)?;
watcher.close().await?;
```

`Watcher::new` spawns the coordinator actor and triggers the first build immediately. The caller provides a `WatcherEventHandler` implementation to receive events.

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
  └── tx: mpsc::Sender ──→ WatchCoordinator (actor, owns everything)
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
  WatchCoordinator ──(handler.on_event().await)──→ Consumer (NAPI/Rust)
```

**Ownership rules:**

- `Watcher` only holds `tx` and `task_handle` — lightweight, no bundler access.
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
    Debouncing { changes: Vec<FileChangeEvent>, deadline: Instant },
    Closing,
    Closed,
}
```

**Debounce coalescing:** When multiple events arrive for the same path during the debounce window, the latest `kind` wins (last-write-wins). The deadline resets on each new event.

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
- More changes → deadline resets, changes accumulate (last-write-wins per path)
- Deadline fires → all changes passed to `run_build_sequence()`

The fs-watcher layer (`notify-debouncer-full`) is available as an option for users who need OS-level event deduplication (noisy editors, network drives), exposed through `watch.notify` options. Using both layers adds latency and makes timing harder to reason about, so it's not the default.

### Default Delay

Rollup defaults `buildDelay` to 0ms. Rolldown currently defaults to 100ms. This is a divergence worth aligning on.

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
         b. task.build() → bundler.write() + update_watch_files()
         c. handler.on_event(BUNDLE_END or ERROR)
      6. handler.on_event(END)
      7. drain_buffered_events() → process events that arrived during build
```

### Close

```
watcher.close() sends WatcherMsg::Close(oneshot_tx)
  → handle_close():
      1. State → Closing
      2. task.call_hook_close_watcher() for each task (plugin hook, awaited)
      3. task.close() for each task (bundler cleanup)
      4. handler.on_close() (awaited)
      5. State → Closed
      6. oneshot reply → close() promise resolves
```

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

### Notify Event Mapping

```
notify::EventKind::Create(_)  → WatcherChangeKind::Create
notify::EventKind::Remove(_)  → WatcherChangeKind::Delete
Everything else                → WatcherChangeKind::Update (defensive: spurious rebuild > missed rebuild)
```

### Path Identity

The watch set stores paths as raw `ArcStr` strings. The `notify` crate reports events with OS-native paths. If these don't match exactly, `is_watched_file()` fails silently. The current `#[cfg(windows)]` backslash fallback is a symptom.

**Recommendation:** Use `PathBuf` for the watched file set instead of `ArcStr`. This handles trailing slashes, double separators, `.` segments, and Windows `\` vs `/` — all common mismatch sources between resolver output and notify events.

See [module-id.md](./module-id.md) for the full analysis of path identity across the bundler, `PathBuf` comparison behavior, and Rollup's approach.

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

All methods are awaited — the coordinator blocks on handler calls, ensuring Rollup-compatible sequential semantics.

## NAPI Bridge

The NAPI handler funnels all events through a single `ThreadsafeFunction` callback. `call_async` awaits the JS Promise, ensuring the Rust coordinator blocks until JS handlers finish.

```rust
struct NapiWatcherEventHandler {
    on_event: ThreadsafeFunction<BindingWatcherEvent, Promise<()>>,
}

impl WatcherEventHandler for NapiWatcherEventHandler {
    async fn on_event(&self, event: WatchEvent) {
        self.on_event.call_async(event.into()).await;
    }
    // same pattern for on_change, on_restart, on_close
}
```

The TypeScript `WatcherEmitter.onEvent()` dispatches based on event kind to the appropriate listener set.

## Configuration

```typescript
interface WatcherOptions {
  skipWrite?: boolean; // Skip bundle.write(). Default: false
  buildDelay?: number; // Debounce ms. Default: 0 (Rust default: 100ms)
  notify?: {
    pollInterval?: number; // Polling backend interval ms. Default: 30000
    compareContents?: boolean; // Content comparison for polling. Default: false
  };
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

### Rust Core (done)

- [x] Actor architecture with single-owner coordinator
- [x] Explicit state machine (`WatcherState`)
- [x] Debouncing with deadline reset on new changes
- [x] Per-task fs watchers with isolated watch sets
- [x] Build sequence matching Rollup's 7-step semantics
- [x] Close flow properly calls `bundler.close()` (fixes old bug)
- [x] No `block_on()` — async actor on tokio runtime (fixes [#6393](https://github.com/rolldown/rolldown/issues/6393))
- [x] `WatcherEventHandler` trait with blocking (awaited) semantics
- [x] `BUNDLE_END`/`ERROR` events carry `Arc<Bundler>` handle ([#6618](https://github.com/rolldown/rolldown/issues/6618))
- [x] Error recovery — build errors emit `ERROR` event, watcher continues
- [x] Buffered event draining after builds (`drain_buffered_events`)
- [x] `FileChangeEvent` mapping from `notify` (in `TaskFsEventHandler`)
- [x] Defensive fallback to `Update` for unknown notify event kinds

### NAPI + TypeScript Bridge (todo)

- [ ] `NapiWatcherEventHandler` implementing `WatcherEventHandler` trait — bridges events to JS via `ThreadsafeFunction`
- [ ] `BindingWatcher` wrapping `rolldown_watcher::Watcher` instead of `rolldown::Watcher`
- [ ] Map `WatchEvent` variants to `BindingWatcherEvent` for JS consumption
- [ ] Update `packages/rolldown/src/api/watch/watcher.ts` to work with new binding API
- [ ] Surface setup errors (e.g. `options` hook) as `ERROR` events, not unhandled rejections ([#6482](https://github.com/rolldown/rolldown/issues/6482))

### Cleanup (todo, after NAPI works)

- [ ] Delete old watcher (`crates/rolldown/src/watch/`)
- [ ] Remove `reset_closed_for_watch_mode()` hack
- [ ] Rename `WatcherChangeKind` → `FileChangeEventKind` (type stays in `rolldown_common`)
- [ ] CLI `--watch` mode working with new watcher ([#7759](https://github.com/rolldown/rolldown/issues/7759))

### Missing Features (todo)

- [ ] Bulk change handling — `git checkout` can produce thousands of file changes. Current issues:
  - O(n²) dedup: `on_file_change()` does `Vec::find()` per change. Should use `IndexMap<String, WatcherChangeKind>` (matches Rollup's `invalidatedIds: Map`).
  - Per-change bundler lock: `call_on_invalidate()` acquires the bundler mutex for each change individually.
  - Per-change state transition: each change moves/reconstructs the `WatcherState` enum and reads the clock.
  - `process_file_changes()` should accept the batch, do a single state transition, and batch `on_invalidate`.
- [ ] Resolver cache invalidation between rebuilds ([#6482](https://github.com/rolldown/rolldown/issues/6482))
- [ ] `skipWrite` support — check `options.watch.skip_write`, call `generate()` instead of `write()`
- [ ] File unwatching — `update_watch_files()` only adds, never removes. Watch set grows monotonically
- [ ] Smart change coalescing — Rollup's `eventsRewrites` table (create+delete=removed, delete+create=update)

### Future

- [ ] Non-blocking builds — spawn builds instead of inline `await` (see Unresolved Questions)
- [ ] Incremental builds — `WatchTask::build()` currently does full rebuild via `bundler.write()`
- [ ] Parallel task builds within a single coordinator

## Unresolved Questions

- **Build should not block the coordinator loop** — Currently the coordinator `await`s builds inline, blocking the entire event loop. The dev engine (`BundleCoordinator`) solves this by `tokio::spawn`ing builds — the coordinator loop stays responsive to messages while builds run. On `Close`, the dev engine still waits for the running build to finish gracefully, but the point is it _receives_ the message immediately rather than being blocked. The watcher should follow the same pattern — spawn builds, receive completion messages back, and keep the loop free to process events during builds.

- **Parallel task builds** — `watch([configA, configB])` builds tasks sequentially (matching Rollup), while calling `watch(configA); watch(configB)` separately runs them in parallel (separate coordinators). This means sequential execution isn't a meaningful guarantee — users can trivially opt into parallelism by splitting calls. Should we just parallelize tasks within a single coordinator too?

- **Shared vs per-task FsWatcher** — Currently each `WatchTask` owns its own `DynFsWatcher`. If two tasks watch the same file, it's watched twice at the OS level. A single shared `DynFsWatcher` at the coordinator level would deduplicate watches and use fewer OS resources. Adding files is straightforward. Unwatching (not yet implemented) would require cross-task coordination — a file can only be unwatched when no task needs it (reference counting or a union check across task watch sets). Since unwatching isn't implemented yet, a shared watcher would be strictly simpler today.

- **Watch files not persisted across builds** — `bundler.watch_files()` returns the watch set from the latest build, but this set is not persisted between builds. With full rebuilds this is fine (each build produces a complete set). But with incremental builds, only a subset of modules are re-processed, so the incremental build's `watch_files()` would be incomplete — it wouldn't include files from modules that weren't re-visited. The watch set needs to be accumulated/persisted across builds, not replaced each time.

## Related

- [module-id](./module-id.md) — Module ID, path identity, and normalization
- [#6482](https://github.com/rolldown/rolldown/issues/6482) — Watch mode issue collection (tracks all known bugs)
- `crates/rolldown_watcher/` — Implementation
- `crates/rolldown_fs_watcher/` — File system watching abstraction over `notify`
- `crates/rolldown_dev/` — Dev mode, uses same actor pattern for reference
- `packages/rolldown/src/api/watch/` — TypeScript API layer
