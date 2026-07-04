# Persistent Transform Cache - Implementation

> Rationale, trade-offs and limitations live in [design.md](./design.md).

## Summary

`experimental.transformCache` persists the result of the plugin `transform`
pipeline per module. On a hit, `ModuleTask` restores the transformed code,
module type, transform sourcemap chain and side-effect override from disk and
never calls a `transform` hook. Everything downstream (parse, scan, link,
codegen) is unchanged.

## Components

| Piece               | Location                                                                                        |
| ------------------- | ----------------------------------------------------------------------------------------------- |
| Option types        | `crates/rolldown_common/src/inner_bundler_options/types/experimental_options.rs`                |
| Store + entry codec | `crates/rolldown/src/utils/transform_cache.rs`                                                  |
| Construction        | `ModuleLoader::new` (`crates/rolldown/src/module_loader/module_loader.rs`)                      |
| Read/write seam     | `ModuleTask::transform_source_with_cache` (`crates/rolldown/src/module_loader/module_task.rs`)  |
| Binding             | `BindingTransformCacheOptions` (`crates/rolldown_binding/.../binding_experimental_options.rs`)  |
| JS options          | `packages/rolldown/src/options/input-options.ts`, `validator.ts`, `bindingify-input-options.ts` |

## Data flow

1. `ModuleLoader::new` builds one `TransformCache` per build when the option
   is enabled, unless the native magic-string sourcemap channel is active
   (those sourcemaps are generated outside the module task and could not be
   captured; see `create_sourcemap_channel` in `scan_stage.rs`).
2. `ModuleTask::load_source` runs the `load` pipeline normally, then routes
   string sources through `transform_source_with_cache` instead of
   `transform_source`.
3. The key is
   `xxh3(salt, stable_id, module_type, xxh3(source))`, hex encoded, where
   `salt = xxh3(format_version, rolldown_version, options.key, ordered plugin names)`.
   `stable_id` (cwd-relative, forward slashes) keeps keys portable across
   machines, which remote cache layers rely on.
4. Hit: extend `sourcemap_chain` with the stored transform elements, restore
   `module_type`, apply the side-effects override if one was stored, return
   the cached code.
5. Miss: run `transform_source`, then store the code, the final module type,
   the side-effects delta (only if the pipeline changed it) and exactly the
   chain elements appended by the transform pipeline
   (`sourcemap_chain[len_before..]`).

## Entry format

`<dir>/transform-v1/<key[0..2]>/<key>`, written to a process-unique temp file
and renamed into place. Layout:

```text
b"RDTC"  format_version:u8  meta_len:u64le  meta_json  raw code bytes
```

`meta_json` holds `moduleType`, the optional `sideEffects` override (0/1/2)
and the serialized chain elements (`Transform` maps as sourcemap JSON,
`Omitted`/`Null` with plugin index and payload). Any decode failure, size
mismatch or unknown version is treated as a miss and the entry is rewritten.

## Invariants

- A `SourcemapChainElement::Load` must never be cached; `encode_entry` refuses
  the whole entry if the caller sliced the chain wrong.
- Cache failures never fail the build: reads degrade to misses, writes log at
  `debug` level and are dropped.
- `PluginIdx` values inside cached chains are only meaningful because the
  ordered plugin name list is part of the salt; changing the plugin list
  invalidates every entry.
