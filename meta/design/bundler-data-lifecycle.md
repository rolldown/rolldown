# Bundler Data Lifecycle

## Summary

Rolldown data falls into two lifecycle tiers: **bundler-level** (lives across all builds) and **bundle-level** (scoped to a single build). Getting this wrong causes real bugs — lost plugin state between HMR rebuilds, unnecessary `ScanStageCache` materialization on non-incremental watch builds, mixed module metadata across full rebuilds. This doc defines which data belongs where and why.

## Background

The original design had `RolldownBuild` create a new `Bundler` for every `generate()`/`write()` call. This meant every build was a fully independent session — no shared state, no reuse. That's fine for one-shot builds, but makes incremental builds, HMR, and watch mode either impossible or fragile. The refactoring (rolldown/rolldown#6877 through rolldown/rolldown#6896) introduced the `BundleFactory`/`Bundle` split and `PluginDriverFactory` to give each piece of data a clear owner and lifetime.

## The Two Tiers

```
Bundler (long-lived)
  ├── BundleFactory (created once)
  │     ├── PluginDriverFactory
  │     ├── SharedResolver
  │     ├── SharedOptions
  │     ├── SharedFileEmitter
  │     ├── module_infos_for_incremental_build     ─┐
  │     └── transform_dependencies_for_incremental_build ─┤ shared via Arc with PluginDriver
  │
  ├── ScanStageCache (moves in/out of Bundle per build)
  │     ├── snapshot (NormalizedScanStageOutput)
  │     ├── module_id_to_idx
  │     ├── importers
  │     ├── barrel_state
  │     ├── module_idx_by_abs_path
  │     └── module_idx_by_stable_id
  │
  └── Session (devtools tracing)

Bundle (per-build, consumed after use)
  ├── PluginDriver (fresh instance, created by PluginDriverFactory)
  │     ├── plugins / contexts (fresh)
  │     ├── watch_files (fresh)
  │     ├── module_infos (Arc → bundler-level)
  │     └── transform_dependencies (Arc → bundler-level)
  ├── warnings
  └── bundle_span
```

### Tier 1: Bundler-Level (Persistent)

Data that survives across all builds. It is either immutable configuration or incrementally-maintained shared state.

| Data                                           | Why bundler-level                                                                                                                                                                                                                                                                                        |
| ---------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `SharedOptions`                                | Immutable config. No reason to recreate.                                                                                                                                                                                                                                                                 |
| `SharedResolver`                               | Expensive to construct; the resolver's internal cache improves rebuild speed.                                                                                                                                                                                                                            |
| `SharedFileEmitter`                            | File emission state must be consistent across builds (e.g. emitted asset dedup).                                                                                                                                                                                                                         |
| `PluginDriverFactory`                          | Plugin definitions don't change between builds. Only the per-build plugin _instances_ and _contexts_ do.                                                                                                                                                                                                 |
| `module_infos_for_incremental_build`           | Plugin-populated module metadata (via `this.getModuleInfo`). Must survive across incremental builds so plugins can query modules they didn't re-process.                                                                                                                                                 |
| `transform_dependencies_for_incremental_build` | `addWatchFile()` dependencies from plugins. Critical for HMR invalidation — must persist so the HMR stage knows which files affect which modules.                                                                                                                                                        |
| `ScanStageCache`                               | Module graph snapshot, module index maps, barrel state. Makes incremental builds possible — on `IncrementalBuild`, only changed modules are re-scanned and merged via `ScanStageCache::merge()`. Temporarily moved into `Bundle` during a build, then moved back (see "ScanStageCache Ownership" below). |

**Reset rules:** `module_infos` and `transform_dependencies` are reset to fresh `Arc::default()` on `FullBuild` and `IncrementalFullBuild` (via `BundleFactory::create_bundle`). They are preserved across `IncrementalBuild`.

### Tier 2: Bundle-Level (Per-Build)

Data created fresh for each build and discarded (or consumed) when the build completes.

| Data              | Why bundle-level                                                                                                                                          |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `PluginDriver`    | Plugin hooks carry per-build state (e.g. accumulated `watch_files`, per-module transform context). A stale driver from a previous build would leak state. |
| `watch_files`     | The set of files a build touched. Must be fresh — a file no longer imported shouldn't trigger rebuilds.                                                   |
| `warnings`        | Diagnostics are per-build output.                                                                                                                         |
| `bundle_span`     | Tracing span for this specific build.                                                                                                                     |
| Plugin `contexts` | `PluginContext` instances carry per-build references (resolver, file emitter handles).                                                                    |

### ScanStageCache Ownership

`ScanStageCache` is bundler-level data, but bundles need mutable access to it during a build. This is handled by temporarily moving it out of `Bundler` into `Bundle`, then moving it back. Managed by `with_cached_bundle_experimental`:

```
Bundler.cache (ScanStageCache) ──(move)──> Bundle.cache (temporary holder) ──(build)──> Bundle.cache ──(move)──> Bundler.cache
```

| `ScanStageCache` field    | Purpose                                             |
| ------------------------- | --------------------------------------------------- |
| `snapshot`                | Full module graph (modules, ASTs, symbols, entries) |
| `module_id_to_idx`        | Module ID to index lookup                           |
| `importers`               | Reverse dependency graph                            |
| `barrel_state`            | Barrel export optimization state                    |
| `module_idx_by_abs_path`  | Path-based lookup for watcher                       |
| `module_idx_by_stable_id` | Stable ID lookup for HMR                            |

## BundleMode

`BundleMode` makes the three incremental states explicit. Before this enum, the code used `ScanMode` + `is_incremental_build_enabled` combinations that were ambiguous and bug-prone.

```rust
pub enum BundleMode {
    FullBuild,              // Fresh ScanStageCache for this build; discard it afterward.
    IncrementalFullBuild,   // Fresh ScanStageCache for this build; retain it for later incremental builds.
    IncrementalBuild,       // Reuse existing ScanStageCache; only rescan changed files.
}
```

| Mode                   | `ScanStageCache` in | `ScanStageCache` out | Shared state reset | Use case                                   |
| ---------------------- | ------------------- | -------------------- | ------------------ | ------------------------------------------ |
| `FullBuild`            | None                | Discarded            | Yes                | One-shot build, non-incremental watch      |
| `IncrementalFullBuild` | None                | Saved                | Yes                | First build with `incremental: true`       |
| `IncrementalBuild`     | Existing            | Updated              | No                 | Subsequent builds with `incremental: true` |

**Key distinction:** `IncrementalFullBuild` vs `FullBuild` — both do a full scan, but `IncrementalFullBuild` retains the resulting `ScanStageCache` for later incremental builds. Without this distinction, watch mode with `incremental: false` was silently paying the cost of materializing and retaining scan-stage state on every rebuild for no benefit.

## PluginDriverFactory

The `PluginDriverFactory` is what makes the bundler-level / bundle-level split work for plugins. It holds the plugin _definitions_ (bundler-level) and produces fresh `PluginDriver` _instances_ (bundle-level) for each build.

The factory also owns the `Arc`s for `module_infos` and `transform_dependencies`. When it creates a `PluginDriver`, it clones these Arcs into the driver. This means:

- Each bundle's `PluginDriver` writes into the **same** underlying `module_infos` map (for incremental builds)
- On full builds, the factory replaces its Arcs with fresh ones before creating the driver, so previous data is dropped

This is what fixed the bug where `this.getModuleInfo()` returned nothing on the second HMR rebuild — the old code created entirely new plugin contexts with no connection to the previous build's module info.

## Bugs Found by This Separation

1. **Lost `module_infos` across HMR rebuilds** (rolldown/rolldown#6891) — Each build created fully independent plugin contexts. `this.getModuleInfo()` in `transform` returned nothing on the second build because the new context had an empty module info map. Fix: `module_infos` became bundler-level, shared via Arc through `PluginDriverFactory`.

2. **No `ScanStageCache` on first incremental build** (rolldown/rolldown#6894) — With `incremental: true`, the first `generate()` called `create_bundle()` (i.e. `FullBuild`) instead of `IncrementalFullBuild`, so no `ScanStageCache` was retained. The second call to `incremental_generate()` panicked because it expected an existing `ScanStageCache`. Fix: `BundleMode` makes the distinction explicit.

3. **Mixed module infos between IncrementalFullBuild calls** (rolldown/rolldown#6894) — If too many files changed, dev mode triggers a second `IncrementalFullBuild`, but the code only cleared `module_infos` in `create_bundle()` (for `FullBuild`), not in the incremental bundle creation path. Two builds' metadata got mixed. Fix: single `create_bundle(BundleMode, Option<ScanStageCache>)` method that handles all modes.

4. **Unnecessary `ScanStageCache` materialization in non-incremental watch mode** (rolldown/rolldown#6894) — Earlier versions materialized scan-stage state even when watch mode ran with `incremental: false`, making the separation problem visible. `BundleMode` made this explicit. Current code resets `ScanStageCache` when incremental build is disabled (see `Bundle::scan_modules()`), so it is no longer retained across non-incremental builds.

## Unresolved Questions

- `Bundler::close()` still exists with a `closed` flag, but `closeBundle` is a per-build concern. It should move to `BundleHandle` — see [rust-bundler.md](./rust-bundler.md).

## Related

- [rust-bundler](./rust-bundler.md) — Bundler struct and build lifecycle
- [rust-classic-bundler](./rust-classic-bundler.md) — Rollup API compatibility wrapper (no shared state)
- rolldown/rolldown#6877 — Introduced Build abstraction
- rolldown/rolldown#6883 — BuildFactory for Bundler
- rolldown/rolldown#6886 — Build/BuildFactory renamed to Bundle/BundleFactory
- rolldown/rolldown#6891 — PluginDriverFactory
- rolldown/rolldown#6894 — BundleMode enum
