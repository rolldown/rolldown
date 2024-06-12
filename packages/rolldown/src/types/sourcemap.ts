import { BindingSourcemap } from '../binding'

export interface SourceMapInputObject {
  file?: string
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
  return { inner: map }
}
