# The Dev Engine in `rolldown_dev` (Full Bundle Mode)

## Summary

The dev engine (`rolldown_dev` crate) is rolldown's dev-mode build
orchestration layer in Full Bundle Mode. It sits between the file watcher
/ dev server and the core `Bundler`, deciding _what_ build to run — an HMR
patch, an incremental rebuild, or a full build — and _when_. It is
structured as a `DevEngine` (the public async API surface) driving a
single message-loop `BundleCoordinator` (a state machine plus a work
queue) that spawns one `BundlingTask` at a time. This document is a map of
that machinery: the component layering, the `CoordinatorMsg` protocol, the
`CoordinatorState` machine, the `TaskInput` work types, and the data-flow
pipelines for file edits, HMR generation, and lazy full-bundle refresh on
browser page loads. It describes **facts** about the current
implementation, not the narrative of any particular change.

---

## 1. Components and layering

The dev engine is built from four cooperating pieces, all in
`crates/rolldown_dev/src/`:

```
                    ┌─────────────────────────────────────────┐
                    │              DevEngine                   │
                    │  (dev_engine.rs)                         │
                    │  - public async API surface              │
                    │  - owns Arc<Mutex<Bundler>>              │
                    │  - owns the coordinator mpsc Sender      │
                    │  - spawns the coordinator task           │
                    └───────────────┬─────────────────────────┘
                                    │  CoordinatorMsg (mpsc)
                                    ▼
                    ┌─────────────────────────────────────────┐
                    │           BundleCoordinator              │
                    │  (bundle_coordinator.rs)                 │
                    │  - single-threaded message loop          │
                    │  - owns CoordinatorState                 │
                    │  - owns queued_tasks: VecDeque<TaskInput>│
                    │  - owns the fs watcher                   │
                    │  - decides WHAT build to run and WHEN    │
                    └───────────────┬─────────────────────────┘
                                    │  spawns
                                    ▼
                    ┌─────────────────────────────────────────┐
                    │             BundlingTask                 │
                    │  (bundling_task.rs)                      │
                    │  - one unit of build work                │
                    │  - locks the Bundler, runs HMR/rebuild   │
                    │  - reports back via CoordinatorMsg       │
                    └───────────────┬─────────────────────────┘
                                    │  calls into
                                    ▼
                    ┌─────────────────────────────────────────┐
                    │               Bundler                    │
                    │  (crates/rolldown/src/bundler/)          │
                    │  - compute_hmr_update_for_file_changes   │
                    │  - incremental_generate / incremental_   │
                    │    write                                 │
                    └─────────────────────────────────────────┘
```

`DevContext` (`dev_context.rs`) is the shared, immutable-ish glue passed
around as `SharedDevContext = Arc<DevContext>`:

```rust
pub struct DevContext {
  pub options: NormalizedDevOptions,
  pub coordinator_tx: CoordinatorSender,   // clone to send messages in
  pub clients: SharedClients,              // connected HMR clients
}
```

### Threading model

- The `BundleCoordinator` runs in **one** dedicated tokio task
  (`DevEngine::run` does `tokio::spawn(coordinator.run())`,
  `dev_engine.rs:115`). Its `run()` is a single `while let Some(msg) =
self.rx.recv().await` loop, so all coordinator state mutation is
  serialized — there is no lock on `CoordinatorState`, the message loop
  _is_ the lock.
- Each `BundlingTask` runs in its **own** spawned task. The coordinator
  keeps a `Shared` future handle to the currently running one
  (`current_bundling_future`).
- The `Bundler` is shared as `Arc<Mutex<Bundler>>`. A `BundlingTask`
  locks it for the duration of its HMR/rebuild work.
- Communication is via an **unbounded** mpsc channel
  (`unbounded_channel::<CoordinatorMsg>()`, `dev_engine.rs:62`).
  Request/response messages carry a `tokio::sync::oneshot` reply
  channel.

---

## 2. The message protocol — `CoordinatorMsg`

Defined in `types/coordinator_msg.rs`. Every interaction with the
coordinator is one of these messages:

```rust
pub enum CoordinatorMsg {
  WatchEvent(FsEventResult),                 // raw fs-watcher event batch
  BundleCompleted {                          // a BundlingTask finished
    has_encountered_error: bool,
    has_generated_bundle_output: bool,
  },
  ScheduleBuildIfStale { reply: … },         // ask coordinator to drain its queue
  GetState { reply: … },                     // snapshot of coordinator state
  EnsureLatestBundleOutput { reply: … },     // "I need a fresh full bundle"
  TriggerFullBuild,                           // unconditional full build (fire-and-forget)
  GetWatchedFiles { reply: … },              // list of watched paths
  ModuleChanged { module_id: String },       // programmatic module change
  Close,                                     // shut the coordinator down
}
```

Routing happens in `BundleCoordinator::run` (`bundle_coordinator.rs:98-150`):

| Message                    | Handler                                      |
| -------------------------- | -------------------------------------------- |
| `WatchEvent`               | `handle_watch_event` → `handle_file_changes` |
| `BundleCompleted`          | `handle_bundle_completed`                    |
| `ScheduleBuildIfStale`     | `schedule_build_if_stale`, reply with result |
| `GetState`                 | `create_state_snapshot`, reply               |
| `EnsureLatestBundleOutput` | `ensure_latest_bundle_output`, reply         |
| `TriggerFullBuild`         | `trigger_full_build` (no reply)              |
| `GetWatchedFiles`          | reply with the `watched_files` set           |
| `ModuleChanged`            | queue a `Rebuild`, schedule                  |
| `Close`                    | await running task, then `break` the loop    |

The producers:

- The **fs watcher** sends `WatchEvent`. The watcher event handler is
  created by `BundleCoordinator::create_watcher_event_handler` and wired
  to the same `coordinator_tx`.
- A finishing **`BundlingTask`** sends `BundleCompleted` from its
  `run()` (`bundling_task.rs:75-80`).
- The **`DevEngine`** sends `ScheduleBuildIfStale`, `GetState`,
  `EnsureLatestBundleOutput`, `GetWatchedFiles`, `ModuleChanged`,
  `Close` on behalf of its public API callers (the dev server, HTTP
  middleware, lazy-compilation endpoint, etc.).

---

## 3. `CoordinatorState` — the scheduler state machine

Defined in `types/coordinator_state.rs`:

```rust
pub enum CoordinatorState {
  Initialized,
  Idle,
  FullBuildInProgress,
  FullBuildFailed,
  InProgress,
  Failed,
}
```

It is a single `Copy` enum field on `BundleCoordinator`, mutated only via
`set_initial_build_state` (`bundle_coordinator.rs:445`). It splits into
two halves joined by `Idle`:

- **Initial-full-build half** — `Initialized`, `FullBuildInProgress`,
  `FullBuildFailed`. Concerns the very first build.
- **Steady-state half** — `Idle`, `InProgress`, `Failed`. Concerns every
  build after the initial one succeeded.

### State meanings

| State                 | Meaning                                                            |
| --------------------- | ------------------------------------------------------------------ |
| `Initialized`         | Constructed but `run()` not yet entered. Transient.                |
| `FullBuildInProgress` | The initial `TaskInput::FullBuild` is running.                     |
| `FullBuildFailed`     | The initial full build errored. No usable output exists at all.    |
| `Idle`                | No build running; last build (if any) succeeded.                   |
| `InProgress`          | An incremental task (`Hmr` / `HmrRebuild` / `Rebuild`) is running. |
| `Failed`              | The last incremental task errored.                                 |

### Transition map

```
            ┌──────────────┐
            │ Initialized  │  (constructor: BundleCoordinator::new)
            └──────┬───────┘
                   │ run() entry: push TaskInput::FullBuild,
                   │ set state=Idle, schedule_build_if_stale()
                   ▼
        ┌────────────────────┐
        │        Idle        │ ◄──────────────────────────────┐
        └─────────┬──────────┘                                │
                  │ schedule_build_if_stale pops a task:      │
                  │   FullBuild → FullBuildInProgress         │
                  │   else      → InProgress                  │
        ┌─────────┴──────────┐                                │
        ▼                    ▼                                │
┌───────────────────┐  ┌───────────────────┐                  │
│FullBuildInProgress│  │    InProgress     │                  │
└─────────┬─────────┘  └─────────┬─────────┘                  │
          │ BundleCompleted      │ BundleCompleted            │
          │  err → FullBuildFailed│  err → Failed             │
          │  ok  → Idle ─────────┼──ok──→ Idle ───────────────┤
          ▼                      ▼  (then schedule_build_if_  │
┌───────────────────┐  ┌───────────────────┐    stale always)│
│  FullBuildFailed  │  │      Failed       │                  │
└─────────┬─────────┘  └─────────┬─────────┘                  │
          │ next file change:    │ next file change:          │
          │  queue FullBuild,    │  queue Hmr/HmrRebuild,     │
          │  schedule →          │  schedule →                │
          │  FullBuildInProgress │  InProgress ───────────────┘
          ▼                      ▼
       (loop)                 (loop)
```

### Where each transition lives

| Transition                                                         | Site                                           |
| ------------------------------------------------------------------ | ---------------------------------------------- |
| `Initialized → Idle`                                               | `run()` startup, `bundle_coordinator.rs:84-87` |
| `Idle/Failed/FullBuildFailed → FullBuildInProgress` / `InProgress` | `schedule_build_if_stale`, `:352-356`          |
| `FullBuildInProgress → FullBuildFailed`                            | `handle_bundle_completed`, `:263`              |
| `FullBuildInProgress → Idle`                                       | `handle_bundle_completed`, `:268`              |
| `InProgress → Failed`                                              | `handle_bundle_completed`, `:288`              |
| `InProgress → Idle`                                                | `handle_bundle_completed`, `:293`              |

`set_initial_build_state` is the single mutation point — a convenient
place to observe all transitions.

---

## 4. The coordinator run loop and startup

`BundleCoordinator::run` (`bundle_coordinator.rs:80-151`):

1. Asserts it starts in `Initialized`; otherwise logs an error and
   returns.
2. Pushes a `TaskInput::FullBuild` into `queued_tasks`, sets state to
   `Idle`, calls `schedule_build_if_stale()` — this kicks off the
   initial build (`Idle → FullBuildInProgress`).
3. Enters the `while let Some(msg) = self.rx.recv().await` loop,
   dispatching each `CoordinatorMsg` as in §2.
4. On `Close`, awaits any running `BundlingTask` (so it doesn't panic
   trying to send `BundleCompleted` into a dropped channel) and breaks.

`BundleCoordinator::new` initializes: `state = Initialized`,
`queued_tasks = []`, `has_stale_bundle_output = true`,
`current_bundling_future = None`.

---

## 5. `TaskInput` — the unit of queued work

Defined in `types/task_input.rs`. The coordinator's work queue is
`queued_tasks: VecDeque<TaskInput>`.

```rust
pub enum TaskInput {
  FullBuild,                              // full build (initial or recovery)
  Rebuild     { changed_files: … },       // incremental rebuild, no HMR patch
  Hmr         { changed_files: … },       // HMR patch only, no rebuild
  HmrRebuild  { changed_files: … },       // HMR patch AND incremental rebuild
}
```

### Predicates

```rust
requires_full_rebuild()      // true only for FullBuild
requires_rebuild()           // true for FullBuild | Rebuild | HmrRebuild
require_generate_hmr_update()// true for Hmr | HmrRebuild
```

These predicates drive the bundling task's behavior (see §8) and the
coordinator's state choice (see §7).

### Mergeability — `is_mergeable_with` / `merge_with`

When the coordinator pops a task it greedily merges adjacent compatible
tasks from the front of the queue. The rules:

| First task   | Merges with           | Result                                         |
| ------------ | --------------------- | ---------------------------------------------- |
| `FullBuild`  | anything              | stays `FullBuild` (absorbs everything)         |
| `Rebuild`    | only `Rebuild`        | `Rebuild` with unioned `changed_files`         |
| `Hmr`        | `Hmr` or `HmrRebuild` | unioned files; `Hmr+HmrRebuild` → `HmrRebuild` |
| `HmrRebuild` | `Hmr` or `HmrRebuild` | `HmrRebuild` with unioned files                |

`Rebuild` and `Hmr`/`HmrRebuild` are **not** mergeable with each other —
an incremental rebuild task pulls in files not intended for HMR
generation, so they must stay separate. `FullBuild` absorbing everything
means a burst of any task types collapses into a single `FullBuild` if
one is present.

---

## 6. From fs event to queued task — `handle_watch_event`

`handle_watch_event` (`bundle_coordinator.rs:154-194`) translates a raw
`notify` event batch into a `FxIndexMap<PathBuf, WatcherChangeKind>`:

| `notify` `EventKind`                          | `WatcherChangeKind` |
| --------------------------------------------- | ------------------- |
| `Create(_)`                                   | `Create`            |
| `Modify(Name(RenameMode::From))`, `Remove(_)` | `Delete`            |
| `Modify(_)` (other)                           | `Update`            |
| `Modify(Metadata(_))` on macOS non-polling    | ignored             |

It then calls `handle_file_changes`. Note that `rolldown_dev` does no
debouncing or Delete+Create consolidation of its own — it dispatches each
raw watcher event batch straight through.

---

## 7. `handle_file_changes` — per-state queueing

`handle_file_changes` (`bundle_coordinator.rs:197-237`) decides what
`TaskInput` a file change becomes, keyed on the current state:

```
state                                 → action
─────────────────────────────────────────────────────────────────
FullBuildInProgress                   → stash files into
                                        queued_file_changes_waited_
                                        for_full_build (no task queued)
Idle | InProgress | Failed            → queue Hmr (or HmrRebuild if
                                        rebuild_strategy == Always),
                                        then schedule_build_if_stale()
FullBuildFailed                       → clear queued_file_changes,
                                        queue TaskInput::FullBuild,
                                        then schedule_build_if_stale()
Initialized                           → error log, ignored
```

Notes:

- During `FullBuildInProgress`, file changes are not turned into tasks;
  they are stashed and replayed when the full build succeeds
  (`handle_bundle_completed` `:269-273` calls `handle_file_changes` with
  the drained set).
- `Idle`, `InProgress`, and `Failed` are treated **identically** here —
  all three queue an `Hmr`/`HmrRebuild`.
- The `Always` vs non-`Always` choice for `Hmr` vs `HmrRebuild` is the
  `rebuild_strategy` option (see §9).
- `FullBuildFailed` is the one state whose file-change handling queues a
  `FullBuild`.

---

## 8. `schedule_build_if_stale` — popping, merging, spawning

`schedule_build_if_stale` (`bundle_coordinator.rs:303-372`) is the bridge
from `queued_tasks` to a running `BundlingTask`. Behavior by state:

| State                                 | Behavior                                                                                       |
| ------------------------------------- | ---------------------------------------------------------------------------------------------- |
| `Initialized`                         | error log, returns `None`                                                                      |
| `FullBuildInProgress` / `InProgress`  | a build is already running — returns the existing `current_bundling_future`, schedules nothing |
| `Idle` / `FullBuildFailed` / `Failed` | pops the front task, greedily merges adjacent mergeable tasks, spawns a `BundlingTask`         |

When it spawns a task:

1. Pop the front `TaskInput`.
2. While the next queued task `is_mergeable_with` it, `merge_with` it.
3. Construct a `BundlingTask`.
4. If `task_input.requires_full_rebuild()` → state `FullBuildInProgress`;
   else → state `InProgress`.
5. `tokio::spawn` the task's `run()` as a `Shared` future; store it in
   `current_bundling_future`.

The key invariant: at most one `BundlingTask` runs at a time. While one
runs, the coordinator is in `*InProgress` and new file changes only
append to `queued_tasks`; they are drained when the current task
finishes (see §11).

---

## 9. `RebuildStrategy` and the `Hmr → HmrRebuild` upgrade

`RebuildStrategy` (`crates/rolldown_dev_common/src/types/rebuild_strategy.rs`):

```rust
pub enum RebuildStrategy {
  Always,   // incremental rebuild ALWAYS issued after HMR
  Auto,     // (default) rebuild only if HMR updates contain a full-reload
  Never,    // never rebuild after HMR
}
```

It influences the dev engine in **two** places:

### 9a. At queue time (`handle_file_changes`)

```rust
let task_input = if rebuild_strategy.is_always() {
  TaskInput::HmrRebuild { changed_files }   // Always
} else {
  TaskInput::Hmr { changed_files }          // Auto or Never
};
```

`Always` commits to a rebuild up front. `Auto` and `Never` queue an
HMR-only task.

### 9b. At run time (`bundling_task.rs:104-114`) — the auto-upgrade

After HMR generation, the bundling task may **rewrite its own input**:

```rust
if rebuild_strategy.is_auto()
  && has_full_reload_update         // a generated HMR update is a full reload
  && !self.input.requires_rebuild() // input was a pure Hmr
{
  self.input = TaskInput::HmrRebuild { changed_files: … };
}
```

The rationale: whether a change is hot-swappable or requires a full page
reload is not knowable until the HMR diff is computed. So with `Auto`,
the coordinator queues a cheap `Hmr`, the task computes the HMR diff, and
_then_ — if the diff turned out to be a full-reload — the task upgrades
itself to `HmrRebuild` and performs the rebuild. `Always` skips this
deferral by always rebuilding; `Never` never rebuilds at run time.

Consequence: the `TaskInput` variant the coordinator queued is not
necessarily the variant that runs. A coordinator-queued `Hmr` can become
a `HmrRebuild` mid-task.

---

## 10. `BundlingTask` — executing one unit of work

`BundlingTask::run` (`bundling_task.rs:58-81`) calls `run_inner`, then
sends `BundleCompleted` back to the coordinator with two booleans:

- `has_encountered_error` — `has_encountered_error` flag OR `run_inner`
  returned `Err`.
- `has_generated_bundle_output` — equals `has_rebuild_happen`, i.e.
  whether the task actually performed a rebuild.

`run_inner` (`bundling_task.rs:83-122`) does, in order:

1. **`watchChange` plugin hook** — for each changed file, calls
   `plugin_driver.watch_change` on the last bundle handle.
2. **HMR generation** — if `require_generate_hmr_update()`, calls
   `generate_hmr_updates`, which sets `has_full_reload_update`.
3. **Auto-upgrade** — the §9b `Hmr → HmrRebuild` rewrite.
4. **Rebuild** — if `requires_rebuild()`, sets `has_rebuild_happen =
true` and calls `rebuild()`.

### `generate_hmr_updates` (`bundling_task.rs:124-186`)

- Locks the `Bundler`.
- Collects `ClientHmrInput` for every connected client from
  `dev_context.clients`.
- Calls `bundler.compute_hmr_update_for_file_changes(...)`.
- Scans the resulting updates; if any `is_full_reload()`, sets
  `has_full_reload_update = true`.
- On error, sets `self.has_encountered_error = true`.
- Invokes the `on_hmr_updates` callback if configured.

### `rebuild` (`bundling_task.rs:189-223`)

- Locks the `Bundler`.
- Picks the scan mode:
  ```rust
  let scan_mode = if self.input.requires_full_rebuild() {
    ScanMode::Full
  } else {
    ScanMode::Partial(<changed file paths>)
  };
  ```
- Calls `bundler.incremental_write(scan_mode)` if `skip_write` is
  false, else `bundler.incremental_generate(scan_mode)`.
- On error, sets `self.has_encountered_error = true`.
- Invokes the `on_output` callback if configured.

Only `TaskInput::FullBuild` produces `ScanMode::Full`. Every other
rebuilding task (`Rebuild`, `HmrRebuild`) produces `ScanMode::Partial`.

---

## 11. `handle_bundle_completed` — closing out a task

`handle_bundle_completed` (`bundle_coordinator.rs:240-299`) processes the
`BundleCompleted` message. The two relevant arms:

### `FullBuildInProgress`

```rust
current_bundling_future = None;
update_watch_paths();                       // even on failure
if has_encountered_error {
  state = FullBuildFailed;
  has_stale_bundle_output = true;
} else {
  has_stale_bundle_output = false;
  state = Idle;
  // replay file changes stashed during the full build
  if !queued_file_changes_waited_for_full_build.is_empty() {
    handle_file_changes(drained_changes);
  }
}
// No schedule_build_if_stale here — on failure we wait for an
// external trigger; on success queued changes were already handled.
```

### `InProgress`

```rust
current_bundling_future = None;
update_watch_paths();                       // register newly-pulled-in files
if has_encountered_error {
  state = Failed;
  has_stale_bundle_output = true;
} else {
  has_stale_bundle_output = !has_generated_bundle_output;
  state = Idle;
}
schedule_build_if_stale();                  // ALWAYS — drain the queue
```

Key facts:

- `has_stale_bundle_output` becomes `true` after any errored build, and
  after a _successful_ build that did **not** rebuild (i.e. an `Hmr`-only
  task: `has_generated_bundle_output == false`).
- `has_stale_bundle_output` becomes `false` after a successful build that
  rebuilt (`Rebuild`, `HmrRebuild`, `FullBuild`).
- The `InProgress` arm always calls `schedule_build_if_stale` afterward,
  on success or failure, so the work queue keeps draining.

---

## 12. `has_stale_bundle_output` — the freshness flag

A single `bool` on `BundleCoordinator`. Semantics: "the full bundle
output on disk / in memory may not reflect the latest source."

| Event                                                 | `has_stale_bundle_output` becomes |
| ----------------------------------------------------- | --------------------------------- |
| Construction                                          | `true`                            |
| Successful `FullBuild`                                | `false`                           |
| Failed `FullBuild`                                    | `true`                            |
| Successful task that rebuilt (`Rebuild`/`HmrRebuild`) | `false`                           |
| Successful `Hmr`-only task (no rebuild)               | `true`                            |
| Failed incremental task                               | `true`                            |
| `ModuleChanged` received                              | `true`                            |

It is consumed by `ensure_latest_bundle_output` (§13) to decide whether
a lazy rebuild is needed before serving the full bundle. It is also
surfaced in `CoordinatorStateSnapshot.has_stale_output` and thence
`BundleState.has_stale_output`.

---

## 13. `ensure_latest_bundle_output` — the lazy full-bundle pipeline

This is the path that guarantees a browser page load / refresh receives
an up-to-date full bundle. It spans both `DevEngine` and
`BundleCoordinator`.

### 13a. `DevEngine::ensure_latest_bundle_output` (`dev_engine.rs:184-227`)

A bounded retry loop:

```rust
loop {
  loop_count += 1;
  if loop_count > 100 { panic!/warn!; break; }   // safety valve

  // send EnsureLatestBundleOutput with a oneshot reply channel
  let received: Option<EnsureLatestBundleOutputReturn> = …;

  if let Some(ret) = received {
    ret.future.await;                            // wait for that build
    if ret.is_ensure_latest_bundle_output_future {
      break;                                     // definitive build done
    }
    // else loop again, re-ask
  } else {
    break;                                       // None → already fresh
  }
}
```

### 13b. `BundleCoordinator::ensure_latest_bundle_output` (`bundle_coordinator.rs:381-438`)

Returns `Option<EnsureLatestBundleOutputReturn>` per state:

| State                                | Action                                   | `future`      | `is_ensure_latest_bundle_output_future` |
| ------------------------------------ | ---------------------------------------- | ------------- | --------------------------------------- |
| `Initialized`                        | warn, return `None`                      | —             | —                                       |
| `Idle`, queue empty, **stale**       | queue an empty-files `Rebuild`, schedule | the new build | `true`                                  |
| `Idle`, queue empty, **fresh**       | return `None`                            | —             | —                                       |
| `Idle`, queue non-empty              | schedule the queued task                 | that build    | `false`                                 |
| `FullBuildInProgress` / `InProgress` | return the running future                | running build | `false`                                 |
| `Failed` / `FullBuildFailed`         | return `None`                            | —             | —                                       |

### 13c. The `is_ensure_latest_bundle_output_future` flag

The flag tells the `DevEngine` loop whether the awaited future is _the_
build that definitively produces fresh output:

- `true` — a build was scheduled specifically to refresh output (a
  `Rebuild` for stale `Idle`). When it resolves, output is fresh — the
  loop breaks.
- `false` — the awaited future is some other build (a pre-existing
  queued task, or an already-running build). When it resolves the output
  may still be stale, so the loop re-sends `EnsureLatestBundleOutput`
  and re-evaluates.
- `None` reply — output is already fresh; the loop breaks immediately.

The `loop_count > 100` guard is a safety valve against a pathological
never-settling cycle.

### 13d. Full pipeline example — page load after an `Hmr`-only task

1. An `Hmr`-only task completes successfully. `has_rebuild_happen ==
false` → `has_generated_bundle_output == false` →
   `has_stale_bundle_output == true`, state `Idle`.
2. A browser loads a page. The dev server middleware (JS/binding glue,
   outside these crates) calls `DevEngine::ensure_latest_bundle_output`.
3. `DevEngine` sends `EnsureLatestBundleOutput` to the coordinator.
4. The coordinator is `Idle` with an empty queue and stale output:
   queues `TaskInput::Rebuild { changed_files: {} }`, schedules it
   (`Idle → InProgress`), returns the future with the flag `true`.
5. The `Rebuild` task runs: no HMR generation (`Rebuild` does not
   `require_generate_hmr_update`), `requires_rebuild()` is true →
   `ScanMode::Partial` → `BundleMode::IncrementalBuild`. It regenerates
   the full bundle output.
6. `BundleCompleted { error: false, has_generated_bundle_output: true }`
   → `has_stale_bundle_output = false`, state `Idle`.
7. `DevEngine`'s awaited future resolves; flag was `true` → loop breaks.
   The middleware serves the now-fresh bundle.

### 13e. Scenarios

The semantics of `ensure_latest_bundle_output` is: **make sure the
caller gets the latest output**. If the output is stale, it schedules
a build to produce fresh output. If a build is already running, it
waits. If the build has failed and no files have changed, the failure
is the latest state — there is nothing it can do.

**Browser refresh — output is stale after Hmr-only task.** The most
common case. The coordinator is `Idle`, `has_stale_bundle_output` is
true, no tasks queued. `ensure_latest_bundle_output` schedules a
`Rebuild` and waits — see §13d for the full walkthrough.

**Browser refresh — build is running.** A file change triggered a
rebuild that hasn't finished yet. The coordinator returns the running
future. The loop waits, then re-checks in case more work queued up
during the build.

**Browser refresh — build previously failed.** The coordinator is in
`FullBuildFailed` or `Failed`. The failure _is_ the latest output —
there is nothing fresher to serve without new file changes.
`ensure_latest_bundle_output` returns `None`.

**Recovery from a failed build.** The user fixes their code. The
watcher detects the change and triggers `handle_file_changes` (§7),
which queues a new build. By the time the user refreshes the browser,
the coordinator is either `InProgress` (build still running —
`ensure_latest_bundle_output` waits for it) or `Idle` (build finished —
output is fresh). This works because `update_watch_paths()` runs even
after a failed build (`handle_bundle_completed`, §11), so files that
were already parsed are watched.

**Edge case: recovery from a missing-import failure.** If the initial
build failed because of a missing import, the missing file was never
parsed and is not in `watch_paths`. The watcher cannot detect its
creation, so editing or creating it does not trigger a rebuild. In this
case, `triggerFullBuild` (below) is needed to force a rebuild.

**`DevEngine::run()` — waiting for the initial build.** `run()` calls
`ensure_latest_bundle_output` to wait for the first `FullBuild`. The
coordinator is in `FullBuildInProgress` and returns the running future.
When the build finishes — success or failure — the output is as
current as it can be. The loop breaks, `run()` returns.

**Manual retry via `triggerFullBuild`.** A separate, fire-and-forget
operation for callers that explicitly want to force a new build
regardless of state (e.g., a dev server reload command).
`DevEngine::trigger_full_build` sends `TriggerFullBuild` to the
coordinator, which unconditionally clears `queued_tasks`, pushes a
`FullBuild`, and schedules it. The call returns immediately without
waiting for the build. Callers that need to wait compose it with
`ensure_latest_bundle_output` — FIFO channel ordering guarantees the
`FullBuild` is scheduled before the ensure message is processed.

---

## 14. The bundler-side incremental entry points

The `BundlingTask::rebuild` calls land in
`crates/rolldown/src/bundler/impl_bundler_incremental_build.rs`:

```rust
incremental_write(scan_mode)     // → incremental_bundle(true,  scan_mode)
incremental_generate(scan_mode)  // → incremental_bundle(false, scan_mode)
```

`incremental_bundle` maps `ScanMode` to `BundleMode`:

```rust
let bundle_mode = match scan_mode {
  ScanMode::Full       => BundleMode::IncrementalFullBuild,
  ScanMode::Partial(_) => BundleMode::IncrementalBuild,
};
```

then runs the work inside `with_cached_bundle`.

### `with_cached_bundle` — cache ownership transfer

`with_cached_bundle` moves the long-lived cache into a per-build
`Bundle`, runs the build closure, and moves the cache back:

```rust
async fn with_cached_bundle<T>(
  &mut self,
  bundle_mode: BundleMode,
  with_fn: impl AsyncFnOnce(&mut Bundle) -> BuildResult<T>,
) -> BuildResult<T> {
  let cache = mem::take(&mut self.cache);       // take from Bundler
  let mut bundle =
    self.bundle_factory.create_bundle(bundle_mode, Some(cache))?;
  let ret = with_fn(&mut bundle).await?;
  self.cache = bundle.cache;                    // move back into Bundler
  Ok(ret)
}
```

There are **two** distinct `ScanStageCache` instances at play:

1. `Bundler::cache` — the long-lived cache held across rebuilds
   (`bundler.rs`).
2. `Bundle::cache` — the per-build cache, alive for one bundle
   invocation.

`with_cached_bundle` is the only place that transfers between them.

### `ScanStageCache` and the snapshot

`ScanStageCache` (`crates/rolldown/src/types/scan_stage_cache.rs`) holds
`snapshot: Option<NormalizedScanStageOutput>` plus module index maps and
barrel state. Relevant methods:

- `set_snapshot(output)` — store the snapshot.
- `take_snapshot() -> Option<…>` — remove and return it.
- `get_snapshot() -> &NormalizedScanStageOutput` — borrow it
  (documented to panic if unset).
- `get_snapshot_mut()` — mutable borrow (panics if unset).
- `merge(...)` — fold an incremental scan output into the snapshot;
  first-time populates it.
- `update_defer_sync_data(...)` — take the snapshot, run the
  `defer_sync_scan_data` work, set it back.

### HMR reads `Bundler::cache`

`impl_bundler_hmr.rs` builds an `HmrStageInput` from `&mut self.cache`
(`Bundler::cache`) at three call sites:

- `compute_hmr_update_for_file_changes` — file-change-driven HMR.
- `compute_update_for_calling_invalidate` — programmatic `invalidate()`.
- `compile_lazy_entry` — lazy-compilation entry compilation.

`HmrStage` then reads the cache's snapshot (e.g. `module_table()` in
`hmr/hmr_stage.rs` calls `get_snapshot()`).

---

## 15. Other `DevEngine` API surface

Beyond `ensure_latest_bundle_output`, the public methods on `DevEngine`
(`dev_engine.rs`):

| Method                                           | Purpose                                                                                        |
| ------------------------------------------------ | ---------------------------------------------------------------------------------------------- |
| `new(config, options)`                           | builds the `Bundler`, normalizes options, creates the watcher and coordinator                  |
| `run()`                                          | spawns the coordinator task, awaits the initial build via `ensure_latest_bundle_output`        |
| `trigger_full_build()`                           | sends `TriggerFullBuild` (fire-and-forget, compose with `ensure_latest_bundle_output` to wait) |
| `wait_for_close()`                               | awaits the coordinator's join handle                                                           |
| `wait_for_ongoing_bundle()`                      | `GetState`, awaits any running future                                                          |
| `get_bundle_state()`                             | `GetState` → `BundleState { last_full_build_failed, has_stale_output }`                        |
| `invalidate(caller, first_invalidated_by)`       | locks the bundler, calls `compute_update_for_calling_invalidate` per client                    |
| `compile_lazy_entry(proxy_module_id, client_id)` | compiles a lazy entry; on success sends `ModuleChanged`                                        |
| `close()`                                        | sends `Close`, runs `closeBundle`, awaits coordinator shutdown                                 |
| `is_closed()` / `bundler_options()`              | accessors                                                                                      |

`ModuleChanged` handling (`bundle_coordinator.rs:123-140`): updates watch
paths, queues a `TaskInput::Rebuild` for the changed module, sets
`has_stale_bundle_output = true`, schedules.

The `#[cfg(feature = "testing")]` methods —
`ensure_task_with_changed_files`, `get_watched_files`,
`create_client_for_testing` — exist for the test harness to drive
synthetic file changes and inspect coordinator state.

---

## 16. Quick reference — concept-to-file map

| Concept                                        | File                                                            |
| ---------------------------------------------- | --------------------------------------------------------------- |
| Public dev API, coordinator spawn              | `crates/rolldown_dev/src/dev_engine.rs`                         |
| State machine, queueing, scheduling            | `crates/rolldown_dev/src/bundle_coordinator.rs`                 |
| One unit of build work                         | `crates/rolldown_dev/src/bundling_task.rs`                      |
| Shared context                                 | `crates/rolldown_dev/src/dev_context.rs`                        |
| `CoordinatorState` enum                        | `crates/rolldown_dev/src/types/coordinator_state.rs`            |
| `TaskInput` enum, merge rules                  | `crates/rolldown_dev/src/types/task_input.rs`                   |
| `CoordinatorMsg` enum                          | `crates/rolldown_dev/src/types/coordinator_msg.rs`              |
| `RebuildStrategy` enum                         | `crates/rolldown_dev_common/src/types/rebuild_strategy.rs`      |
| Incremental entry points, `with_cached_bundle` | `crates/rolldown/src/bundler/impl_bundler_incremental_build.rs` |
| HMR entry points                               | `crates/rolldown/src/bundler/impl_bundler_hmr.rs`               |
| `ScanStageCache`                               | `crates/rolldown/src/types/scan_stage_cache.rs`                 |

---

## Unresolved Questions

- **Auto-recovery from missing-import failures.** When a build fails
  because of an unresolved import, the missing file was never parsed and
  is not in `watch_paths`. Creating it does not trigger a rebuild — the
  user must either touch a watched file or use `triggerFullBuild`. A
  fix: during resolution, when a file is not found, record its path and
  add its parent directory to the watcher. A directory-level create event
  matching a previously-missing path would then trigger a rebuild
  automatically. The existing watcher tests acknowledge this gap
  (`watch.test.ts`: "the missing file's directory is not auto-watched,
  so we need to touch a watched file").

---

## Related

- [bundler-data-lifecycle](./bundler-data-lifecycle.md) — `BundleMode`,
  `Bundle` / `BundleFactory`, and the `ScanStageCache` lifecycle the dev
  engine's incremental builds run through
- [rust-bundler](./rust-bundler.md) — the core `Bundler` struct and build
  lifecycle the dev engine drives
- [watch-mode](./watch-mode.md) — `rolldown_watcher`, the actor-based
  watch architecture; `rolldown_dev` reuses the same actor pattern
- [lazy-compilation](./lazy-compilation.md) — lazy entry compilation,
  reached via `DevEngine::compile_lazy_entry` and the `ModuleChanged`
  message
- [dev-server-browser-tests](./dev-server-browser-tests.md) — browser
  test harness for the dev server
- `crates/rolldown_dev/` — dev engine implementation
- `crates/rolldown_dev_common/` — `RebuildStrategy`, dev options
