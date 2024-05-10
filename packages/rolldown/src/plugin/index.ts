import type {
  BindingHookResolveIdExtraOptions,
  RenderedChunk,
} from '../binding'
import type { NormalizedInputOptions } from '../options/normalized-input-options'
import { AnyFn, AnyObj, NullValue, MaybePromise } from '../types/utils'
import type { SourceMapInput } from '../types/sourcemap'
import { pathToFileURL } from 'node:url'
import type { NormalizedOutputOptions } from '../options/output-options'
import type { ModuleInfo } from '../types/module-info'
import type { OutputBundle } from '../types/output-bundle'
import { PluginContext } from './plugin-context'

type FormalHook<Handler extends AnyFn, HookOptions extends AnyObj = AnyObj> = {
  handler: Handler
} & HookOptions

export type Hook<Handler extends AnyFn, HookOptions extends AnyObj = AnyObj> =
  | FormalHook<Handler, HookOptions>
  | Handler

export type ResolveIdResult =
  | string
  | NullValue
  | false
  | {
      id: string
      external?: boolean
    }

export interface Plugin {
  name?: string

  // --- Build hooks ---

  buildStart?: Hook<
    (
      this: PluginContext,
      options: NormalizedInputOptions,
    ) => MaybePromise<NullValue>
  >

  resolveId?: Hook<
    (
      this: PluginContext,
      source: string,
      importer: string | undefined,
      extraOptions: BindingHookResolveIdExtraOptions,
    ) => MaybePromise<ResolveIdResult>
  >

  /**
   * @deprecated
   * This hook is only for rollup plugin compatibility. Please use `resolveId` instead.
   */
  resolveDynamicImport?: Hook<
    (
      this: PluginContext,
      source: string,
      importer: string | undefined,
    ) => MaybePromise<ResolveIdResult>
  >

  load?: Hook<
    (
      this: PluginContext,
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

  buildEnd?: Hook<(this: PluginContext, err?: Error) => MaybePromise<NullValue>>

  // --- Generate hooks ---

  renderStart?: Hook<
    (
      this: PluginContext,
      outputOptions: NormalizedOutputOptions,
      inputOptions: NormalizedInputOptions,
    ) => MaybePromise<NullValue>
  >

  renderChunk?: Hook<
    (
      this: PluginContext,
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

  renderError?: Hook<
    (this: PluginContext, error: Error) => MaybePromise<NullValue>
  >

  generateBundle?: Hook<
    (
      this: PluginContext,
      outputOptions: NormalizedOutputOptions,
      bundle: OutputBundle,
      isWrite: boolean,
    ) => MaybePromise<NullValue>
  >

  writeBundle?: Hook<
    (
      this: PluginContext,
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
