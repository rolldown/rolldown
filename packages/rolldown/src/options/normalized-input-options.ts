import type { LogHandler } from '../rollup'
import { BindingNormalizedOptions } from '../binding'
import { InputOptions } from '..'

export interface NormalizedInputOptions {
  input: string[] | Record<string, string>
  cwd: string | undefined
  platform: InputOptions['platform']
  shimMissingExports: boolean
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

  get cwd() {
    return this.inner.cwd ?? undefined
  }

  get platform() {
    return this.inner.platform
  }
}
