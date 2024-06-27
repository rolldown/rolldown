import type {
  BindingHookResolveIdExtraOptions,
  RenderedChunk,
} from '../binding'
import type { NormalizedInputOptions } from '../options/normalized-input-options'
import type {
  AnyFn,
  AnyObj,
  NullValue,
  MaybePromise,
  PartialNull,
} from '../types/utils'
import type { SourceMapInput } from '../types/sourcemap'
import { pathToFileURL } from 'node:url'
import type { ModuleInfo } from '../types/module-info'
import type { OutputBundle } from '../types/output-bundle'
import type { PluginContext } from './plugin-context'
import type { TransformPluginContext } from './transfrom-plugin-context'
import type { NormalizedOutputOptions } from '../options/normalized-output-options'
import type { LogLevel } from '../log/logging'
import type { RollupLog } from '../rollup'
import type { MinimalPluginContext } from '../log/logger'
import { InputOptions, OutputOptions } from '..'
import { BuiltinPlugin } from './bindingify-builtin-plugin'

type FormalHook<Handler extends AnyFn, HookOptions extends AnyObj = AnyObj> = {
  handler: Handler
} & HookOptions

export type ObjectHook<
  Handler extends AnyFn,
  HookOptions extends AnyObj = AnyObj,
> = FormalHook<Handler, HookOptions> | Handler

export type ModuleSideEffects = boolean | 'no-treeshake' | null

export type ImportKind = BindingHookResolveIdExtraOptions['kind']

export interface CustomPluginOptions {
  [plugin: string]: any
}

export interface ModuleOptions {
  moduleSideEffects: ModuleSideEffects
}

export interface ResolvedId extends ModuleOptions {
  external: boolean
  id: string
}

export interface PartialResolvedId extends Partial<PartialNull<ModuleOptions>> {
  external?: boolean
  id: string
}

export interface SourceDescription extends Partial<PartialNull<ModuleOptions>> {
  code: string
  map?: SourceMapInput
}

export type ResolveIdResult = string | NullValue | false | PartialResolvedId

export type LoadResult = NullValue | string | SourceDescription

export type TransformResult = NullValue | string | SourceDescription

export interface Plugin {
  name?: string

  onLog?: ObjectHook<
    (
      this: MinimalPluginContext,
      level: LogLevel,
      log: RollupLog,
    ) => NullValue | boolean
  >

  options?: ObjectHook<
    (
      this: MinimalPluginContext,
      options: InputOptions,
    ) => MaybePromise<NullValue | InputOptions>
  >

  // TODO find a way to make `this: PluginContext` work.
  outputOptions?: ObjectHook<
    (
      this: null,
      options: OutputOptions,
    ) => MaybePromise<NullValue | OutputOptions>
  >

  // --- Build hooks ---

  buildStart?: ObjectHook<
    (
      this: PluginContext,
      options: NormalizedInputOptions,
    ) => MaybePromise<NullValue>
  >

  resolveId?: ObjectHook<
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
  resolveDynamicImport?: ObjectHook<
    (
      this: PluginContext,
      source: string,
      importer: string | undefined,
    ) => MaybePromise<ResolveIdResult>
  >

  load?: ObjectHook<
    (this: PluginContext, id: string) => MaybePromise<LoadResult>
  >

  transform?: ObjectHook<
    (
      this: TransformPluginContext,
      code: string,
      id: string,
    ) => MaybePromise<TransformResult>
  >

  moduleParsed?: ObjectHook<
    (this: PluginContext, moduleInfo: ModuleInfo) => MaybePromise<NullValue>
  >

  buildEnd?: ObjectHook<
    (this: PluginContext, err?: Error) => MaybePromise<NullValue>
  >

  // --- Generate hooks ---

  renderStart?: ObjectHook<
    (
      this: PluginContext,
      outputOptions: NormalizedOutputOptions,
      inputOptions: NormalizedInputOptions,
    ) => MaybePromise<NullValue>
  >

  renderChunk?: ObjectHook<
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
          map?: SourceMapInput
        }
    >
  >

  augmentChunkHash?: ObjectHook<
    (this: PluginContext, chunk: RenderedChunk) => MaybePromise<string | void>
  >

  renderError?: ObjectHook<
    (this: PluginContext, error: Error) => MaybePromise<NullValue>
  >

  generateBundle?: ObjectHook<
    (
      this: PluginContext,
      outputOptions: NormalizedOutputOptions,
      bundle: OutputBundle,
      isWrite: boolean,
    ) => MaybePromise<NullValue>
  >

  writeBundle?: ObjectHook<
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

export type RolldownPlugin = Plugin | ParallelPlugin | BuiltinPlugin

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

export type FunctionPluginHooks = Plugin

export type SyncPluginHooks =
  | 'augmentChunkHash'
  | 'onLog'
  | 'outputOptions'
  | 'renderDynamicImport'
  | 'resolveFileUrl'
  | 'resolveImportMeta'

export type AsyncPluginHooks = Exclude<
  keyof FunctionPluginHooks,
  SyncPluginHooks
>

export type FirstPluginHooks =
  | 'load'
  | 'renderDynamicImport'
  | 'resolveDynamicImport'
  | 'resolveFileUrl'
  | 'resolveId'
  | 'resolveImportMeta'
  | 'shouldTransformCachedModule'

export type SequentialPluginHooks =
  | 'augmentChunkHash'
  | 'generateBundle'
  | 'onLog'
  | 'options'
  | 'outputOptions'
  | 'renderChunk'
  | 'transform'

export type AddonHooks = 'banner' | 'footer' | 'intro' | 'outro'

export type ParallelPluginHooks = Exclude<
  keyof FunctionPluginHooks | AddonHooks,
  FirstPluginHooks | SequentialPluginHooks
>
