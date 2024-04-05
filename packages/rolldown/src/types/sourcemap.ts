export interface SourceMapInputObject {
  file?: string
  mappings: string
  names?: string[]
  sources?: (string | null)[]
  sourcesContent?: (string | null)[]
  version: number
}

export type SourceMapInput = SourceMapInputObject | string | null
