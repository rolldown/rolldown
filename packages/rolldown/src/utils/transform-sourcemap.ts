import type { ExistingRawSourceMap, SourceMapInput } from '../types/sourcemap';

export function isEmptySourcemapFiled(
  array: undefined | (string | null)[],
): boolean {
  if (!array) {
    return true;
  }
  if (array.length === 0 || !array[0] /* null or '' */) {
    return true;
  }
  return false;
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
  // The rust side already has the feature at `crates/rolldown_plugin/src/plugin_driver/build_hooks.rs#transform`.
  // but it could be failed at `rolldown_sourcemap::SourceMap::from_json`, because the map is invalid.
  let map = typeof rawMap === 'object'
    ? rawMap
    : (JSON.parse(rawMap) as ExistingRawSourceMap);
  if (isEmptySourcemapFiled(map.sourcesContent)) {
    map.sourcesContent = [originalCode];
  }
  if (
    isEmptySourcemapFiled(map.sources) ||
    (map.sources && map.sources.length === 1 && map.sources[0] !== id) // the transform sourcemaps maybe contain multiple sources
  ) {
    map.sources = [id];
  }
  return map;
}
