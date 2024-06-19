import { BindingSourcemap } from '../binding'

export interface ExistingRawSourceMap {
  file?: string | null
  mappings: string
  names?: string[]
  sources?: (string | null)[]
  sourcesContent?: (string | null)[]
  sourceRoot?: string
  version?: number // make it optional to compat { mappings: '' }
  x_google_ignoreList?: number[]
}

export type SourceMapInput = ExistingRawSourceMap | string | null

export function bindingifySourcemap(
  map?: SourceMapInput,
): undefined | BindingSourcemap {
  if (map == null) return
  return {
    inner:
      typeof map === 'string'
        ? map
        : {
            file: map.file ?? undefined,
            mappings: map.mappings,
            sourceRoot: map.sourceRoot,
            sources: map.sources?.map((s) => s ?? undefined),
            sourcesContent: map.sourcesContent?.map((s) => s ?? undefined),
            names: map.names,
          },
  }
}
