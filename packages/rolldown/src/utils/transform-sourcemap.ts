import type { SourceMapInput } from '../rollup-types'

export function transformSourcemap(value?: SourceMapInput): string | undefined {
  if (typeof value === 'object') {
    return JSON.stringify(value)
  }
  return value
}
