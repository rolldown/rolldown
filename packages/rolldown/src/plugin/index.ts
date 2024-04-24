import type {
  BindingHookResolveIdExtraOptions,
  BindingPluginContext,
  RenderedChunk,
  BindingOutputs,
  BindingOutputOptions,
} from '../binding'
import type { RolldownNormalizedInputOptions } from '../options/input-options'
import { AnyFn, AnyObj, NullValue, MaybePromise } from '../types/utils'
import type { SourceMapInput } from '../types/sourcemap'
import { pathToFileURL } from 'node:url'
import type { NormalizedOutputOptions } from '../options/output-options'
import type { ModuleInfo } from '../types/module-info'
import type { OutputBundle } from '../types/output-bundle'

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
    (
      outputOptions: NormalizedOutputOptions,
      bundle: OutputBundle,
      isWrite: boolean,
    ) => MaybePromise<NullValue>
  >

  writeBundle?: Hook<
    (
      outputOptions: NormalizedOutputOptions,
      bundle: OutputBundle,
    ) => MaybePromise<NullValue>
  >
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
