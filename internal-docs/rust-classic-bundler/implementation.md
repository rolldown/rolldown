# ClassicBundler

## Summary

`ClassicBundler` is the Rollup API compatibility wrapper for one-time builds. It lives in the NAPI binding layer and implements the two-step `build()` + `write()`/`generate()` pattern that Rollup exposes. Each call creates a completely fresh `BundleFactory` and `Bundle` with no shared state — no caching, no incremental rebuilds.

## The Rollup API Compatibility Problem

Rollup's JS API is:

```javascript
const bundle = await rollup(inputOptions); // build step
await bundle.write(outputOptions); // output step
bundle.close(); // cleanup
```

This is a **two-step pattern**: the build is separate from the output. Rolldown's internal `Bundle` combines both into a single operation (`write()` or `generate()` consumes the bundle). `ClassicBundler` bridges this gap by providing the Rollup-compatible surface while delegating to Rolldown's internals.

## Struct

```rust
// crates/rolldown_binding/src/classic_bundler.rs
pub struct ClassicBundler {
    session_id: Arc<str>,
    debug_tracer: Option<rolldown_devtools::DebugTracer>,
    session: rolldown_devtools::Session,
    closed: bool,
    last_bundle_handle: Option<BundleHandle>,
}
```

Each `create_bundle()` call:

1. Checks the `closed` flag — rejects if already closed
2. Creates a **fresh `BundleFactory`** with the provided options and plugins
3. Creates a `Bundle` with `FullBuild` mode and **no cache** (`None`)
4. Stores the `BundleHandle` for later cleanup

There is no persistent state between builds. No `ScanStageCache`, no shared resolver, no reused factory.

## Key Differences from Bundler

| Aspect             | `Bundler`                    | `ClassicBundler`                   |
| ------------------ | ---------------------------- | ---------------------------------- |
| Location           | `crates/rolldown/`           | `crates/rolldown_binding/`         |
| BundleFactory      | Created once, reused         | Fresh each `create_bundle()` call  |
| ScanStageCache     | Persisted across builds      | None                               |
| SharedResolver     | Shared, cache survives       | Fresh each build                   |
| Incremental builds | Supported                    | Not supported                      |
| Use case           | Watch mode, dev mode, HMR    | Rollup-compatible `rollup()` API   |
| Close semantics    | Being refactored (see below) | User-facing `closed` flag, correct |

## Close Mechanism

The `closed` flag on `ClassicBundler` is **user-observable** — it's what `RolldownBuild.closed` checks in the JS API. This is correct and stays:

```rust
pub fn close(&mut self) -> impl Future<Output = anyhow::Result<()>> + Send + 'static {
    self.closed = true;
    // calls plugin_driver.close_bundle(None) on the last bundle handle
}

pub fn closed(&self) -> bool {
    self.closed
}
```

This is fundamentally different from `Bundler.closed`:

- **`ClassicBundler.closed`** — User-facing API contract. "This build result is done, don't call write/generate again." Correct.
- **`Bundler.closed`** — Owner guard. `Bundler::close()` sets it, then `write`/`generate`/`scan` reject with "Bundler is closed" and a second `close()` is a no-op. `closeBundle` itself is per-build state on `BundleHandle`; see [rust-bundler](../rust-bundler/implementation.md), "Close mechanism".

`close()` takes the last `BundleHandle` (`take`, not `clone`): a copy left on
the struct would be freed only by the N-API finalizer, which never runs on
threadless WASI. A second close therefore finds no handle and does not run
`closeBundle` again. `closeBundle` runs under `catch_unwind`, so a panicking hook
becomes an error.

Because the handle is gone, a second native close resolves at once, before the first one's `closeBundle` ends. The JS side guarantees one call: `RolldownBuild.close()` in `packages/rolldown/src/api/rolldown/rolldown-build.ts` keeps the first close promise, and a repeated `close()` only waits for it (the first caller gets any error). Inside that first close every cleanup step runs exactly once, even when an earlier one fails: it stops the parallel-plugin workers, then always calls `BindingBundler.close()`, and only then rethrows a worker shutdown error.

## Related

- [rust-bundler](../rust-bundler/implementation.md) — Long-lived bundler for watch/dev/HMR
- `crates/rolldown_binding/src/classic_bundler.rs` — Implementation
