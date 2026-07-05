# Watch Mode — Design & Principles

> **Implementation** — actor architecture, API contract, state machine,
> debouncing, event lifecycle, the NAPI bridge, and migration status: see
> [implementation.md](./implementation.md).

## Summary

Watch mode monitors source files and automatically rebuilds when changes
are detected. The `rolldown_watcher` crate is the foundation, using a clean
actor-based architecture. This doc captures the principles that govern the
design and the open questions; the machinery lives in
[implementation.md](./implementation.md).

## Design Principles

- **JS API aligns with Rollup** — The TypeScript surface (events, options, plugin hooks, lifecycle ordering) should match Rollup's behavior unless there's a technical reason not to. Divergences are documented explicitly.
- **Close is complete from the creation tick onward** — Calling `close()` immediately after `watch()` must still start the native coordinator, run `closeWatcher`, stop parallel-plugin workers, and emit `close`. No build means no synthetic `closeBundle`.
- **Close is re-entrant** — A watcher event listener may call and await `watcher.close()`. Native cleanup and listener completion are separate memoized phases: a close listener awaits the completed native phase, while outside callers await the full phase including every close listener. Listener failures reject the full close promise coherently.
- **Rust code follows Rust idioms** — The Rust core should feel native: ownership-driven, enum state machines, trait-based extensibility, no unnecessary `Arc`/`Mutex` beyond what the architecture requires.
- **Consistent naming across the stack** — Rollup defines the canonical event/concept names (e.g. `BUNDLE_START`/`BUNDLE_END`). The Rust side should use the same terminology so there's a clean 1:1 mapping and no mental translation at the NAPI boundary.

## Unresolved Questions

- **Build should not block the coordinator loop** — Currently the coordinator `await`s builds inline, blocking the entire event loop. The dev engine (`BundleCoordinator`) solves this by `tokio::spawn`ing builds — the coordinator loop stays responsive to messages while builds run. On `Close`, the dev engine still waits for the running build to finish gracefully, but the point is it _receives_ the message immediately rather than being blocked. The watcher should follow the same pattern — spawn builds, receive completion messages back, and keep the loop free to process events during builds.

- **Parallel task builds** — `watch([configA, configB])` builds tasks sequentially (matching Rollup), while calling `watch(configA); watch(configB)` separately runs them in parallel (separate coordinators). This means sequential execution isn't a meaningful guarantee — users can trivially opt into parallelism by splitting calls. Should we just parallelize tasks within a single coordinator too?

- **Shared vs per-task FsWatcher** — Currently each `WatchTask` owns its own `DynFsWatcher`. If two tasks watch the same file, it's watched twice at the OS level. A single shared `DynFsWatcher` at the coordinator level would deduplicate watches and use fewer OS resources. Adding files is straightforward. Unwatching (not yet implemented) would require cross-task coordination — a file can only be unwatched when no task needs it (reference counting or a union check across task watch sets). Since unwatching isn't implemented yet, a shared watcher would be strictly simpler today.

- **Watch files not persisted across builds** — `bundler.watch_files()` returns the watch set from the latest build, but this set is not persisted between builds. With full rebuilds this is fine (each build produces a complete set). But with incremental builds, only a subset of modules are re-processed, so the incremental build's `watch_files()` would be incomplete — it wouldn't include files from modules that weren't re-visited. The watch set needs to be accumulated/persisted across builds, not replaced each time.

## Related

- [implementation.md](./implementation.md) — the watch-mode implementation map
- [rust-bundler](../rust-bundler/implementation.md) — Core Bundler struct and `Bundle.close()` design
- [rust-classic-bundler](../rust-classic-bundler/implementation.md) — Rollup API compatibility wrapper
- [module-id](../module-id/implementation.md) — Module ID, path identity, and normalization
- [#6482](https://github.com/rolldown/rolldown/issues/6482) — Watch mode issue collection (tracks all known bugs)
