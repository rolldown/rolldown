import type { BindingSourcemap } from '../binding';
import type { SourceMap } from './rolldown-output';

export interface ExistingRawSourceMap {
  file?: string | null;
  mappings: string;
  names?: string[];
  sources?: (string | null)[];
  sourcesContent?: (string | null)[];
  sourceRoot?: string;
  version?: number; // make it optional to compat { mappings: '' }
  x_google_ignoreList?: number[];
}

export type SourceMapInput = ExistingRawSourceMap | string | null;

export function bindingifySourcemap(
  map?: SourceMapInput | SourceMap,
): undefined | BindingSourcemap {
  if (map == null) return;
  return {
    inner: typeof map === 'string'
      ? map
      : {
        file: map.file ?? undefined,
        mappings: map.mappings,
        // according to the spec, `sourceRoot: null` is not valid,
        // but some tools returns a sourcemap with it.
        // in that case, napi-rs outputs an error which is difficult
        // to understand by users ("Value is non of these types `String`, `BindingJsonSourcemap`").
        // we convert it to undefined to skip that error.
        // note that if `sourceRoot: null` is included in a string sourcemap,
        // it will be converted to None by serde-json.
        sourceRoot: 'sourceRoot' in map
          ? (map.sourceRoot ?? undefined)
          : undefined,
        sources: map.sources?.map((s) => s ?? undefined),
        sourcesContent: map.sourcesContent?.map((s) => s ?? undefined),
        names: map.names,
        x_google_ignoreList: map.x_google_ignoreList,
        debugId: 'debugId' in map ? map.debugId : undefined,
      },
  };
}
