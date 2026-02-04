import type { Program } from '@oxc-project/types';
import type { InputOptions, OutputOptions } from '..';
import type {
  BindingHookResolveIdExtraArgs,
  BindingMagicString,
  BindingTransformHookExtraArgs,
} from '../binding.cjs';
import type { BuiltinPlugin } from '../builtin-plugin/utils';
import type { DefinedHookNames } from '../constants/plugin';
import type { DEFINED_HOOK_NAMES } from '../constants/plugin';
import type { LogLevel, RolldownLog } from '../log/logging';
import type { NormalizedInputOptions } from '../options/normalized-input-options';
import type { NormalizedOutputOptions } from '../options/normalized-output-options';
import type { ModuleInfo } from '../types/module-info';
import type { OutputBundle } from '../types/output-bundle';
import type { RenderedChunk } from '../types/rolldown-output';
import type { SourceMapInput } from '../types/sourcemap';
import type { MakeAsync, MaybePromise, NullValue, PartialNull } from '../types/utils';
import type { GeneralHookFilter, HookFilter } from './hook-filter';
import type { MinimalPluginContext } from './minimal-plugin-context';
import type { ParallelPlugin } from './parallel-plugin';
import type { PluginContext } from './plugin-context';
import type { TransformPluginContext } from './transform-plugin-context';
import type { TopLevelFilterExpression } from '@rolldown/pluginutils';
// oxlint-disable-next-line no-unused-vars -- this is used in JSDoc links
import type { rolldown } from '../api/rolldown/index';
// oxlint-disable-next-line no-unused-vars -- this is used in JSDoc links
import type { TreeshakingOptions } from '../types/module-side-effects';
// oxlint-disable-next-line no-unused-vars -- this is used in JSDoc links
import type { WatcherOptions } from '../options/input-options';
// oxlint-disable-next-line no-unused-vars -- this is used in JSDoc links
import type { RolldownBuild } from '../api/rolldown/rolldown-build';

type ModuleSideEffects = boolean | 'no-treeshake' | null;
export { withFilter } from './with-filter';

// ref: https://github.com/microsoft/TypeScript/issues/33471#issuecomment-1376364329
/** @category Plugin APIs */
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

/** @category Plugin APIs */
export type ImportKind = BindingHookResolveIdExtraArgs['kind'];

/** @category Plugin APIs */
export interface CustomPluginOptions {
  [plugin: string]: any;
}

/** @category Plugin APIs */
export interface ModuleOptions {
  moduleSideEffects: ModuleSideEffects;
  /** See [Custom module meta-data section](https://rolldown.rs/apis/plugin-api/inter-plugin-communication#custom-module-meta-data) for more details. */
  meta: CustomPluginOptions;
  // flag used to check if user directly modified the `ModuleInfo`
  // this is used to sync state between Rust and JavaScript
  invalidate?: boolean;
  packageJsonPath?: string;
}

/** @category Plugin APIs */
export interface ResolvedId extends ModuleOptions {
  external: boolean | 'absolute';
  id: string;
}

// Separate interface to dedupe the JSDoc comment
interface SpecifiedModuleOptions {
  /**
   * Indicates whether the module has side effects to Rolldown.
   *
   * - If `false` is set and no other module imports anything from this module, then this module will not be included in the bundle even if the module would have side effects.
   * - If `true` is set, Rolldown will use its default algorithm to include all statements in the module that has side effects.
   * - If `"no-treeshake"` is set, treeshaking will be disabled for this module, and this module will be included in one of the chunks even if it is empty.
   *
   * The precedence of this option is as follows (highest to lowest):
   * 1. {@linkcode Plugin.transform | transform} hook's returned `moduleSideEffects` option
   * 2. {@linkcode Plugin.load | load} hook's returned `moduleSideEffects` option
   * 3. {@linkcode Plugin.resolveId | resolveId} hook's returned `moduleSideEffects` option
   * 4. {@linkcode TreeshakingOptions.moduleSideEffects | treeshake.moduleSideEffects} option
   * 5. `sideEffects` field in the `package.json` file
   * 6. `true` (default)
   */
  moduleSideEffects?: ModuleSideEffects | null;
}

/** @category Plugin APIs */
export interface PartialResolvedId
  extends SpecifiedModuleOptions, Partial<PartialNull<ModuleOptions>> {
  /**
   * Whether this id should be treated as external.
   *
   * Relative external ids, i.e. ids starting with `./` or `../`, will not be internally
   * converted to an absolute id and converted back to a relative id in the output,
   * but are instead included in the output unchanged.
   * If you want relative ids to be re-normalized and deduplicated instead, return
   * an absolute file system location as id and choose `external: "relative"`.
   *
   * - If `true`, absolute ids will be converted to relative ids based on the user's choice for the {@linkcode InputOptions.makeAbsoluteExternalsRelative | makeAbsoluteExternalsRelative} option.
   * - If `'relative'`, absolute ids will always be converted to relative ids.
   * - If `'absolute'`, absolute ids will always be kept as absolute ids.
   */
  external?: boolean | 'absolute' | 'relative';
  id: string;
}

/** @category Plugin APIs */
export interface SourceDescription
  extends SpecifiedModuleOptions, Partial<PartialNull<ModuleOptions>> {
  code: string;
  /**
   * The source map for the transformation.
   *
   * If the transformation does not move code, you can preserve existing sourcemaps by setting this to `null`.
   *
   * See [Source Code Transformations section](https://rolldown.rs/apis/plugin-api/transformations#source-code-transformations) for more details.
   */
  map?: SourceMapInput;
  moduleType?: ModuleType;
}

/** @inline */
export interface ResolveIdExtraOptions {
  /**
   * Plugin-specific options.
   *
   * See [Custom resolver options section](https://rolldown.rs/apis/plugin-api/inter-plugin-communication#custom-resolver-options) for more details.
   */
  custom?: CustomPluginOptions;
  /**
   * Whether this is resolution for an entry point.
   *
   * {@include ./docs/plugin-hooks-resolveid-isentry.md}
   */
  isEntry: boolean;
  /**
   * The kind of import being resolved.
   *
   * - `import-statement`: `import { foo } from './lib.js';`
   * - `dynamic-import`: `import('./lib.js')`
   * - `require-call`: `require('./lib.js')`
   * - `import-rule`: `@import 'bg-color.css'` (experimental)
   * - `url-token`: `url('./icon.png')` (experimental)
   * - `new-url`: `new URL('./worker.js', import.meta.url)` (experimental)
   * - `hot-accept`: `import.meta.hot.accept('./lib.js', () => {})` (experimental)
   */
  kind: BindingHookResolveIdExtraArgs['kind'];
}

/** @inline @category Plugin APIs */
export type ResolveIdResult = string | NullValue | false | PartialResolvedId;

/** @inline @category Plugin APIs */
export type LoadResult = NullValue | string | SourceDescription;

/** @inline @category Plugin APIs */
export type TransformResult =
  | NullValue
  | string
  | (Omit<SourceDescription, 'code'> & { code?: string | BindingMagicString });

export type RenderedChunkMeta = {
  /**
   * Contains information about all chunks that are being rendered.
   * This is useful to explore the entire chunk graph.
   */
  chunks: Record<string, RenderedChunk>;
  /**
   * A lazily-created MagicString instance for the chunk's code.
   * Use this to perform string transformations with automatic source map support.
   * This is only available when `experimental.nativeMagicString` is enabled.
   */
  magicString?: BindingMagicString;
};

/** @category Plugin APIs */
export interface FunctionPluginHooks {
  /**
   * A function that receives and filters logs and warnings generated by Rolldown and
   * plugins before they are passed to the {@linkcode InputOptions.onLog | onLog} option
   * or printed to the console.
   *
   * If `false` is returned, the log will be filtered out.
   * Otherwise, the log will be handed to the `onLog` hook of the next plugin,
   * the {@linkcode InputOptions.onLog | onLog} option, or printed to the console.
   * Plugins can also change the log level of a log or turn a log into an error by passing
   * the `log` object to {@linkcode MinimalPluginContext.error | this.error},
   * {@linkcode MinimalPluginContext.warn | this.warn},
   * {@linkcode MinimalPluginContext.info | this.info} or
   * {@linkcode MinimalPluginContext.debug | this.debug} and returning `false`.
   *
   * {@include ./docs/plugin-hooks-onlog.md}
   *
   * @group Build Hooks
   */
  [DEFINED_HOOK_NAMES.onLog]: (
    this: MinimalPluginContext,
    level: LogLevel,
    log: RolldownLog,
  ) => NullValue | boolean;

  /**
   * Replaces or manipulates the options object passed to {@linkcode rolldown | rolldown()}.
   *
   * Returning `null` does not replace anything.
   *
   * If you just need to read the options, it is recommended to use
   * the {@linkcode buildStart} hook as that hook has access to the options
   * after the transformations from all `options` hooks have been taken into account.
   *
   * @group Build Hooks
   */
  [DEFINED_HOOK_NAMES.options]: (
    this: MinimalPluginContext,
    options: InputOptions,
  ) => NullValue | InputOptions;

  // TODO find a way to make `this: PluginContext` work.
  /**
   * Replaces or manipulates the output options object passed to
   * {@linkcode RolldownBuild.generate | bundle.generate()} or
   * {@linkcode RolldownBuild.write | bundle.write()}.
   *
   * Returning null does not replace anything.
   *
   * If you just need to read the output options, it is recommended to use
   * the {@linkcode renderStart} hook as this hook has access to the output options
   * after the transformations from all `outputOptions` hooks have been taken into account.
   *
   * @group Build Hooks
   */
  [DEFINED_HOOK_NAMES.outputOptions]: (
    this: MinimalPluginContext,
    options: OutputOptions,
  ) => NullValue | OutputOptions;

  /**
   * Called on each {@linkcode rolldown | rolldown()} build.
   *
   * This is the recommended hook to use when you need access to the options passed to {@linkcode rolldown | rolldown()} as it takes the transformations by all options hooks into account and also contains the right default values for unset options.
   *
   * @group Build Hooks
   */
  [DEFINED_HOOK_NAMES.buildStart]: (this: PluginContext, options: NormalizedInputOptions) => void;

  /**
   * Defines a custom resolver.
   *
   * A resolver can be useful for e.g. locating third-party dependencies.
   *
   * Returning `null` defers to other `resolveId` hooks and eventually the default resolution behavior.
   * Returning `false` signals that `source` should be treated as an external module and not included in the bundle. If this happens for a relative import, the id will be renormalized the same way as when the {@linkcode InputOptions.external} option is used.
   * If you return an object, then it is possible to resolve an import to a different id while excluding it from the bundle at the same time.
   *
   * Note that while `resolveId` will be called for each import of a module and can therefore
   * resolve to the same `id` many times, values for `external`, `meta` or `moduleSideEffects`
   * can only be set once before the module is loaded. The reason is that after this call,
   * Rolldown will continue with the {@linkcode load} and {@linkcode transform} hooks for that
   * module that may override these values and should take precedence if they do so.
   *
   * @group Build Hooks
   */
  [DEFINED_HOOK_NAMES.resolveId]: (
    this: PluginContext,
    /**
     * The importee exactly as it is written in the import statement.
     *
     * For example, given `import foo from './foo.js'`, the `source` will be `"./foo.js"`.
     */
    source: string,
    /**
     * The fully resolved id of the importing module.
     *
     * When resolving entry points, `importer` will usually be undefined.
     * An exception here is entry points generated via
     * {@linkcode PluginContext.emitFile | this.emitFile} as here, you can provide
     * an importer argument.
     * For those cases, the {@linkcode ResolveIdExtraOptions.isEntry | isEntry} option
     * will tell you if we are resolving a user defined entry point, an emitted chunk,
     * or if the `isEntry` parameter was provided for the
     * {@linkcode PluginContext.resolve | this.resolve} function.
     */
    importer: string | undefined,
    extraOptions: ResolveIdExtraOptions,
  ) => ResolveIdResult;

  /**
   * Defines a custom resolver for dynamic imports.
   *
   * @deprecated
   * This hook exists only for Rollup compatibility. Please use {@linkcode resolveId} instead.
   *
   * @group Build Hooks
   */
  [DEFINED_HOOK_NAMES.resolveDynamicImport]: (
    this: PluginContext,
    /**
     * The importee exactly as it is written in the import statement.
     *
     * For example, given `import('./foo.js')`, the `source` will be `"./foo.js"`.
     *
     * In Rollup, this parameter can also be an AST node. But Rolldown always provides a string.
     */
    source: string,
    /**
     * The fully resolved id of the importing module.
     *
     * This will be `undefined` when {@linkcode PluginContext.resolve | this.resolve(source, undefined, { kind: 'dynamic-import' })} is called.
     */
    importer: string | undefined,
  ) => ResolveIdResult;

  /**
   * Defines a custom loader.
   *
   * Returning `null` defers to other `load` hooks or the built-in loading mechanism.
   *
   * You can use {@linkcode PluginContext.getModuleInfo | this.getModuleInfo()} to find out the previous values of `meta`, `moduleSideEffects` inside this hook.
   *
   * @group Build Hooks
   */
  [DEFINED_HOOK_NAMES.load]: (this: PluginContext, id: string) => MaybePromise<LoadResult>;

  /**
   * Can be used to transform individual modules.
   *
   * Note that it's possible to return only properties and no code transformations.
   *
   * You can use {@linkcode PluginContext.getModuleInfo | this.getModuleInfo()} to find out the previous values of `meta`, `moduleSideEffects` inside this hook.
   *
   * {@include ./docs/plugin-hooks-transform.md}
   *
   * @group Build Hooks
   */
  [DEFINED_HOOK_NAMES.transform]: (
    this: TransformPluginContext,
    code: string,
    id: string,
    meta: BindingTransformHookExtraArgs & {
      moduleType: ModuleType;
      magicString?: BindingMagicString;
      ast?: Program;
    },
  ) => TransformResult;

  /**
   * This hook is called each time a module has been fully parsed by Rolldown.
   *
   * This hook will wait until all imports are resolved so that the information in
   * {@linkcode ModuleInfo.importedIds | moduleInfo.importedIds},
   * {@linkcode ModuleInfo.dynamicallyImportedIds | moduleInfo.dynamicallyImportedIds}
   * are complete and accurate. Note however that information about importing modules
   * may be incomplete as additional importers could be discovered later.
   * If you need this information, use the {@linkcode buildEnd} hook.
   *
   * @group Build Hooks
   */
  [DEFINED_HOOK_NAMES.moduleParsed]: (this: PluginContext, moduleInfo: ModuleInfo) => void;

  /**
   * Called when Rolldown has finished bundling, but before Output Generation Hooks.
   * If an error occurred during the build, it is passed on to this hook.
   *
   * @group Build Hooks
   */
  [DEFINED_HOOK_NAMES.buildEnd]: (
    this: PluginContext,
    /** The error occurred during the build if applicable. */
    err?: Error,
  ) => void;

  /**
   * Called initially each time {@linkcode RolldownBuild.generate | bundle.generate()} or
   * {@linkcode RolldownBuild.write | bundle.write()} is called.
   *
   * To get notified when generation has completed, use the {@linkcode generateBundle} and
   * {@linkcode renderError} hooks.
   *
   * This is the recommended hook to use when you need access to the output options passed to
   * {@linkcode RolldownBuild.generate | bundle.generate()} or
   * {@linkcode RolldownBuild.write | bundle.write()} as it takes the transformations by all outputOptions hooks into account and also contains the right default values for unset options.
   *
   * It also receives the input options passed to {@linkcode rolldown | rolldown()} so that
   * plugins that can be used as output plugins, i.e. plugins that only use generate phase hooks,
   * can get access to them.
   *
   * @group Output Generation Hooks
   */
  [DEFINED_HOOK_NAMES.renderStart]: (
    this: PluginContext,
    outputOptions: NormalizedOutputOptions,
    inputOptions: NormalizedInputOptions,
  ) => void;

  /**
   * Can be used to transform individual chunks. Called for each Rolldown output chunk file.
   *
   * Returning null will apply no transformations. If you change code in this hook and want to support source maps, you need to return a map describing your changes, see [Source Code Transformations section](https://rolldown.rs/apis/plugin-api/transformations#source-code-transformations).
   *
   * `chunk` is mutable and changes applied in this hook will propagate to other plugins and
   * to the generated bundle.
   * That means if you add or remove imports or exports in this hook, you should update
   * {@linkcode RenderedChunk.imports | imports}, {@linkcode RenderedChunk.importedBindings | importedBindings} and/or {@linkcode RenderedChunk.exports | exports} accordingly.
   *
   * @group Output Generation Hooks
   */
  [DEFINED_HOOK_NAMES.renderChunk]: (
    this: PluginContext,
    code: string,
    chunk: RenderedChunk,
    outputOptions: NormalizedOutputOptions,
    meta: RenderedChunkMeta,
  ) =>
    | NullValue
    | string
    | BindingMagicString
    | {
        code: string | BindingMagicString;
        map?: SourceMapInput;
      };

  /**
   * Can be used to augment the hash of individual chunks. Called for each Rolldown output chunk.
   *
   * Returning a falsy value will not modify the hash.
   * Truthy values will be used as an additional source for hash calculation.
   *
   * {@include ./docs/plugin-hooks-augmentchunkhash.md}
   *
   * @group Output Generation Hooks
   */
  [DEFINED_HOOK_NAMES.augmentChunkHash]: (
    this: PluginContext,
    chunk: RenderedChunk,
  ) => string | void;

  /**
   * Called when Rolldown encounters an error during
   * {@linkcode RolldownBuild.generate | bundle.generate()} or
   * {@linkcode RolldownBuild.write | bundle.write()}.
   *
   * To get notified when generation completes successfully, use the
   * {@linkcode generateBundle} hook.
   *
   * @group Output Generation Hooks
   */
  [DEFINED_HOOK_NAMES.renderError]: (this: PluginContext, error: Error) => void;

  /**
   * Called at the end of {@linkcode RolldownBuild.generate | bundle.generate()} or
   * immediately before the files are written in
   * {@linkcode RolldownBuild.write | bundle.write()}.
   *
   * To modify the files after they have been written, use the {@linkcode writeBundle} hook.
   *
   * {@include ./docs/plugin-hooks-generatebundle.md}
   *
   * @group Output Generation Hooks
   */
  [DEFINED_HOOK_NAMES.generateBundle]: (
    this: PluginContext,
    outputOptions: NormalizedOutputOptions,
    /** Provides the full list of files being written or generated along with their details. */
    bundle: OutputBundle,
    isWrite: boolean,
  ) => void;

  /**
   * Called only at the end of {@linkcode RolldownBuild.write | bundle.write()} once
   * all files have been written.
   *
   * @group Output Generation Hooks
   */
  [DEFINED_HOOK_NAMES.writeBundle]: (
    this: PluginContext,
    outputOptions: NormalizedOutputOptions,
    /** Provides the full list of files being written or generated along with their details. */
    bundle: OutputBundle,
  ) => void;

  /**
   * Can be used to clean up any external service that may be running.
   *
   * Rolldown's CLI will make sure this hook is called after each run, but it is the responsibility
   * of users of the JavaScript API to manually call
   * {@linkcode RolldownBuild.close | bundle.close()} once they are done generating bundles.
   * For that reason, any plugin relying on this feature should carefully mention this in
   * its documentation.
   *
   * If a plugin wants to retain resources across builds in watch mode, they can check for
   * {@linkcode PluginContextMeta.watchMode | this.meta.watchMode} in this hook and perform
   * the necessary cleanup for watch mode in closeWatcher.
   *
   * @group Output Generation Hooks
   */
  [DEFINED_HOOK_NAMES.closeBundle]: (
    this: PluginContext,
    /** An error that occurred during build or the buildEnd hook, if any. */
    error?: Error,
  ) => void;

  /**
   * Notifies a plugin whenever Rolldown has detected a change to a monitored file in watch mode.
   *
   * If a build is currently running, this hook is called once the build finished.
   * It will be called once for every file that changed.
   *
   * This hook cannot be used by output plugins.
   *
   * If you need to be notified immediately when a file changed, you can use the {@linkcode WatcherOptions.onInvalidate | watch.onInvalidate} option.
   *
   * @group Build Hooks
   */
  [DEFINED_HOOK_NAMES.watchChange]: (
    this: PluginContext,
    id: string,
    event: { event: ChangeEvent },
  ) => void;

  /**
   * Notifies a plugin when the watcher process will close so that all open resources can be closed too.
   *
   * This hook cannot be used by output plugins.
   *
   * @group Build Hooks
   */
  [DEFINED_HOOK_NAMES.closeWatcher]: (this: PluginContext) => void;
}

export type ChangeEvent = 'create' | 'update' | 'delete';

export type PluginOrder = 'pre' | 'post' | null;

/** @inline */
export type ObjectHookMeta = {
  order?: PluginOrder;
};

/**
 * A hook in a function or an object form with additional properties.
 *
 * @typeParam T - The type of the hook function.
 * @typeParam O - Additional properties that are specific to some hooks.
 *
 * {@include ./docs/object-hook.md}
 *
 * @category Plugin APIs
 */
export type ObjectHook<T, O = {}> = T | ({ handler: T } & ObjectHookMeta & O);
type SyncPluginHooks = DefinedHookNames['augmentChunkHash' | 'onLog' | 'outputOptions'];
// | 'renderDynamicImport'
// | 'resolveFileUrl'
// | 'resolveImportMeta'

/** @category Plugin APIs */
export type AsyncPluginHooks = Exclude<keyof FunctionPluginHooks, SyncPluginHooks>;

type FirstPluginHooks = DefinedHookNames[
  | 'load'
  // | 'renderDynamicImport'
  | 'resolveDynamicImport'
  // | 'resolveFileUrl'
  | 'resolveId'];
// | 'resolveImportMeta'
// | 'shouldTransformCachedModule'

type SequentialPluginHooks = DefinedHookNames[
  | 'augmentChunkHash'
  | 'generateBundle'
  | 'onLog'
  | 'options'
  | 'outputOptions'
  | 'renderChunk'
  | 'transform'];

interface AddonHooks {
  /**
   * A hook equivalent to {@linkcode OutputOptions.banner | output.banner} option.
   *
   * @group Output Generation Hooks
   */
  [DEFINED_HOOK_NAMES.banner]: AddonHook;
  /**
   * A hook equivalent to {@linkcode OutputOptions.footer | output.footer} option.
   *
   * @group Output Generation Hooks
   */
  [DEFINED_HOOK_NAMES.footer]: AddonHook;
  /**
   * A hook equivalent to {@linkcode OutputOptions.intro | output.intro} option.
   *
   * @group Output Generation Hooks
   */
  [DEFINED_HOOK_NAMES.intro]: AddonHook;
  /**
   * A hook equivalent to {@linkcode OutputOptions.outro | output.outro} option.
   *
   * @group Output Generation Hooks
   */
  [DEFINED_HOOK_NAMES.outro]: AddonHook;
}

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
  | 'writeBundle'];

/** @internal */
export type ParallelPluginHooks = Exclude<
  keyof FunctionPluginHooks | keyof AddonHooks,
  FirstPluginHooks | SequentialPluginHooks
>;

/** @category Plugin APIs */
export type HookFilterExtension<K extends keyof FunctionPluginHooks> = K extends 'transform'
  ? {
      filter?: HookFilter | TopLevelFilterExpression[];
    }
  : K extends 'load'
    ? {
        filter?: Pick<HookFilter, 'id'> | TopLevelFilterExpression[];
      }
    : K extends 'resolveId'
      ? {
          filter?:
            | {
                id?: GeneralHookFilter<RegExp>;
              }
            | TopLevelFilterExpression[];
        }
      : K extends 'renderChunk'
        ? {
            filter?: Pick<HookFilter, 'code'> | TopLevelFilterExpression[];
          }
        : {};

export type PluginHooks = {
  [K in keyof FunctionPluginHooks]: ObjectHook<
    K extends AsyncPluginHooks ? MakeAsync<FunctionPluginHooks[K]> : FunctionPluginHooks[K],
    HookFilterExtension<K> &
      (K extends ParallelPluginHooks
        ? {
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

type AddonHookFunction = (this: PluginContext, chunk: RenderedChunk) => string | Promise<string>;

type AddonHook = string | AddonHookFunction;

interface OutputPlugin
  extends
    Partial<{
      // Use key remapping pattern to provide better  "go to definition" experience.
      // https://github.com/rolldown/rolldown/pull/7610
      [K in keyof PluginHooks as K & OutputPluginHooks]: PluginHooks[K];
    }>,
    Partial<{ [K in keyof AddonHooks]: ObjectHook<AddonHook> }> {
  // cacheKey?: string
  /** The name of the plugin, for use in error messages and logs. */
  name: string;
  /** The version of the plugin, for use in inter-plugin communication scenarios. */
  version?: string;
}

/**
 * The Plugin interface.
 *
 * See [Plugin API document](https://rolldown.rs/apis/plugin-api) for details.
 *
 * @typeParam A - The type of the {@link Plugin.api | api} property.
 *
 * @category Plugin APIs
 */
export interface Plugin<A = any> extends OutputPlugin, Partial<PluginHooks> {
  /**
   * Used for inter-plugin communication.
   */
  api?: A;
}

export type RolldownPlugin<A = any> = Plugin<A> | BuiltinPlugin | ParallelPlugin;
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
