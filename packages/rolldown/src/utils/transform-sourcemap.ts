import type { ExistingRawSourceMap, SourceMapInput } from '../types/sourcemap';

export function isEmptySourcemapFiled(array: undefined | (string | null | undefined)[]): boolean {
  if (!array) {
    return true;
  }
  if (array.length === 0 || !array[0] /* null or '' */) {
    return true;
  }
  return false;
}

/**
 * Copies the fields that Rolldown reads from a hook's sourcemap object into a new plain
 * object. The glue then fills in `sources` and `sourcesContent` on the copy, so it does not
 * change the plugin's object. A spread does not work for the map object from the native
 * MagicString's `generateMap()`: its fields are getters on the prototype, and a write to them throws.
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

export function normalizeTransformHookSourcemap(
  id: string,
  originalCode: string,
  rawMap?: SourceMapInput,
): ExistingRawSourceMap | undefined {
  if (!rawMap) {
    return;
  }
  // If sourcemap hasn't `sourcesContent` and `sources`, using original code to fill it.
  // The Rust side already has the feature at `crates/rolldown_plugin/src/plugin_driver/build_hooks.rs#transform`.
  // but it could be failed at `rolldown_sourcemap::SourceMap::from_json`, because the map is invalid.
  let map =
    typeof rawMap === 'object'
      ? copySourcemapObject(rawMap)
      : (JSON.parse(rawMap) as ExistingRawSourceMap);
  const source = map.sources?.[0];
  if (
    source === undefined ||
    source === '' ||
    (map.sources?.length === 1 && source !== null && source !== id)
  ) {
    map.sources = [id];
  }
  if (map.sources?.[0] !== null && isEmptySourcemapFiled(map.sourcesContent)) {
    map.sourcesContent = [originalCode];
  }
  return map;
}
