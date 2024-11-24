import type { LogHandler } from '../rollup'
import { BindingNormalizedInputOptions } from '../binding'

// TODO: I guess we make these getters enumerable so it act more like a plain object
export class NormalizedInputOptions {
  inner: BindingNormalizedInputOptions
  constructor(
    inner: BindingNormalizedInputOptions,
    public onLog: LogHandler,
  ) {
    this.inner = inner
  }

  get shimMissingExports() {
    return this.inner.shimMissingExports
  }
}
