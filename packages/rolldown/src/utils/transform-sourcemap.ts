import type { ExistingRawSourceMap, SourceMapInput } from '../types/sourcemap';

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
  return map.sources?.length ? map : { ...map, sources: [null] };
}
