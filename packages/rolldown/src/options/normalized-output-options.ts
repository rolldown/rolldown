import { unsupported } from '../utils/misc'
import type { BindingNormalizedOptions } from '../binding'
import { ChunkFileNamesFunction } from './output-options'

export type InternalModuleFormat = 'es' | 'cjs' | 'iife' | 'umd' | 'app'

export interface NormalizedOutputOptions {
  entryFileNames: string | ChunkFileNamesFunction
  format: InternalModuleFormat
}

// TODO: I guess we make these getters enumerable so it act more like a plain object
export class NormalizedOutputOptionsImpl implements NormalizedOutputOptions {
  inner: BindingNormalizedOptions
  constructor(inner: BindingNormalizedOptions) {
    this.inner = inner
  }

  get entryFileNames() {
    return (
      this.inner.entryFilenames ||
      unsupported(
        'You should not take `NormalizedOutputOptions#entryFileNames` and call it directly',
      )
    )
  }

  get format() {
    return this.inner.format
  }
}
