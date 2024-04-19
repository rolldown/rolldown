export * from './async-flatten'
export * from './transform-to-rollup-output'
export * from './normalize-plugin-option'
export * from './ensure-array'
export * from './create-bundler'
export * from './transform-sourcemap'
export * from './transform-module-info'

export function arraify<T>(value: T | T[]): T[] {
  return Array.isArray(value) ? value : [value]
}

export function unimplemented(info?: string): never {
  if (info) {
    throw new Error(`unimplemented: ${info}`)
  }
  throw new Error('unimplemented')
}

export function unreachable(info?: string): never {
  if (info) {
    throw new Error(`unreachable: ${info}`)
  }
  throw new Error('unreachable')
}

export function unsupported(info: string): never {
  throw new Error(`Rolldown unsupported api: ${info}`)
}

export function noop(..._args: any[]) {}
