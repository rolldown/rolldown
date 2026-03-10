# Module ID

## Summary

Module IDs are the primary keys for the entire bundler — module graph, caches, plugin APIs, HMR, watch files. In Rolldown they're string-based (`ArcStr`), so path identity depends on exact string equality. This doc describes how paths flow through the system, where mismatches can occur, and how Rollup handles the same problem.

## How Rollup Does It

Rollup uses a **single normalization point** design. The `resolveId` hook (and its default implementation via `path.resolve()`) is the one place where paths are normalized. The resolved path becomes the module ID used everywhere — module graph, caches, `graph.watchFiles`, plugin hooks, etc.

**Module IDs use native OS separators.** On Windows, module IDs contain `\` separators (e.g. `D:\project\src\main.js`). The `path.resolve()` output is stored as-is — no separator normalization is applied to module IDs. ([Verified on Windows CI](https://github.com/hyf0-agent/rollup-win-test/actions/runs/22542074808))

Rollup does have a `normalize` function that converts `\` to `/`:

```javascript
// rollup/src/utils/path.ts
const BACKSLASH_REGEX = /\\/g;
export function normalize(path) {
  return path.replace(BACKSLASH_REGEX, '/');
}
```

However, this is **only used in downstream/output contexts**, not in the core module ID pipeline:

- `pluginFilter.ts` — normalizes IDs before matching include/exclude patterns
- `Chunk.ts` — generating preserveModules chunk file names
- `renderChunks.ts` — source map source paths
- `relativeId.ts` — computing relative import paths
- `MetaProperty.ts` — import.meta relative paths

Plugin APIs like `addWatchFile()` do **no normalization** — they trust the caller to provide a path consistent with the module ID convention.

## How Rolldown Does It Today

### ModuleId

`ModuleId` wraps `ArcStr`. Equality is raw string comparison — no path normalization.

```rust
// rolldown_common/src/types/module_id.rs
pub struct ModuleId { inner: ArcStr }
```

The resolver (`oxc_resolver`) returns a `PathBuf`. Rolldown converts it to a string via `full_path().to_str()` and stores it as-is — no separator normalization. On Windows, module IDs contain native `\` separators.

### Comparison with Rollup

|                      | Rollup                           | Rolldown                         |
| -------------------- | -------------------------------- | -------------------------------- |
| Module ID on Windows | `C:\Users\project\src\file.js`   | `C:\Users\project\src\file.js`   |
| Module ID on Linux   | `/home/user/project/src/file.js` | `/home/user/project/src/file.js` |
| Normalization        | None (native OS separators)      | None (native OS separators)      |
| Platform-dependent?  | Prefix **and** separators        | Prefix **and** separators        |

Rollup and Rolldown are **aligned** here — both store `path.resolve()` / resolver output as-is, with native OS separators. The `normalize` function in Rollup only applies in downstream/output contexts (see above), not to module IDs.

Note: some plugins may internally assume `/` separators when doing string matching on module IDs. This is a plugin-level concern, not a Rollup-vs-Rolldown divergence.

### StableModuleId

`StableModuleId` is a cwd-relative, forward-slash-normalized version of `ModuleId`. Used for cross-machine stability (source maps, HMR client-side references).

```rust
// Absolute → relative from cwd, forward slashes
// "\0foo" → "\\0foo" (virtual module escape)
// "fs" → "fs" (non-path specifiers unchanged)
```

### Where Path Identity Matters

| Subsystem                  | Key type                  | Normalization                        | Risk                                              |
| -------------------------- | ------------------------- | ------------------------------------ | ------------------------------------------------- |
| Module graph lookup        | `ModuleId` (ArcStr)       | None                                 | Resolver output must be consistent                |
| Scan stage cache           | `ModuleId` → `VisitState` | None                                 | Same path resolved differently = duplicate module |
| `module_idx_by_abs_path`   | `ArcStr`                  | `to_slash()` at insertion            | HMR changed-file paths must match                 |
| Plugin `get_module_info()` | `&str` lookup             | None                                 | Plugin must use exact module ID                   |
| Plugin `add_watch_file()`  | `ArcStr` into `FxDashSet` | None                                 | Watch set uses raw strings                        |
| Watch file comparison      | `ArcStr` eq               | `#[cfg(windows)]` backslash fallback | Fragile                                           |
| Resolver package cache     | `PathBuf`                 | PathBuf component comparison         | Handles separator differences                     |

### Existing Normalization Utilities

- `PathExt::expect_to_slash()` — Converts `\` to `/` (only on non-Unix platforms). Used in `StableModuleId`, HMR, source maps.
- `SugarPath::relative()` — Produces relative paths. Used in `StableModuleId`.
- `stabilize_id()` — Absolute → cwd-relative with forward slashes. Legacy utility, functionality now in `StableModuleId`.

## The Core Problem

Module IDs are strings, and different parts of the system produce path strings differently:

1. **Resolver** produces absolute paths (platform-native separators)
2. **Plugins** provide paths via `addWatchFile()` (no normalization guaranteed)
3. **notify crate** reports file change events with OS-native paths
4. **HMR client** sends stable IDs (relative, forward slashes)

If any two of these disagree on how to represent the same file, lookups silently fail — the module isn't found, the cache misses, the watch file isn't matched, the HMR update is dropped.

Today this mostly works because the resolver is consistent with itself, and most lookups use the resolver's output on both sides. The fragile spots are at **boundaries** — where an externally-produced path (notify event, plugin input, HMR client) is compared against a resolver-produced module ID.

## `PathBuf` Comparison Behavior

`Path`/`PathBuf` comparison works by comparing [components](https://doc.rust-lang.org/std/path/struct.Components.html), not raw bytes. From the [official docs](https://doc.rust-lang.org/std/path/index.html): normalization disregards "repeated separators, non-leading `.` components, and trailing separators" for iteration, inspection, and comparisons. On Windows, both `/` and `\` are treated as separators.

| Scenario                               | `str` eq | `PathBuf` eq           |
| -------------------------------------- | -------- | ---------------------- |
| `/foo/bar` vs `/foo/bar/`              | false    | **true**               |
| `/foo//bar` vs `/foo/bar`              | false    | **true**               |
| `/foo/./bar` vs `/foo/bar`             | false    | **true**               |
| `/foo/../foo/bar` vs `/foo/bar`        | false    | false                  |
| (Windows) `C:\foo\bar` vs `C:/foo/bar` | false    | **true**               |
| `/foo/Bar` vs `/foo/bar`               | false    | false (case sensitive) |

Hash is consistent with equality — safe to use in `HashSet`/`HashMap`.

**Limitation:** `PathBuf` does not resolve `..` or symlinks. For that you need `fs::canonicalize()`, which has its own downsides (resolves symlinks, may fail for nonexistent paths).

## Unresolved Questions

- **Should module IDs be normalized at creation time?** Rollup does **not** normalize module ID separators — on Windows, plugins see `\` in module IDs. Rolldown currently matches this behavior. Should Rolldown diverge and normalize to `/` in `ModuleId::new()` for simpler cross-platform logic? This would change the observable module ID on Windows but could simplify plugin filter matching and internal comparisons.

- **Should the watch file set use `PathBuf` instead of `ArcStr`?** `PathBuf` handles trailing slashes, double slashes, `.` segments, and Windows separators. The downside is losing cheap `ArcStr` cloning and `&str` lookups. See [watch-mode.md](./watch-mode.md) for the watch-specific discussion.

- **`..` segments and symlinks** — Neither `PathBuf` comparison nor string comparison handles these. In practice, `..` shouldn't appear in resolver output (resolvers canonicalize), and symlinks are a rare edge case. Should Rolldown guarantee anything here?

## Related

- [watch-mode](./watch-mode.md) — Watch file set path matching
- `crates/rolldown_common/src/types/module_id.rs` — `ModuleId` type
- `crates/rolldown_common/src/types/stable_module_id.rs` — `StableModuleId` type
- `crates/rolldown_std_utils/src/path_ext.rs` — `expect_to_slash()` utility
