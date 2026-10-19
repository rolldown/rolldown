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

/** Whether `mappings` has at least one segment, i.e. it is not only `;` and `,` separators. */
function hasMappings(mappings: string | undefined): boolean {
  return !!mappings && /[^;,]/.test(mappings);
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
  // A map without mappings (e.g. `{ mappings: '' }`) says the output has no mapping back to
  // the original code, so it must not be filled with the original id and code.
  // See https://rollupjs.org/plugin-development/#source-code-transformations
  if (!hasMappings(map.mappings)) {
    return map;
  }
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
