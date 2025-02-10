import type { LogHandler } from '../types/misc'
import { BindingNormalizedOptions } from '../binding'
import { InputOptions } from '..'

export interface NormalizedInputOptions {
  input: string[] | Record<string, string>
  cwd: string | undefined
  platform: InputOptions['platform']
  shimMissingExports: boolean
  preserveEntrySignatures: InputOptions['preserveEntrySignatures']
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

  get shimMissingExports(): boolean {
    return this.inner.shimMissingExports
  }

  get input(): string[] | Record<string, string> {
    return this.inner.input
  }

  get cwd(): string | undefined {
    return this.inner.cwd ?? undefined
  }

  get platform(): 'browser' | 'node' | 'neutral' {
    return this.inner.platform
  }

  get preserveEntrySignatures():
    | 'strict'
    | 'allow-extension'
    | 'exports-only'
    | false {
    return this.inner.preserveEntrySignatures
  }
}

//&要添加PreserveEntrySignutarue这个属性
//& package/rolldown, rolldown/binidng ,rolldown/common, rolldown
//& package/rolldown中input-options添加preserveEntrySignatures属性
// normalized-input-options暂时没加
//^ rolldown_binding中binding_input_options,添加了preserve_entry_signatures
//normalize_binding_options根据bindingInputOptions生成BundlerOptions
//bundlerOptions则是放入nativeBundler
//我的想法是新增的这个属性也要加入到BundlerOptions，这样才能参与chunk的分割
//现在在的BundlerOptions的input属性有
// input，cwd,external,platform,shim_missing_exports
