import type { SourceMapInput } from '../rollup-types'

export function transformSourcemap(value?: SourceMapInput): string | undefined {
  if (typeof value === 'object') {
    return JSON.stringify(value)
  }
  return value
}

export function isEmptySourcemapFiled(array: undefined | string[]): boolean {
  if (!array) {
    return true
  }
  if (array.length === 0 || array[0] === '') {
    return true
  }
  return false
}
