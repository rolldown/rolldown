import type { SourceMap } from '@rolldown/node-binding'
import type { SourceMapInput } from '../rollup-types'
import type { ExistingRawSourceMap } from '../rollup'

export function transformSourcemap(value?: SourceMapInput): SourceMap | undefined {
  if (!value) return undefined

  if (typeof value === 'string') {
    try {
      return JSON.parse(value) as SourceMap
    } catch (error) {
      console.error('Error parsing source map:', error)
      return undefined
    }
  }

  if (typeof value === 'object') {
    const { mappings, names = [], sourceRoot = '', sources = [], sourcesContent = [] } = value as ExistingRawSourceMap

    return {
      // TODO file, version, x_google_ignoreList
      mappings,
      names,
      sourceRoot,
      sources,
      sourcesContent: sourcesContent.filter((v): v is string => typeof v === 'string'),
    }
  }

  return undefined
}
