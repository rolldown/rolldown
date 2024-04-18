import {
  BindingHookResolveIdExtraOptions,
  BindingPluginContext,
  RenderedChunk,
  BindingOutputs,
  BindingOutputOptions,
  BindingInputOptions,
} from '../binding'
import { RolldownNormalizedInputOptions } from '../options/input-options'
import { AnyFn, AnyObj, NullValue, MaybePromise } from '../types/utils'
import { SourceMapInput } from '../types/sourcemap'
import { pathToFileURL } from 'node:url'
import { NormalizedOutputOptions } from '../options/output-options'
import { ModuleInfo } from '../types/module-info'

// Use a type alias here, we might wrap `BindingPluginContext` in the future
type PluginContext = BindingPluginContext

type FormalHook<Handler extends AnyFn, HookOptions extends AnyObj = AnyObj> = {
  handler: Handler
} & HookOptions

export type Hook<Handler extends AnyFn, HookOptions extends AnyObj = AnyObj> =
  | FormalHook<Handler, HookOptions>
  | Handler

export interface Plugin {
  name?: string

  // --- Build hooks ---

  buildStart?: Hook<
    (
      this: PluginContext,
      options: RolldownNormalizedInputOptions,
    ) => MaybePromise<NullValue>
  >

  resolveId?: Hook<
    (
      this: null,
      source: string,
      importer: string | undefined,
      extraOptions: BindingHookResolveIdExtraOptions,
    ) => MaybePromise<
      | string
      | NullValue
      | false
      | {
          id: string
          external?: boolean
        }
    >
  >

  load?: Hook<
    (
      this: null,
      id: string,
    ) => MaybePromise<
      NullValue | string | { code: string; map?: SourceMapInput }
    >
  >

  transform?: Hook<
    (
      this: null,
      code: string,
      id: string,
    ) => MaybePromise<
      | NullValue
      | string
      | {
          code: string
          map?: string | null | SourceMapInput
        }
    >
  >

  moduleParsed?: Hook<
    (this: PluginContext, moduleInfo: ModuleInfo) => MaybePromise<NullValue>
  >

  buildEnd?: Hook<(this: null, err?: Error) => MaybePromise<NullValue>>

  // --- Generate hooks ---

  renderStart?: Hook<
    (
      outputOptions: BindingOutputOptions,
      inputOptions: RolldownNormalizedInputOptions,
    ) => MaybePromise<NullValue>
  >

  renderChunk?: Hook<
    (
      this: null,
      code: string,
      chunk: RenderedChunk,
      outputOptions: NormalizedOutputOptions,
    ) => MaybePromise<
      | NullValue
      | string
      | {
          code: string
          map?: string | null | SourceMapInput
        }
    >
  >

  renderError?: Hook<(this: null, error: Error) => MaybePromise<NullValue>>

  generateBundle?: Hook<
    (bundle: BindingOutputs, isWrite: boolean) => MaybePromise<NullValue>
  >
  writeBundle?: Hook<(bundle: BindingOutputs) => MaybePromise<NullValue>>
}

export type ParallelPlugin = {
  /** @internal */
  _parallel: {
    fileUrl: string
    options: unknown
  }
}

export type DefineParallelPluginResult<Options> = (
  options: Options,
) => ParallelPlugin

export function defineParallelPlugin<Options>(
  pluginPath: string,
): DefineParallelPluginResult<Options> {
  return (options) => {
    return { _parallel: { fileUrl: pathToFileURL(pluginPath).href, options } }
  }
}
