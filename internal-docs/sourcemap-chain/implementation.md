# Module Sourcemap Chain — Implementation

## Summary

Each module keeps a `sourcemap_chain` of the maps that the `load` and `transform` hooks return. At render time, `collapse_module_sourcemap` (`crates/rolldown/src/utils/render_ecma_module.rs`) collapses the chain with the oxc codegen map into the module's map. The chunk then joins the module maps.

## Chain elements

`SourcemapChainElement` (`crates/rolldown_common/src/types/sourcemap_chain_element.rs`):

- `Load(map)`: the `load` hook map (`crates/rolldown/src/utils/load_source.rs`). It is always first.
- `Transform((plugin_idx, map))`: a `transform` hook map (`PluginDriver::transform` in `crates/rolldown_plugin/src/plugin_driver/build_hooks.rs`).
- `MagicString((plugin_idx, pending))`: the map of a native MagicString that a `transform` hook returned without a `map`, when `experimental.nativeMagicString` and source maps are on. `PluginDriver::transform` takes it from the transform context right after the hook and adds it in plugin order. Thus a context that a plugin keeps does not keep the map alive. `PendingSourcemap` generates the map on first use: in `this.getCombinedSourcemap()` of a later hook, or else on the sourcemap worker thread (`create_sourcemap_channel` in `crates/rolldown/src/stages/scan_stage.rs`) after the module's last `transform` hook. The scan stage joins the worker, so every map exists before render.
- `Omitted`: the hook returned code without a map, even if the code did not change. It collapses as an empty map, so the module map has no mappings, and the build warns `SOURCEMAP_BROKEN`.
- `Null`: the hook changed the code and returned `map: null`. Positions pass through unchanged. If no real map is in the chain, its pre-transform code becomes the content.

## Collapse rule

`collapse_sourcemaps_traced` (`crates/rolldown_sourcemap/src/lib.rs`) takes `sources` and `sourcesContent` from the first map only. It traces each token back through the earlier maps by line and column, and it ignores the source id of each later map. So only the first map's `sources` and `sourcesContent` ship. Names can come from a later map too: each token takes its name from the earliest map that has a name for it, so a later map's name stays when the earlier maps have none. As Rollup's `traceMappings()` does, it keeps only the sources and names that a traced token references.

Before the collapse, `resolve_sources_against_module` (`crates/rolldown_sourcemap/src/resolve_sources.rs`) resolves the relative `sources` of the first map against the module directory and `sourceRoot`. The chain itself does not change, because `this.getCombinedSourcemap()` reads it.

A saved transform context reads the chain through a weak reference. The driver takes the chain when a transform hook returns a result other than `null`, and after the module's last transform hook. Calls to `getCombinedSourcemap()` through a context whose reference has expired throw an error. The JS object of a transform context holds the context itself weakly as well. The driver keeps each context until the chain changes, and calls after that throw the same error.

## Normalization at the hook boundary

- Transform maps from JS: `prepareTransformHookSourcemap` (`packages/rolldown/src/utils/transform-sourcemap.ts`) parses a string map. It gives a map without `sources` one unnamed source, so that its mappings decode. It never writes to the plugin's object.
- `normalize_transform_sourcemap` (`crates/rolldown_plugin/src/utils/normalize_transform_sourcemap.rs`) handles maps from JS and Rust `transform` hooks. In a map with at most one source, Rolldown names the source with the module id. It fills missing, `null`, or `''` content with the transform input. In a map with more sources, Rolldown fills content only for entries that name the module (#11236). `source_names_module` (`crates/rolldown_sourcemap/src/resolve_sources.rs`) checks the module id. It also compares absolute paths after normalization, and relative paths after resolution by `resolve_sources_against_module`, with the normalized module id. A `sourceRoot` that contains `://` or starts with `data:` blocks relative path matching.
- `preProcessSourceMap` (`packages/rolldown/src/plugin/bindingify-build-hooks.ts`) handles maps from JS `load` hooks. It skips path resolution if `sources` is absent or empty, or if its first entry is `''`. Otherwise, it uses `path.resolve` to resolve every source except `null` or `undefined` against the module directory and `sourceRoot`. This makes a later `''` entry resolve to a directory. It updates the copy from `copySourcemapObject`.

## Known gaps

- An unnamed `''` source or a URL source that a map keeps becomes a path under the cwd in the output map (for example `..` or `../https:/example.test/a.ts`), because `crates/rolldown/src/utils/process_code_and_sourcemap.rs` makes each source path that is not `null` relative to the output file. A `null` source stays `null`. A relative source under a URL `sourceRoot` also becomes a path under the cwd, because the output map drops the `sourceRoot`.
- A map with more than one source keeps its own `sources` only when it is the first map in the chain. In a later position, the collapse rule above traces its tokens as if they pointed into the previous code.
