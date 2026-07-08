# The Dev Engine — Design & Principles (`rolldown_dev`, Full Bundle Mode)

> **Implementation map** — component layering, the `CoordinatorMsg`
> protocol, the `CoordinatorState` machine, the `TaskInput` work types, and
> the per-stage data-flow pipelines: see
> [implementation.md](./implementation.md). The `§N` section references
> below point to that file.

## Summary

The dev engine (`rolldown_dev` crate) is rolldown's dev-mode build
orchestration layer in Full Bundle Mode. It sits between the file watcher
/ dev server and the core `Bundler`, deciding _what_ build to run — an HMR
patch, an incremental rebuild, or a full build — and _when_. It is
structured as a `DevEngine` (the public async API surface) driving a
single message-loop `BundleCoordinator` (a state machine plus a work
queue) that spawns one `BundlingTask` at a time.

This document captures the **why** — the principles that govern when the
engine rebuilds and how its errors flow out to the binding consumer. For
the machinery that realizes them, see
[implementation.md](./implementation.md).

## Design principles

Five principles govern when the dev engine rebuilds and how its errors
flow out to the binding consumer. They define rolldown_dev's contract
with its consumer (typically Vite) and constrain the implementation in
§7, §13, and §16.

### 1. Conservative rebuilds

Rebuilds happen only when the bundle is **stale** — when input has
changed since the last build attempt. Page access and browser
reconnect on their own never trigger a rebuild. In particular: if the
previous build failed, an access request does not retry — without new
input the same error would recur.

Realized in: `BundleCoordinator::ensure_latest_bundle_output` returning
`None` for `Failed` / `FullBuildFailed` (§13b, §13e).

### 2. Errors are emitted on every build

rolldown_dev surfaces build errors to the binding consumer on every
build via the `on_output` / `on_hmr_updates` callbacks (§16b). It never
silently retries past an error, never silently swallows one, and never
caches one across requests — rolldown_dev is stateless across HTTP
requests. The binding consumer (Vite) is responsible for retaining the
most recent error and replaying it on each client reconnect, so the
error overlay appears even after a browser refresh.

Vite-side realization (in `fullBundleEnvironment.ts`): a single
`lastBuildError: Error | null` field caches the most recent error from
**either** channel — it is set in both `onOutput` (full-build errors)
and `onHmrUpdates` (HMR errors), and cleared back to `null` on a
successful build from **either** channel (a successful `onOutput` _or_
a successful `onHmrUpdates`, since an HMR patch that computes cleanly
supersedes a previously cached error). It is replayed on the **`vite:client:connect`**
event for every freshly connected client (including a post-refresh
reconnect), so the error overlay reappears after a browser refresh.
The two channels differ only in their _live_
delivery: an `onOutput` error is additionally logged to the terminal
(`logger.error`) so a build break is visible without a browser, and is
broadcast to all clients via `hot.send`; an `onHmrUpdates` error is sent
to each connected client individually and is not logged to the terminal.

### 3. File changes are the only recovery trigger

After a failed build, the engine waits for a file change before
rebuilding. Both Vite config edits and user-land source edits are
valid triggers. Inside rolldown_dev nothing else counts as recovery —
not page refresh, not elapsed time, not manual UI dismissal:
`ensure_latest_bundle_output` no-ops in every failed state (§13b), so
access never rebuilds on its own.

**One consumer-side exception — page refresh after an HMR-stage
failure.** When the last failure originated in HMR generation
(`last_error_stage == Hmr`), the consumer is permitted to treat a page
refresh as a recovery trigger: on access it calls `triggerFullBuild`
(§13e) to force a full rebuild that bypasses the possibly-buggy HMR
path, instead of replaying the cached error. This stays scoped to the
consumer — rolldown_dev itself does not change behavior; the escalation
is the consumer's decision, keyed on the `last_error_stage` it reads
from `BundleState` (§12). A `Rebuild`-stage or full-build failure gets
no such exception — only a file change recovers those. (Wired up in the
in-repo reference consumer: `triggerBundleRegenerationIfStale` in
`packages/test-dev-server/src/environments/full-bundle-dev-environment.ts`.)

Realized in: `handle_file_changes` (§7) is the sole producer of
post-failure rebuild tasks. `triggerFullBuild` (§13e) is an explicit
escape hatch for cases the watcher cannot observe (e.g. missing-import
resolution; see Unresolved Questions).

Corollary: a file change after a failed build must schedule work that
can undo the failure. In practice this means tracking where the
failure originated (HMR computation vs incremental rebuild) so the
next task covers the stage that broke (§7).

### 4. Build errors are recoverable; panics are bugs

Every error reaching the consumer via `on_output` / `on_hmr_updates`
is treated as a **user error** — caused by source code or plugin
behavior, recoverable by editing source. Rolldown and Vite themselves
are assumed bug-free in this model. The only state not recoverable
through a file-change cycle is a panic, which signals an invariant
violation in rolldown_dev itself (§16g).

### 5. Quiesce before terminal cleanup

Closing publishes the engine's closed state immediately so new work is
rejected. The JavaScript and binding owners then drain public operations before
asking the coordinator to close. This ordering lets an active `run()` finish
its final state request before the coordinator enters `closeBundle`; otherwise
a hook that awaits that `run()` can form a cycle. Reentrant callback close
remains possible because the close-identity dependency graph acknowledges the
callback's close request, lets the callback and operation guard return, and
keeps the full terminal result for an external or later caller.
The raw N-API owner provides the same two-phase behavior when one of its own
dev callbacks is active: the callback receives a close acknowledgement, native
cleanup continues after accepted operations drain, and the next non-callback
close caller receives the terminal result. This is intentionally conservative
for direct binding consumers because N-API cannot distinguish an async callback
continuation from unrelated JavaScript while that callback promise is pending.

The coordinator separately drains the active HMR/rebuild task. That task may
install a replacement bundle handle, so only the coordinator can identify and
close the final handle after the task settles. Its `closeBundle` hooks finish
while parallel-plugin workers are still alive; worker shutdown follows native
close. Concurrent and later `close()` callers share and replay the same
terminal success or failure instead of returning before cleanup completes or
retrying a partially consumed hook chain. All terminal diagnostics cross N-API
individually, preserving JavaScript error identities, and the JavaScript owner
aggregates them exactly once.

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

## Related

- [implementation.md](./implementation.md) — the dev engine's
  implementation map (components, message protocol, state machine,
  per-stage data flow)
- [bundler-data-lifecycle](../bundler-data-lifecycle/implementation.md) — `BundleMode`,
  `Bundle` / `BundleFactory`, and the `ScanStageCache` lifecycle the dev
  engine's incremental builds run through
- [rust-bundler](../rust-bundler/implementation.md) — the core `Bundler` struct and build
  lifecycle the dev engine drives
- [watch-mode](../watch-mode/implementation.md) — `rolldown_watcher`, the actor-based
  watch architecture; `rolldown_dev` reuses the same actor pattern
- [lazy-compilation](../lazy-compilation/implementation.md) — lazy entry compilation,
  reached via `DevEngine::compile_lazy_entry` and the `ModuleChanged`
  message
- [dev-server-test-harness](../dev-server-test-harness/implementation.md) — browser
  test harness for the dev server
