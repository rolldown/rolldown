import type { InputOptions, OutputOptions } from '..';
import type {
  BindingHookResolveIdExtraArgs,
  BindingMagicString,
  BindingTransformHookExtraArgs,
} from '../binding';
import type { BuiltinPlugin } from '../builtin-plugin/utils';
import type { DefinedHookNames } from '../constants/plugin';
import type { DEFINED_HOOK_NAMES } from '../constants/plugin';
import type { LogLevel, RollupLog } from '../log/logging';
import type { NormalizedInputOptions } from '../options/normalized-input-options';
import type { NormalizedOutputOptions } from '../options/normalized-output-options';
import type { ModuleInfo } from '../types/module-info';
import type { OutputBundle } from '../types/output-bundle';
import type { RenderedChunk } from '../types/rolldown-output';
import type { SourceMapInput } from '../types/sourcemap';
import type {
  MakeAsync,
  MaybePromise,
  NullValue,
  PartialNull,
} from '../types/utils';
import type {
  GeneralHookFilter,
  HookFilter,
  TUnionWithTopLevelFilterExpressionArray,
} from './hook-filter';
import type { MinimalPluginContext } from './minimal-plugin-context';
import type { ParallelPlugin } from './parallel-plugin';
import type { PluginContext } from './plugin-context';
import type { TransformPluginContext } from './transform-plugin-context';

type ModuleSideEffects = boolean | 'no-treeshake' | null;
export { withFilter } from './with-filter';

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
  | (string & {});

export type ImportKind = BindingHookResolveIdExtraArgs['kind'];

export interface CustomPluginOptions {
  [plugin: string]: any;
}

export interface ModuleOptions {
  moduleSideEffects: ModuleSideEffects;
  meta: CustomPluginOptions;
  // flag used to check if user directly modified the `ModuleInfo`
  // this is used to sync state between Rust and JavaScript
  invalidate?: boolean;
  packageJsonPath?: string;
}

export interface ResolvedId extends ModuleOptions {
  external: boolean | 'absolute';
  id: string;
}

export interface PartialResolvedId extends Partial<PartialNull<ModuleOptions>> {
  external?: boolean | 'absolute' | 'relative';
  id: string;
}

export interface SourceDescription extends Partial<PartialNull<ModuleOptions>> {
  code: string;
  map?: SourceMapInput;
  moduleType?: ModuleType;
}

export interface ResolveIdExtraOptions {
  custom?: CustomPluginOptions;
  isEntry: boolean;
  kind: BindingHookResolveIdExtraArgs['kind'];
}

export type ResolveIdResult = string | NullValue | false | PartialResolvedId;

export type LoadResult = NullValue | string | SourceDescription;

export type TransformResult =
  | NullValue
  | string
  | Omit<SourceDescription, 'code'> & { code?: string | BindingMagicString };

export type RenderedChunkMeta = { chunks: Record<string, RenderedChunk> };

export interface FunctionPluginHooks {
  [DEFINED_HOOK_NAMES.onLog]: (
    this: MinimalPluginContext,
    level: LogLevel,
    log: RollupLog,
  ) => NullValue | boolean;

  [DEFINED_HOOK_NAMES.options]: (
    this: MinimalPluginContext,
    options: InputOptions,
  ) => NullValue | InputOptions;

  // TODO find a way to make `this: PluginContext` work.
  [DEFINED_HOOK_NAMES.outputOptions]: (
    this: MinimalPluginContext,
    options: OutputOptions,
  ) => NullValue | OutputOptions;

  // --- Build hooks ---

  [DEFINED_HOOK_NAMES.buildStart]: (
    this: PluginContext,
    options: NormalizedInputOptions,
  ) => void;

  [DEFINED_HOOK_NAMES.resolveId]: (
    this: PluginContext,
    source: string,
    importer: string | undefined,
    extraOptions: ResolveIdExtraOptions,
  ) => ResolveIdResult;

  /**
   * @deprecated
   * This hook is only for rollup plugin compatibility. Please use `resolveId` instead.
   */
  [DEFINED_HOOK_NAMES.resolveDynamicImport]: (
    this: PluginContext,
    source: string,
    importer: string | undefined,
  ) => ResolveIdResult;

  [DEFINED_HOOK_NAMES.load]: (
    this: PluginContext,
    id: string,
  ) => MaybePromise<LoadResult>;

  [DEFINED_HOOK_NAMES.transform]: (
    this: TransformPluginContext,
    code: string,
    id: string,
    meta: BindingTransformHookExtraArgs & {
      moduleType: ModuleType;
      magicString?: BindingMagicString;
    },
  ) => TransformResult;

  [DEFINED_HOOK_NAMES.moduleParsed]: (
    this: PluginContext,
    moduleInfo: ModuleInfo,
  ) => void;

  [DEFINED_HOOK_NAMES.buildEnd]: (this: PluginContext, err?: Error) => void;

  // --- Generate hooks ---

  [DEFINED_HOOK_NAMES.renderStart]: (
    this: PluginContext,
    outputOptions: NormalizedOutputOptions,
    inputOptions: NormalizedInputOptions,
  ) => void;

  [DEFINED_HOOK_NAMES.renderChunk]: (
    this: PluginContext,
    code: string,
    chunk: RenderedChunk,
    outputOptions: NormalizedOutputOptions,
    meta: RenderedChunkMeta,
  ) =>
    | NullValue
    | string
    | {
      code: string;
      map?: SourceMapInput;
    };

  [DEFINED_HOOK_NAMES.augmentChunkHash]: (
    this: PluginContext,
    chunk: RenderedChunk,
  ) => string | void;

  [DEFINED_HOOK_NAMES.renderError]: (this: PluginContext, error: Error) => void;

  [DEFINED_HOOK_NAMES.generateBundle]: (
    this: PluginContext,
    outputOptions: NormalizedOutputOptions,
    bundle: OutputBundle,
    isWrite: boolean,
  ) => void;

  [DEFINED_HOOK_NAMES.writeBundle]: (
    this: PluginContext,
    outputOptions: NormalizedOutputOptions,
    bundle: OutputBundle,
  ) => void;

  [DEFINED_HOOK_NAMES.closeBundle]: (this: PluginContext) => void;

  // --- watch hooks ---
  [DEFINED_HOOK_NAMES.watchChange]: (
    this: PluginContext,
    id: string,
    event: { event: ChangeEvent },
  ) => void;

  [DEFINED_HOOK_NAMES.closeWatcher]: (this: PluginContext) => void;
}

export type ChangeEvent = 'create' | 'update' | 'delete';

export type PluginOrder = 'pre' | 'post' | null;

export type ObjectHookMeta = { order?: PluginOrder };

export type ObjectHook<T, O = {}> = T | ({ handler: T } & ObjectHookMeta & O);
type SyncPluginHooks = DefinedHookNames[
  | 'augmentChunkHash'
  | 'onLog'
  | 'outputOptions'
];
// | 'renderDynamicImport'
// | 'resolveFileUrl'
// | 'resolveImportMeta'

export type AsyncPluginHooks = Exclude<
  keyof FunctionPluginHooks,
  SyncPluginHooks
>;

type FirstPluginHooks = DefinedHookNames[
  | 'load'
  // | 'renderDynamicImport'
  | 'resolveDynamicImport'
  // | 'resolveFileUrl'
  | 'resolveId'
];
// | 'resolveImportMeta'
// | 'shouldTransformCachedModule'

type SequentialPluginHooks = DefinedHookNames[
  | 'augmentChunkHash'
  | 'generateBundle'
  | 'onLog'
  | 'options'
  | 'outputOptions'
  | 'renderChunk'
  | 'transform'
];

type AddonHooks = DefinedHookNames[
  | 'banner'
  | 'footer'
  | 'intro'
  | 'outro'
];

type OutputPluginHooks = DefinedHookNames[
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
];

export type ParallelPluginHooks = Exclude<
  keyof FunctionPluginHooks | AddonHooks,
  FirstPluginHooks | SequentialPluginHooks
>;

export type HookFilterExtension<K extends keyof FunctionPluginHooks> = K extends
  'transform' ? { filter?: TUnionWithTopLevelFilterExpressionArray<HookFilter> }
  : K extends 'load' ? {
      filter?: TUnionWithTopLevelFilterExpressionArray<Pick<HookFilter, 'id'>>;
    }
  : K extends 'resolveId' ? {
      filter?: TUnionWithTopLevelFilterExpressionArray<
        { id?: GeneralHookFilter<RegExp> }
      >;
    }
  : K extends 'renderChunk' ? {
      filter?: TUnionWithTopLevelFilterExpressionArray<
        Pick<HookFilter, 'code'>
      >;
    }
  : {};

export type PluginHooks = {
  [K in keyof FunctionPluginHooks]: ObjectHook<
    K extends AsyncPluginHooks ? MakeAsync<FunctionPluginHooks[K]>
      : FunctionPluginHooks[K],
    & HookFilterExtension<K>
    & (K extends ParallelPluginHooks ? {
        /**
         * @deprecated
         * this is only for rollup Plugin type compatibility.
         * hooks always work as `sequential: true`.
         */
        sequential?: boolean;
      }
      : {})
  >;
};

type AddonHookFunction = (
  this: PluginContext,
  chunk: RenderedChunk,
) => string | Promise<string>;

type AddonHook = string | AddonHookFunction;

interface OutputPlugin
  extends
    Partial<{ [K in OutputPluginHooks]: PluginHooks[K] }>,
    Partial<{ [K in AddonHooks]: ObjectHook<AddonHook> }>
{
  // cacheKey?: string
  name: string;
  // version?: string
}

export interface Plugin<A = any> extends OutputPlugin, Partial<PluginHooks> {
  // for inter-plugin communication
  api?: A;
}

export type RolldownPlugin<A = any> =
  | Plugin<A>
  | BuiltinPlugin
  | ParallelPlugin;
export type RolldownPluginOption<A = any> = MaybePromise<
  | NullValue<RolldownPlugin<A>>
  | { name: string } // for rollup plugin compatibility
  | false
  | RolldownPluginOption[]
>;
export type RolldownOutputPlugin = OutputPlugin | BuiltinPlugin;
export type RolldownOutputPluginOption = MaybePromise<
  | NullValue<RolldownOutputPlugin>
  | { name: string } // for rollup plugin compatibility
  | false
  | RolldownOutputPluginOption[]
>;
