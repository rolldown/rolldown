import type { SourceMap } from '@rolldown/node-binding'
import type { SourceMapInput } from '../rollup-types'
import type { ExistingRawSourceMap } from '../rollup'

export function transformSourcemap(
  value?: SourceMapInput,
): SourceMap | undefined {
  if (!value) return undefined

  if (typeof value === 'string') {
    return parseSourceMap(value)
  }

  if (typeof value === 'object') {
    const {
      mappings,
      names = [],
      sourceRoot = '',
      sources = [],
      sourcesContent = [],
    } = value as ExistingRawSourceMap

    return {
      // TODO file, version, x_google_ignoreList
      mappings,
      names,
      sourceRoot,
      sources,
      sourcesContent: sourcesContent.filter(
        (v): v is string => typeof v === 'string',
      ),
    }
  }

  return undefined
}

function parseSourceMap(value: string): SourceMap | undefined {
  try {
    return JSON.parse(value) as SourceMap
  } catch {}
  return undefined
}
