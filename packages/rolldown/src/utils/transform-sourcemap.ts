import type { ExistingRawSourceMap, SourceMapInput } from '../types/sourcemap';

/**
 * Copies the fields that Rolldown reads from a hook's sourcemap object into a new plain
 * object. The glue then writes `sources` on the copy, so it does not change the plugin's
 * object. A spread does not work for the map object from the native MagicString's
 * `generateMap()`: its fields are getters on the prototype, and a write to them throws.
 */
export function copySourcemapObject(
  map: ExistingRawSourceMap,
): ExistingRawSourceMap & { debugId?: string | undefined } {
  return {
    file: map.file,
    mappings: map.mappings,
    names: map.names,
    sources: map.sources,
    sourcesContent: map.sourcesContent,
    sourceRoot: map.sourceRoot,
    version: map.version,
    ignoreList: map.ignoreList,
    x_google_ignoreList: map.x_google_ignoreList,
    // Not in `ExistingRawSourceMap`, but `bindingifySourcemap` passes it on when it is set.
    debugId: (map as { debugId?: string | undefined }).debugId,
  };
}

/**
 * Parses a `transform` hook sourcemap for the binding, without changing the plugin's object.
 *
 * The Rust side names the module's own source and fills its content, see
 * `crates/rolldown_plugin/src/utils/normalize_transform_sourcemap.rs`. A map without `sources`
 * gets one unnamed source here, so that mappings that refer to source 0 decode in the binding.
 */
export function prepareTransformHookSourcemap(
  rawMap?: SourceMapInput,
): ExistingRawSourceMap | undefined {
  if (!rawMap) {
    return;
  }
  const map = typeof rawMap === 'object' ? rawMap : (JSON.parse(rawMap) as ExistingRawSourceMap);
  return map.sources?.length ? map : { ...copySourcemapObject(map), sources: [null] };
}
