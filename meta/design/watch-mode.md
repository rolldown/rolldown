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
  DynFsWatcher ──(TaskFsEventHandler)──→ WatcherMsg::FsEvent ──→ WatchCoordinator
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
├── lib.rs                // Public exports
├── watcher.rs            // Watcher (public API) + TaskFsEventHandler + WatcherConfig
├── watch_coordinator.rs  // WatchCoordinator (actor + event loop)
├── watch_task.rs         // WatchTask (bundler + fs watcher) + WatchTaskIdx + BuildOutcome
├── handler.rs            // WatcherEventHandler async trait
├── event.rs              // WatchEvent, BundleStartEventData, BundleEndEventData, WatchErrorEventData
├── state.rs              // WatcherState enum + transitions + ChangeEntry
└── msg.rs                // WatcherMsg enum (FsEvent, Close)
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
    Debouncing { changes: Vec<ChangeEntry>, deadline: Instant },
    Closing,
    Closed,
}
```

**Debounce coalescing:** When multiple events arrive for the same path during the debounce window, the latest `kind` wins (last-write-wins). The deadline resets on each new event.

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
  → TaskFsEventHandler sends WatcherMsg::FsEvent
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
notify::EventKind::Create(_)                    → WatcherChangeKind::Create
notify::EventKind::Modify(Data | Any)           → WatcherChangeKind::Update
notify::EventKind::Modify(Name(RenameMode::To)) → WatcherChangeKind::Update
notify::EventKind::Remove(_)                    → WatcherChangeKind::Delete
Other                                           → ignored
```

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

## Known Gaps

The new `rolldown_watcher` is expected to resolve all issues tracked in [#6482](https://github.com/rolldown/rolldown/issues/6482). Each gap below links to the relevant issue(s).

### Must Fix (from #6482)

1. **Close mechanism is broken** — The old watcher never calls `Bundler::close()`, so `bundler.is_closed` is always false. The new watcher's close flow (the Close section above) properly calls `task.close()` for each bundler. ([#6482](https://github.com/rolldown/rolldown/issues/6482))

2. **Watch hangs with small thread pools** — The old implementation uses `block_on()` which can deadlock with limited blocking threads. The new async actor model avoids this entirely — the coordinator runs on the tokio runtime without blocking threads. ([#6393](https://github.com/rolldown/rolldown/issues/6393), [#6482](https://github.com/rolldown/rolldown/issues/6482))

3. **Unhandled errors in `options` hook cause promise rejections** — The old `watch()` is sync but `createWatcher()` is async, so errors in the options hook become unhandled rejections. The new design must ensure errors during setup are surfaced as `event('ERROR')` rather than unhandled rejections. ([#6482](https://github.com/rolldown/rolldown/issues/6482))

4. **`BUNDLE_END`/`ERROR` events need `RolldownBuild` handle** — Rollup's watch events include a `result` (build handle) that consumers can use. Rolldown's bundler model differs (each `write/generate` is a new build), so the new watcher exposes `Arc<TokioMutex<Bundler>>` in event data as the access mechanism. ([#6618](https://github.com/rolldown/rolldown/issues/6618), [#6482](https://github.com/rolldown/rolldown/issues/6482))

5. **Resolver cache not cleared between rebuilds** — The resolver cache must be invalidated for changed files on rebuild. The new watcher should ensure proper cache invalidation as part of the rebuild sequence. ([#6482](https://github.com/rolldown/rolldown/issues/6482))

6. **`--watch` CLI mode not working** — Reported for tsdown/rolldown CLI. The new watcher must work correctly when invoked via CLI. ([#7759](https://github.com/rolldown/rolldown/issues/7759))

7. **NAPI handler not yet wired** — `WatcherEventHandler` trait needs to be implemented in the NAPI binding layer to complete the migration. ([#6482](https://github.com/rolldown/rolldown/issues/6482))

### Should Fix

8. **No file unwatching** — `update_watch_files()` only adds, never removes files no longer in the module graph. Watch set grows monotonically.

9. **No `skipWrite` support** — Always calls `bundler.write()`. Should check `options.watch.skip_write` and call `bundler.generate()` instead.

10. **Simplified change coalescing** — Uses last-write-wins. Rollup's `eventsRewrites` table (e.g. create+delete=removed, delete+create=update) is not implemented.

### Future

11. **No incremental build** — `WatchTask::build()` calls `bundler.write()` (full rebuild). Incremental builds are a future optimization.

## Related

- [#6482](https://github.com/rolldown/rolldown/issues/6482) — Watch mode issue collection (tracks all known bugs)
- `crates/rolldown_watcher/` — Implementation
- `crates/rolldown_fs_watcher/` — File system watching abstraction over `notify`
- `crates/rolldown_dev/` — Dev mode, uses same actor pattern for reference
- `packages/rolldown/src/api/watch/` — TypeScript API layer
