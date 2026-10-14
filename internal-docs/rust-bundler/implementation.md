# Bundler

## Summary

`Bundler` is the long-lived, cache-preserving bundler used by watch mode, dev mode, and HMR. It creates `Bundle` instances for each build while persisting scan-stage caches and resolver state across builds. This is distinct from `ClassicBundler`, which creates a fresh factory for each build with no shared state — see [rust-classic-bundler.md](../rust-classic-bundler/implementation.md).

## Struct & Persistent State

```rust
// crates/rolldown/src/bundler/bundler.rs
pub struct Bundler {
    session: rolldown_devtools::Session,
    bundle_factory: BundleFactory,
    cache: ScanStageCache,
    closed: bool,
}
```

- **`BundleFactory`** — Reused across builds. Holds the shared resolver, plugin driver factory, file emitter, and options. Each build calls `factory.create_bundle()` to produce a fresh `Bundle` without discarding the factory.
- **`ScanStageCache`** — Persists the module graph, barrel state, and module index maps across builds. Swapped in/out of `Bundle` via `with_cached_bundle()` so incremental builds only re-scan changed modules.
- **`SharedResolver`** — Owned by the factory, shared across builds. The resolution cache survives between builds.
- **`closed`** — Owner guard that rejects new builds after close, see "Close mechanism" below.

`Bundler` derefs to `BundleFactory`, so callers can access factory fields directly (e.g. `bundler.options`, `bundler.resolver`).

## Build Lifecycle

Each build goes through `with_cached_bundle_experimental`:

```rust
pub async fn with_cached_bundle_experimental<T>(
    &mut self,
    bundle_mode: BundleMode,
    with_fn: impl AsyncFnOnce(&mut Bundle) -> BuildResult<T>,
) -> BuildResult<T>
```

1. Takes the current `ScanStageCache` out of `self`
2. Calls `bundle_factory.create_bundle(bundle_mode, Some(cache))` to produce a `Bundle`
3. Passes `&mut Bundle` to the closure — the caller orchestrates scan/render/write phases
4. Stores the cache back into `self` when the closure returns

The watch mode closure typically does:

```rust
bundler.with_cached_bundle_experimental(FullBuild, |bundle| async {
    let scan_output = bundle.scan_modules(scan_mode).await?;
    // register FS watches from bundle.get_watch_files() BEFORE render
    let output = bundle.bundle_write(scan_output).await?;
    Ok(output)
}).await
```

## Bundle

```rust
// crates/rolldown/src/bundle/bundle.rs
pub struct Bundle {
    fs: OsFileSystem,
    options: SharedOptions,
    resolver: SharedResolver,
    file_emitter: SharedFileEmitter,
    plugin_driver: SharedPluginDriver,
    warnings: Vec<BuildDiagnostic>,
    cache: ScanStageCache,
    bundle_span: tracing::Span,
}
```

A `Bundle` represents a single build. Its consuming methods (`write()`, `generate()`, `scan()`) take ownership of `self` to enforce single-use semantics.

For watch mode, the non-consuming methods (`scan_modules()`, `bundle_write()`, `bundle_generate()`, `get_watch_files()`) allow manual phase orchestration via `with_cached_bundle_experimental`.

### Close mechanism

`closeBundle` is a per-build concern, so its state lives on `BundleHandle`.
`Bundler::close()` sets `closed` (new builds are rejected) and closes the
latest handle.

- Before each `write`/`generate`/`scan`, the bundler closes the previous
  handle (`ensure_last_bundle_closed`); a no-op if it is already closed.
- `BundleHandle.close()` does not reset cache or resolver data, so closing a
  watch result does not force the next build cold.

### `BundleHandle.close()` — Design Decision

`BundleHandle` owns a `close()` method that:

1. Calls the `closeBundle` plugin hook
2. Is **idempotent** — calling close twice is safe (no-op on second call, tracked via `Arc<AtomicBool>`)
3. Turns a panicking hook into an error and clears the plugin driver's
   retained resources on every outcome

This is the correct place because `closeBundle` signals that no more output processing will happen for a specific build. The watcher's BUNDLE_END/ERROR event data carries a `BundleHandle` (not the full bundler), and JS `result.close()` calls `handle.close()` directly — no bundler lock needed.

## Relationship to Watcher

`rolldown_watcher` owns the build lifecycle:

1. Each `WatchTask` holds an `Arc<async_lock::Mutex<Bundler>>` (imported as `TokioMutex` in `watch_task.rs`)
2. On rebuild, the coordinator locks the bundler, calls `with_cached_bundle_experimental`, and orchestrates scan/write phases
3. The emitted `BUNDLE_END`/`ERROR` result owns that build's `BundleHandle`; JavaScript calls `event.result.close()` when finished, and the handle remains valid across later rebuilds
4. Watcher shutdown closes the latest handle as a backstop after `closeWatcher`

`BindingWatcherBundler::close` calls `BundleHandle.close()` directly, without
the bundler lock.

## Related

- [rust-classic-bundler](../rust-classic-bundler/implementation.md) — Rollup API compatibility wrapper
- [watch-mode](../watch-mode/implementation.md) — Watch mode architecture and lifecycle
- `crates/rolldown/src/bundler/` — Bundler implementation
- `crates/rolldown/src/bundle/` — Bundle and BundleFactory implementation
