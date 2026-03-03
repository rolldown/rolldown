# Bundler

## Summary

`Bundler` is the long-lived, cache-preserving bundler used by watch mode, dev mode, and HMR. It creates `Bundle` instances for each build while persisting scan-stage caches and resolver state across builds. This is distinct from `ClassicBundler`, which creates a fresh factory for each build with no shared state — see [rust-classic-bundler.md](./rust-classic-bundler.md).

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
- **`closed`** — Legacy flag, see "Close Mechanism" below.

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
    bundle_span: Arc<tracing::Span>,
}
```

A `Bundle` represents a single build. Its consuming methods (`write()`, `generate()`, `scan()`) take ownership of `self` to enforce single-use semantics.

For watch mode, the non-consuming methods (`scan_modules()`, `bundle_write()`, `bundle_generate()`, `get_watch_files()`) allow manual phase orchestration via `with_cached_bundle_experimental`.

### `Bundler` doesn't need `close()`

`Bundler` is a long-lived Rust struct. Resources clean up on drop — there's nothing to "close." The only meaningful work `Bundler::close()` does today is call the `closeBundle` plugin hook, but that's a **per-build lifecycle concern**, not a per-bundler concern. It belongs on the build artifact (`BundleHandle`), not on the bundler.

The current `Bundler::close()` also resets the scan stage cache and clears the resolver cache — but these are rebuild concerns, not close concerns. In watch mode, destroying caches on `result.close()` is actively harmful (forces a cold rebuild).

The following should be removed from `Bundler`:

- **`closed` flag** — No purpose once `closeBundle` moves to `BundleHandle`.
- **`inner_close()`** — Its only real job (calling `closeBundle`) moves to `BundleHandle.close()`. Cache/resolver cleanup happens on drop.
- **`reset_closed_for_watch_mode()`** — This hack exists because `BindingWatcherBundler.close()` calls `bundler.close()` which sets `closed = true`, requiring a reset before each rebuild. With `closeBundle` on `BundleHandle`, no reset is needed.
- **`create_error_if_closed()`** — Callers that need a closed guard (`ClassicBundler`, `DevEngine`) have their own `closed` flags.
- **`close()`** — Removed entirely.

### `BundleHandle.close()` — Design Decision

`BundleHandle` should own a `close()` method that:

1. Calls the `closeBundle` plugin hook
2. Is **idempotent** — calling close twice is safe (no-op on second call, tracked via `Arc<AtomicBool>`)

This is the correct place because `closeBundle` signals that no more output processing will happen for a specific build. The watcher's BUNDLE_END/ERROR event data carries a `BundleHandle` (not the full bundler), and JS `result.close()` calls `handle.close()` directly — no bundler lock needed.

## Relationship to Watcher

`rolldown_watcher` owns the build lifecycle:

1. Each `WatchTask` holds an `Arc<TokioMutex<Bundler>>`
2. On rebuild, the coordinator locks the bundler, calls `with_cached_bundle_experimental`, and orchestrates scan/write phases
3. After each build, `rolldown_watcher` should call `Bundle.close()` (or `BundleHandle.close()`) to fire `closeBundle` — this is the watcher's responsibility, not something JS reaches in to do
4. On watcher close, the bundler is dropped, cleaning up resources

This means `BindingWatcherBundler` should NOT call `bundler.close()` — the `closeBundle` hook is the contract of `rolldown_watcher`, triggered at the right point in the build lifecycle.

## Related

- [rust-classic-bundler](./rust-classic-bundler.md) — Rollup API compatibility wrapper
- [watch-mode](./watch-mode.md) — Watch mode architecture and lifecycle
- `crates/rolldown/src/bundler/` — Bundler implementation
- `crates/rolldown/src/bundle/` — Bundle and BundleFactory implementation
