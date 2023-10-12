export * from './async-flatten'
export * from './transformToRollupOutput'
export * from './normalizePluginOption'

export function arraify<T>(value: T | T[]): T[] {
  return Array.isArray(value) ? value : [value]
}

export function unimplemented(info?: string): never {
  if (info) {
    throw new Error(`unimplemented: ${info}`)
  }
  throw new Error('unimplemented')
}

export function noop(..._args: any[]) {}
