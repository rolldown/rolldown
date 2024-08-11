import type {
  BindingHookResolveIdExtraArgs,
  BindingTransformHookExtraArgs,
  RenderedChunk,
} from '../binding'
import type { NormalizedInputOptions } from '../options/normalized-input-options'
import type {
  NullValue,
  MaybePromise,
  PartialNull,
  MakeAsync,
} from '../types/utils'
import type { SourceMapInput } from '../types/sourcemap'
import type { ModuleInfo } from '../types/module-info'
import type { OutputBundle } from '../types/output-bundle'
import { PluginContext } from './plugin-context'
import type { TransformPluginContext } from './transfrom-plugin-context'
import type { NormalizedOutputOptions } from '../options/normalized-output-options'
import type { LogLevel } from '../log/logging'
import type { RollupLog } from '../rollup'
import type { MinimalPluginContext } from '../log/logger'
import { InputOptions, OutputOptions } from '..'
import { BuiltinPlugin } from './builtin-plugin'
import { ParallelPlugin } from './parallel-plugin'

export type ModuleSideEffects = boolean | 'no-treeshake' | null

// ref: https://github.com/microsoft/TypeScript/issues/33471#issuecomment-1376364329
export type ModuleType =
  | 'js'
  | 'jsx'
  | 'ts'
  | 'tsx'
  | 'json'
  | 'text'
  | 'base64'
  | 'dataurl'
  | 'binary'
  | 'empty'
  | (string & {})

export type ImportKind = BindingHookResolveIdExtraArgs['kind']

export interface CustomPluginOptions {
  [plugin: string]: any
}

export interface ModuleOptions {
  moduleSideEffects: ModuleSideEffects
  meta: CustomPluginOptions
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

interface ResolveIdExtraOptions {
  custom?: CustomPluginOptions
  isEntry: boolean
  kind: 'import' | 'dynamic-import' | 'require-call'
}

export type ResolveIdResult = string | NullValue | false | PartialResolvedId

export type LoadResult = NullValue | string | SourceDescription

export type TransformResult =
  | NullValue
  | string
  | (Partial<SourceDescription> & { moduleType?: ModuleType })

export interface FunctionPluginHooks {
  onLog: (
    this: MinimalPluginContext,
    level: LogLevel,
    log: RollupLog,
  ) => NullValue | boolean

  options: (
    this: MinimalPluginContext,
    options: InputOptions,
  ) => NullValue | InputOptions

  // TODO find a way to make `this: PluginContext` work.
  outputOptions: (
    this: null,
    options: OutputOptions,
  ) => NullValue | OutputOptions

  // --- Build hooks ---

  buildStart: (this: PluginContext, options: NormalizedInputOptions) => void

  resolveId: (
    this: PluginContext,
    source: string,
    importer: string | undefined,
    extraOptions: ResolveIdExtraOptions,
  ) => ResolveIdResult

  /**
   * @deprecated
   * This hook is only for rollup plugin compatibility. Please use `resolveId` instead.
   */
  resolveDynamicImport: (
    this: PluginContext,
    source: string,
    importer: string | undefined,
  ) => ResolveIdResult

  load: (this: PluginContext, id: string) => MaybePromise<LoadResult>

  transform: (
    this: TransformPluginContext,
    code: string,
    id: string,
    meta: BindingTransformHookExtraArgs & { moduleType: ModuleType },
  ) => TransformResult

  moduleParsed: (this: PluginContext, moduleInfo: ModuleInfo) => void

  buildEnd: (this: PluginContext, err?: Error) => void

  // --- Generate hooks ---

  renderStart: (
    this: PluginContext,
    outputOptions: NormalizedOutputOptions,
    inputOptions: NormalizedInputOptions,
  ) => void

  renderChunk: (
    this: PluginContext,
    code: string,
    chunk: RenderedChunk,
    outputOptions: NormalizedOutputOptions,
  ) =>
    | NullValue
    | string
    | {
        code: string
        map?: SourceMapInput
      }

  augmentChunkHash: (this: PluginContext, chunk: RenderedChunk) => string | void

  renderError: (this: PluginContext, error: Error) => void

  generateBundle: (
    this: PluginContext,
    outputOptions: NormalizedOutputOptions,
    bundle: OutputBundle,
    isWrite: boolean,
  ) => void

  writeBundle: (
    this: PluginContext,
    outputOptions: NormalizedOutputOptions,
    bundle: OutputBundle,
  ) => void
}

export type PluginOrder = 'pre' | 'post' | null

export type ObjectHookMeta<O = {}> = { order?: PluginOrder } & O

export type ObjectHook<T, O = {}> = T | ({ handler: T } & ObjectHookMeta<O>)

export type SyncPluginHooks = 'augmentChunkHash' | 'onLog' | 'outputOptions'
// | 'renderDynamicImport'
// | 'resolveFileUrl'
// | 'resolveImportMeta'

export type AsyncPluginHooks = Exclude<
  keyof FunctionPluginHooks,
  SyncPluginHooks
>

export type FirstPluginHooks =
  | 'load'
  | 'renderDynamicImport'
  | 'resolveDynamicImport'
  // | 'resolveFileUrl'
  | 'resolveId'
// | 'resolveImportMeta'
// | 'shouldTransformCachedModule'

export type SequentialPluginHooks =
  | 'augmentChunkHash'
  | 'generateBundle'
  | 'onLog'
  | 'options'
  | 'outputOptions'
  | 'renderChunk'
  | 'transform'

export type AddonHooks = 'banner' | 'footer' | 'intro' | 'outro'

export type OutputPluginHooks =
  | 'augmentChunkHash'
  | 'generateBundle'
  | 'outputOptions'
  | 'renderChunk'
  // | 'renderDynamicImport'
  | 'renderError'
  | 'renderStart'
  // | 'resolveFileUrl'
  // | 'resolveImportMeta'
  | 'writeBundle'

export type ParallelPluginHooks = Exclude<
  keyof FunctionPluginHooks | AddonHooks,
  FirstPluginHooks | SequentialPluginHooks
>

export type PluginHooks = {
  [K in keyof FunctionPluginHooks]: ObjectHook<
    K extends AsyncPluginHooks
      ? MakeAsync<FunctionPluginHooks[K]>
      : FunctionPluginHooks[K]
    // eslint-disable-next-line @typescript-eslint/ban-types
    // TODO
    // K extends ParallelPluginHooks ? { sequential?: boolean } : {}
  >
}

export type AddonHookFunction = (
  this: PluginContext,
  chunk: RenderedChunk,
) => string | Promise<string>

export type AddonHook = string | AddonHookFunction

export interface OutputPlugin
  extends Partial<{ [K in OutputPluginHooks]: PluginHooks[K] }>,
    Partial<{ [K in AddonHooks]: ObjectHook<AddonHook> }> {
  // cacheKey?: string
  name?: string
  // version?: string
}

export interface Plugin<A = any> extends OutputPlugin, Partial<PluginHooks> {
  // for inter-plugin communication
  api?: A
}

export type RolldownPlugin<A = any> = Plugin<A> | ParallelPlugin | BuiltinPlugin
