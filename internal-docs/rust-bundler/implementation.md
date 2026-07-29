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

- **`BundleFactory`** — Reused across builds. Holds the shared resolver, plugin driver factory, and options. Each build calls `factory.create_bundle()` to produce a fresh `Bundle` without discarding the factory; emitter continuity is selected by `BundleMode`.
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
    bundle_span: tracing::Span,
}
```

A `Bundle` represents a single build. Its consuming methods (`write()`, `generate()`, `scan()`) take ownership of `self` to enforce single-use semantics.

For watch mode, the non-consuming methods (`scan_modules()`, `bundle_write()`, `bundle_generate()`, `get_watch_files()`) allow manual phase orchestration via `with_cached_bundle_experimental`.

### Close mechanism

`closeBundle` is a **per-build lifecycle concern**, so the terminal hook state
lives on `BundleHandle`. `Bundler::close()` remains the owner-level guard used
by dev/watch shutdown: it marks the bundler closed to reject further builds and
delegates to the latest handle. Repeated calls still await that handle's
memoized result instead of converting an earlier failure into success.

Cache and resolver data are not reset by `BundleHandle.close()`; those are
rebuild/drop concerns. In watch mode, `event.result.close()` therefore releases
the bundle's plugin-driver resources without forcing the next build cold.
Each full build owns an independent `FileEmitter`; only incremental builds
reuse the preceding handle's emitter intentionally. Closing a retained watch
result can therefore clear its own emitted-file state without invalidating a
newer full rebuild.

### `BundleHandle.close()` — Design Decision

`BundleHandle` should own a `close()` method that:

1. Calls the `closeBundle` plugin hook
2. Clears retained plugin-driver resources after the hook settles, including failure
3. Is **terminal and idempotent** — one shared future runs the hook once;
   concurrent callers wait for it, and later callers replay the same success or
   failure

This is the correct place because `closeBundle` signals that no more output processing will happen for a specific build. The watcher's BUNDLE_END/ERROR event data carries a `BundleHandle` (not the full bundler), and JS `result.close()` calls `handle.close()` directly — no bundler lock needed.

A failed close is not retried. Hook dispatch stops at the first failing plugin,
so earlier plugins may already have completed cleanup; rerunning the chain
could duplicate side effects. Failure replay gives every owner the same
observable result while exact-once execution preserves plugin lifecycle order.
The TypeScript watch event adapter does not replace this native memoization. It
compares the opaque identity shared by the result's `BundleHandle` and plugin
contexts, giving an immediate callback-scope acknowledgement only when
`closeBundle` re-enters that same underlying result. Closing another result
continues to await that result's shared terminal outcome, as do external and
later callers.

## Relationship to Watcher

`rolldown_watcher` owns the build lifecycle:

1. Each `WatchTask` holds an `Arc<TokioMutex<Bundler>>`
2. On rebuild, the coordinator locks the bundler, calls `with_cached_bundle_experimental`, and orchestrates scan/write phases
3. The emitted `BUNDLE_END`/`ERROR` result owns that build's `BundleHandle`; JavaScript calls `event.result.close()` when finished, and the handle remains valid across later rebuilds
4. Watcher shutdown closes the latest handle as a backstop after `closeWatcher`

`BindingWatcherBundler` therefore calls `BundleHandle.close()` directly rather
than taking the bundler lock or closing the whole watch task.

## Related

- [rust-classic-bundler](../rust-classic-bundler/implementation.md) — Rollup API compatibility wrapper
- [watch-mode](../watch-mode/implementation.md) — Watch mode architecture and lifecycle
- `crates/rolldown/src/bundler/` — Bundler implementation
- `crates/rolldown/src/bundle/` — Bundle and BundleFactory implementation
