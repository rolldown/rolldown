import { BindingSourcemap } from '../binding'

export interface SourceMapInputObject {
  file?: string | null
  mappings: string
  names?: string[]
  sources?: (string | null)[]
  sourcesContent?: (string | null)[]
  sourceRoot?: string
  version: number
}

export type SourceMapInput = SourceMapInputObject | string | null

export function bidingSourcemap(
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
