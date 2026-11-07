# Module Sourcemap Chain — Implementation

## Summary

Each module keeps a `sourcemap_chain` of the maps that the `load` and `transform` hooks return. At render time, `collapse_module_sourcemap` (`crates/rolldown/src/utils/render_ecma_module.rs`) collapses the chain with the oxc codegen map into the module's map. The chunk then joins the module maps.

## Chain elements

`SourcemapChainElement` (`crates/rolldown_common/src/types/sourcemap_chain_element.rs`):

- `Load(map)`: the `load` hook map (`crates/rolldown/src/utils/load_source.rs`). It is always first.
- `Transform((plugin_idx, map))`: a `transform` hook map (`PluginDriver::transform` in `crates/rolldown_plugin/src/plugin_driver/build_hooks.rs`), or a native MagicString map (`create_sourcemap_channel` in `crates/rolldown/src/stages/scan_stage.rs`).
- `Omitted`: the hook changed the code and returned no map. It collapses as an empty map, so the module map has no mappings, and the build warns `SOURCEMAP_BROKEN`.
- `Null`: the hook changed the code and returned `map: null`. Positions pass through unchanged. If no real map is in the chain, its pre-transform code becomes the content.

## Collapse rule

`collapse_sourcemaps` (`crates/rolldown_sourcemap/src/lib.rs`) takes `sources` and `sourcesContent` from the first map only. It traces each token back through the earlier maps by line and column, and it ignores the source id of each later map. So only the first map's `sources` and `sourcesContent` ship, and a later map only moves positions.

## Normalization at the hook boundary

- Transform maps from JS: `prepareTransformHookSourcemap` (`packages/rolldown/src/utils/transform-sourcemap.ts`) parses a string map. It gives a map without `sources` one unnamed source, so that its mappings decode. It never writes to the plugin's object.
- Every map that a `transform` hook returns, from a JS or Rust plugin: `normalize_transform_sourcemap` (`crates/rolldown_plugin/src/utils/normalize_transform_sourcemap.rs`). A map with at most one source maps back to the transform input, so its source is the module: the id becomes its name, and the transform input fills a missing, `null`, or `''` content. Rolldown knows that code, so it replaces `''` too; Rollup ignores this content altogether. In a map with more sources, only an entry named with the module id gets that fill, and every other entry stays as returned, `''` and `null` included (#11236).
- Load maps from JS: `preProcessSourceMap` (`packages/rolldown/src/plugin/bindingify-build-hooks.ts`) resolves each named source against the module directory and `sourceRoot`, as Rollup does. It skips an unnamed source.

## Known gaps

- Relative `sources` and `sourceRoot` of a transform map are not resolved against the module directory, so the output stage resolves them against the process cwd (#10823).
- An unnamed source (`''` or `null`) that a map keeps becomes the cwd in the output map (for example `..`), because `crates/rolldown/src/utils/process_code_and_sourcemap.rs` makes each source path relative to the output file.
- A map with more than one source keeps its names only when it is the first map in the chain. In a later position, the collapse rule above traces its tokens as if they pointed into the previous code.
