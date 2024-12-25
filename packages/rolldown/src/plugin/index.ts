import type {
  BindingHookResolveIdExtraArgs,
  BindingTransformHookExtraArgs,
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
import type { PluginContext } from './plugin-context'
import type { TransformPluginContext } from './transform-plugin-context'
import type { NormalizedOutputOptions } from '../options/normalized-output-options'
import type { LogLevel } from '../log/logging'
import type { RollupLog } from '../types/misc'
import type { MinimalPluginContext } from './minimal-plugin-context'
import type { InputOptions, OutputOptions } from '..'
import type { BuiltinPlugin } from '../builtin-plugin/constructors'
import type { ParallelPlugin } from './parallel-plugin'
import type { DefinedHookNames } from '../constants/plugin'
import type { DEFINED_HOOK_NAMES } from '../constants/plugin'
import type { SYMBOL_FOR_RESOLVE_CALLER_THAT_SKIP_SELF } from '../constants/plugin-context'
import type { HookFilter } from './hook-filter'
import { RenderedChunk } from '../types/rolldown-output'

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
  moduleType?: ModuleType
}

interface ResolveIdExtraOptions {
  custom?: CustomPluginOptions
  isEntry: boolean
  kind: 'import' | 'dynamic-import' | 'require-call'
}

export interface PrivateResolveIdExtraOptions extends ResolveIdExtraOptions {
  [SYMBOL_FOR_RESOLVE_CALLER_THAT_SKIP_SELF]?: symbol
}

export type ResolveIdResult = string | NullValue | false | PartialResolvedId

export type LoadResult = NullValue | string | SourceDescription

export type TransformResult = NullValue | string | Partial<SourceDescription>

export interface FunctionPluginHooks {
  [DEFINED_HOOK_NAMES.onLog]: (
    this: MinimalPluginContext,
    level: LogLevel,
    log: RollupLog,
  ) => NullValue | boolean

  [DEFINED_HOOK_NAMES.options]: (
    this: MinimalPluginContext,
    options: InputOptions,
  ) => NullValue | InputOptions

  // TODO find a way to make `this: PluginContext` work.
  [DEFINED_HOOK_NAMES.outputOptions]: (
    this: null,
    options: OutputOptions,
  ) => NullValue | OutputOptions

  // --- Build hooks ---

  [DEFINED_HOOK_NAMES.buildStart]: (
    this: PluginContext,
    options: NormalizedInputOptions,
  ) => void

  [DEFINED_HOOK_NAMES.resolveId]: (
    this: PluginContext,
    source: string,
    importer: string | undefined,
    extraOptions: ResolveIdExtraOptions,
  ) => ResolveIdResult

  /**
   * @deprecated
   * This hook is only for rollup plugin compatibility. Please use `resolveId` instead.
   */
  [DEFINED_HOOK_NAMES.resolveDynamicImport]: (
    this: PluginContext,
    source: string,
    importer: string | undefined,
  ) => ResolveIdResult

  [DEFINED_HOOK_NAMES.load]: (
    this: PluginContext,
    id: string,
  ) => MaybePromise<LoadResult>

  [DEFINED_HOOK_NAMES.transform]: (
    this: TransformPluginContext,
    code: string,
    id: string,
    meta: BindingTransformHookExtraArgs & { moduleType: ModuleType },
  ) => TransformResult

  [DEFINED_HOOK_NAMES.moduleParsed]: (
    this: PluginContext,
    moduleInfo: ModuleInfo,
  ) => void

  [DEFINED_HOOK_NAMES.buildEnd]: (this: PluginContext, err?: Error) => void

  // --- Generate hooks ---

  [DEFINED_HOOK_NAMES.renderStart]: (
    this: PluginContext,
    outputOptions: NormalizedOutputOptions,
    inputOptions: NormalizedInputOptions,
  ) => void

  [DEFINED_HOOK_NAMES.renderChunk]: (
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

  [DEFINED_HOOK_NAMES.augmentChunkHash]: (
    this: PluginContext,
    chunk: RenderedChunk,
  ) => string | void

  [DEFINED_HOOK_NAMES.renderError]: (this: PluginContext, error: Error) => void

  [DEFINED_HOOK_NAMES.generateBundle]: (
    this: PluginContext,
    outputOptions: NormalizedOutputOptions,
    bundle: OutputBundle,
    isWrite: boolean,
  ) => void

  [DEFINED_HOOK_NAMES.writeBundle]: (
    this: PluginContext,
    outputOptions: NormalizedOutputOptions,
    bundle: OutputBundle,
  ) => void

  [DEFINED_HOOK_NAMES.closeBundle]: (this: PluginContext) => void

  // --- watch hooks ---
  [DEFINED_HOOK_NAMES.watchChange]: (
    this: PluginContext,
    id: string,
    event: { event: ChangeEvent },
  ) => void

  [DEFINED_HOOK_NAMES.closeWatcher]: (this: PluginContext) => void
}

export type ChangeEvent = 'create' | 'update' | 'delete'

export type PluginOrder = 'pre' | 'post' | null

export type ObjectHookMeta = { order?: PluginOrder }

export type ObjectHook<T, O = {}> = T | ({ handler: T } & ObjectHookMeta & O)
export type SyncPluginHooks = DefinedHookNames[
  | 'augmentChunkHash'
  | 'onLog'
  | 'outputOptions']
// | 'renderDynamicImport'
// | 'resolveFileUrl'
// | 'resolveImportMeta'

export type AsyncPluginHooks = Exclude<
  keyof FunctionPluginHooks,
  SyncPluginHooks
>

export type FirstPluginHooks = DefinedHookNames[
  | 'load'
  // | 'renderDynamicImport'
  | 'resolveDynamicImport'
  // | 'resolveFileUrl'
  | 'resolveId']
// | 'resolveImportMeta'
// | 'shouldTransformCachedModule'

export type SequentialPluginHooks = DefinedHookNames[
  | 'augmentChunkHash'
  | 'generateBundle'
  | 'onLog'
  | 'options'
  | 'outputOptions'
  | 'renderChunk'
  | 'transform']

export type AddonHooks = DefinedHookNames[
  | 'banner'
  | 'footer'
  | 'intro'
  | 'outro']

export type OutputPluginHooks = DefinedHookNames[
  | 'augmentChunkHash'
  | 'generateBundle'
  | 'outputOptions'
  | 'renderChunk'
  // | 'renderDynamicImport'
  | 'renderError'
  | 'renderStart'
  // | 'resolveFileUrl'
  // | 'resolveImportMeta'
  | 'writeBundle']

export type ParallelPluginHooks = Exclude<
  keyof FunctionPluginHooks | AddonHooks,
  FirstPluginHooks | SequentialPluginHooks
>
export type HookFilterExtension<K extends keyof FunctionPluginHooks> =
  K extends 'transform'
    ? { filter?: HookFilter }
    : K extends 'load' | 'resolveId'
      ? { filter?: Pick<HookFilter, 'id'> }
      : {}

export type PluginHooks = {
  [K in keyof FunctionPluginHooks]: ObjectHook<
    K extends AsyncPluginHooks
      ? MakeAsync<FunctionPluginHooks[K]>
      : FunctionPluginHooks[K],
    HookFilterExtension<K>
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
  name: string
  // version?: string
}

export interface Plugin<A = any> extends OutputPlugin, Partial<PluginHooks> {
  // for inter-plugin communication
  api?: A
}

export type RolldownPlugin<A = any> = Plugin<A> | BuiltinPlugin | ParallelPlugin
export type RolldownPluginOption<A = any> = MaybePromise<
  NullValue<RolldownPlugin<A>> | false | RolldownPluginOption[]
>
export type RolldownOutputPlugin = OutputPlugin | BuiltinPlugin
export type RolldownOutputPluginOption = MaybePromise<
  NullValue<RolldownOutputPlugin> | false | RolldownOutputPluginOption[]
>
