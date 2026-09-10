# Persistent Build Cache - Implementation

> Rationale, trade-offs and limitations live in [design.md](./design.md).

## Summary

`experimental.buildCache` persists the result of a module task's plugin
pipeline per module: the post-transform code, module type, sourcemap chain,
side-effect override and the resolved dependencies. On a hit, `ModuleTask`
restores all of it from disk and never calls a `resolveId`, `load` or
`transform` hook, nor the native resolver, for that module. Parsing and
scanning re-run natively on the cached code; everything downstream (link,
codegen) is unchanged.

## Components

| Piece               | Location                                                                                        |
| ------------------- | ----------------------------------------------------------------------------------------------- |
| Option types        | `crates/rolldown_common/src/inner_bundler_options/types/experimental_options.rs`                |
| Store + entry codec | `crates/rolldown/src/utils/build_cache.rs`                                                      |
| Construction        | `ModuleLoader::new` (`crates/rolldown/src/module_loader/module_loader.rs`)                      |
| Read/write seam     | `ModuleTask::run_inner` (`crates/rolldown/src/module_loader/module_task.rs`)                    |
| Binding             | `BindingBuildCacheOptions` (`crates/rolldown_binding/.../binding_experimental_options.rs`)      |
| JS options          | `packages/rolldown/src/options/input-options.ts`, `validator.ts`, `bindingify-input-options.ts` |

## Data flow

1. `ModuleLoader::new` builds one `BuildCache` per build when the option is
   enabled, unless the native magic-string sourcemap channel is active (those
   sourcemaps are generated outside the module task and could not be
   captured; see `create_sourcemap_channel` in `scan_stage.rs`).
   `BuildCache::new` also returns `None` when no plugin beyond rolldown's
   cheap inner plugins registers a `resolveId`/`load`/`transform` hook; with
   nothing to skip, the cache would be pure overhead.
2. `ModuleTask::run_inner` reads the module's raw on-disk content
   (`read_disk_source_for_cache_key`) before running any hook. Ids that are
   not absolute filesystem paths (virtual modules, data URLs, `rolldown:`)
   or cannot be read bypass the cache entirely.
3. The key is
   `xxh3(salt, stable_id, asserted_module_type, xxh3(disk_bytes))`, hex
   encoded, where
   `salt = xxh3(format_version, rolldown_version, options.key, platform, sorted moduleTypes, ordered plugin names)`.
   `stable_id` (cwd-relative, forward slashes) keeps keys portable across
   machines, which remote cache layers rely on.
4. Hit (`BuildCache::get` decoded the entry and every cached non-external
   dependency with an absolute path still exists on disk):
   - the module id is registered as a watch file, mirroring the
     read-from-disk path in `load_source`;
   - the stored sourcemap chain (including `Load` elements) replaces the
     empty chain, the side-effects override is applied if one was stored, and
     the cached code + module type feed straight into `create_ecma_view`;
   - after the native scan, the cached `ResolvedId`s are used instead of
     calling `resolve_dependencies`. They are positionally aligned with the
     scanned import records; a length mismatch falls back to fresh
     resolution.
5. Miss: the normal pipeline runs (`load_source` → `transform_source` →
   `create_ecma_view` → `resolve_dependencies`). If the source was a string
   and resolution emitted no warnings, the entry is stored: code, final
   module type, side-effects delta (only if the load/transform pipeline
   changed it), the full sourcemap chain (taken from
   `ecma_view.sourcemap_chain`, which `create_ecma_view` moves verbatim) and
   the resolved dependencies.

## Entry format

`<dir>/build-v1/<key[0..2]>/<key>`, written to a process-unique temp file and
renamed into place. Layout:

```text
b"RDBC"  format_version:u8  meta_len:u64le  meta_json  raw code bytes
```

`meta_json` holds `moduleType`, the optional `sideEffects` override (0/1/2),
the serialized chain elements (`Load`/`Transform` maps as sourcemap JSON,
`Omitted`/`Null` with plugin index and payload) and `deps`, one object per
import record: the resolved id, `ModuleDefFormat`, external kind,
side-effect data and the dependency's `package.json` fields (name, version,
type, `sideEffects`, realpath). Absolute paths inside entries are stored
cwd-relative with an `abs` marker and re-absolutized against the reader's
cwd. Any decode failure, size mismatch or unknown version is treated as a
miss and the entry is rewritten.

## Invariants

- Cache failures never fail the build: reads degrade to misses, writes log at
  `debug` level and are dropped.
- `PluginIdx` values inside cached chains are only meaningful because the
  ordered plugin name list is part of the salt; changing the plugin list
  invalidates every entry.
- Cached `resolved_deps` line up with the import records scanned from the
  cached code because scanning is deterministic; the length check is the
  corruption guard.
- The runtime module task and emitted-entry virtual modules never touch the
  cache (their ids are not absolute paths).
