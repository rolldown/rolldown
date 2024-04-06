import { SourceMapInput } from '../types/sourcemap'

export function transformSourcemap(value?: SourceMapInput): string | undefined {
  if (typeof value === 'object') {
    return JSON.stringify(value)
  }
  return value
}

export function isEmptySourcemapFiled(
  array: undefined | (string | null)[],
): boolean {
  if (!array) {
    return true
  }
  if (array.length === 0 || !array[0] /* null or '' */) {
    return true
  }
  return false
}
