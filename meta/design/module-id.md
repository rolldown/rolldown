# Module ID

## Summary

Module IDs are the primary keys for the entire bundler — module graph, caches, plugin APIs, HMR, watch files. In Rolldown they're string-based (`ArcStr`), so path identity depends on exact string equality. This doc describes how paths flow through the system, where mismatches can occur, and how Rollup handles the same problem.

## How Rollup Does It

Rollup uses a **single normalization point** design. The `resolveId` hook (and its default implementation via `path.resolve()`) is the one place where paths are normalized. The resolved path becomes the module ID used everywhere — module graph, caches, `graph.watchFiles`, plugin hooks, etc.

On top of that, Rollup has an explicit **backslash-to-slash normalization**:

```javascript
// rollup/src/utils/path.ts
const BACKSLASH_REGEX = /\\/g;
export function normalize(path) {
  return path.replace(BACKSLASH_REGEX, '/');
}
```

This is applied to module IDs, plugin filters, source maps, and chunk file names. The result: **module IDs always use forward slashes**, even on Windows. The same file gets a different absolute prefix on Windows vs Linux (`C:/Users/...` vs `/home/...`), but separators are always `/`.

Plugin APIs like `addWatchFile()` do **no normalization** — they trust the caller to provide a path consistent with the module ID convention.

## How Rolldown Does It Today

### ModuleId

`ModuleId` wraps `ArcStr`. Equality is raw string comparison — no path normalization.

```rust
// rolldown_common/src/types/module_id.rs
pub struct ModuleId { inner: ArcStr }
```

The resolver (`oxc_resolver`) returns a `PathBuf`. Rolldown converts it to a string via `full_path().to_str()` and stores it as-is — no separator normalization. On Windows, module IDs contain native `\` separators.

### Divergence from Rollup

|                      | Rollup                           | Rolldown                         |
| -------------------- | -------------------------------- | -------------------------------- |
| Module ID on Windows | `C:/Users/project/src/file.js`   | `C:\Users\project\src\file.js`   |
| Module ID on Linux   | `/home/user/project/src/file.js` | `/home/user/project/src/file.js` |
| Normalization        | `\` → `/` at `resolveId` level   | None                             |
| Platform-dependent?  | Prefix only (`C:/` vs `/home/`)  | Prefix **and** separators        |

This means:

- **Rollup plugins running in Rolldown on Windows see `\` in module IDs** instead of the `/` they expect. This can break plugin logic that does string matching or manipulation on module IDs.
- **The same Rolldown project produces different module IDs on Windows vs Linux**, differing not just in the absolute prefix but also in separators.

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

- **Should module IDs be normalized at creation time?** Rollup normalizes backslashes to forward slashes globally. Rolldown could do the same in `ModuleId::new()`. This would make all downstream comparisons safe for the separator problem, but it changes the observable module ID on Windows (plugins would see `/` instead of `\`). Rollup plugins expect `/`, so this may actually be the correct behavior.

- **Should the watch file set use `PathBuf` instead of `ArcStr`?** `PathBuf` handles trailing slashes, double slashes, `.` segments, and Windows separators. The downside is losing cheap `ArcStr` cloning and `&str` lookups. See [watch-mode.md](./watch-mode.md) for the watch-specific discussion.

- **`..` segments and symlinks** — Neither `PathBuf` comparison nor string comparison handles these. In practice, `..` shouldn't appear in resolver output (resolvers canonicalize), and symlinks are a rare edge case. Should Rolldown guarantee anything here?

## Related

- [watch-mode](./watch-mode.md) — Watch file set path matching
- `crates/rolldown_common/src/types/module_id.rs` — `ModuleId` type
- `crates/rolldown_common/src/types/stable_module_id.rs` — `StableModuleId` type
- `crates/rolldown_std_utils/src/path_ext.rs` — `expect_to_slash()` utility
