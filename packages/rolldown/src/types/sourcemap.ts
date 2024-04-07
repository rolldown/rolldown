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
