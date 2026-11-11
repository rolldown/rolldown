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
  let map = typeof rawMap === 'object' ? rawMap : (JSON.parse(rawMap) as ExistingRawSourceMap);
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
