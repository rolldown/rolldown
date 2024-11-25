import type { LogHandler } from '../rollup'
import { BindingNormalizedOptions } from '../binding'

export interface NormalizedInputOptions {
  shimMissingExports: boolean
  input: string[] | Record<string, string>
}

// TODO: I guess we make these getters enumerable so it act more like a plain object
export class NormalizedInputOptionsImpl implements NormalizedInputOptions {
  inner: BindingNormalizedOptions
  constructor(
    inner: BindingNormalizedOptions,
    public onLog: LogHandler,
  ) {
    this.inner = inner
  }

  get shimMissingExports() {
    return this.inner.shimMissingExports
  }

  get input() {
    return this.inner.input
  }
}
