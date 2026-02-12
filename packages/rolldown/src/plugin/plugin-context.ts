import type { Program } from '@oxc-project/types';
import type {
  BindingPluginContext,
  BindingPluginContextResolveOptions,
  BindingVitePluginCustom,
  ParserOptions,
} from '../binding.cjs';
import type { LogHandler } from '../log/log-handler';
import { LOG_LEVEL_WARN, type LogLevelOption } from '../log/logging';
import { logCycleLoading } from '../log/logs';
import type { OutputOptions } from '../options/output-options';
import { parseAst } from '../parse-ast-index';
import {
  type MinimalPluginContext,
  MinimalPluginContextImpl,
} from '../plugin/minimal-plugin-context';
import type { Extends, TypeAssert } from '../types/assert';
import type { ModuleInfo } from '../types/module-info';
import type { SourceMap } from '../types/rolldown-output';
import { bindingifySourcemap } from '../types/sourcemap';
import type { PartialNull } from '../types/utils';
import { type AssetSource, bindingAssetSource } from '../utils/asset-source';
import { bindingifyPreserveEntrySignatures } from '../utils/bindingify-input-options';
import { unreachable } from '../utils/misc';
import { fsModule, type RolldownFsModule } from './fs';
import type { CustomPluginOptions, ModuleOptions, Plugin, ResolvedId } from './index';
import type { PluginContextData } from './plugin-context-data';
// oxlint-disable-next-line no-unused-vars -- this is used in JSDoc links
import type { InputOptions } from '../options/input-options';
// oxlint-disable-next-line no-unused-vars -- this is used in JSDoc links
import type { PreRenderedAsset } from '../options/output-options';
// oxlint-disable-next-line no-unused-vars -- this is used in JSDoc links
import type { OutputAsset } from '../types/rolldown-output';
// oxlint-disable-next-line no-unused-vars -- this is used in JSDoc links
import type { ResolveIdExtraOptions } from '../index';

/**
 * Either a {@linkcode name} or a {@linkcode fileName} can be supplied.
 * If a {@linkcode fileName} is provided, it will be used unmodified as the name
 * of the generated file, throwing an error if this causes a conflict.
 * Otherwise, if a {@linkcode name} is supplied, this will be used as substitution
 * for `[name]` in the corresponding
 * {@linkcode OutputOptions.assetFileNames | output.assetFileNames} pattern, possibly
 * adding a unique number to the end of the file name to avoid conflicts.
 * If neither a {@linkcode name} nor {@linkcode fileName} is supplied, a default name will be used.
 *
 * @category Plugin APIs
 */
export interface EmittedAsset {
  type: 'asset';
  name?: string;
  fileName?: string;
  /**
   * An absolute path to the original file if this asset corresponds to a file on disk.
   *
   * This property will be passed on to subsequent plugin hooks that receive a
   * {@linkcode PreRenderedAsset} or an {@linkcode OutputAsset} like
   * {@linkcode Plugin.generateBundle | generateBundle}.
   * In watch mode, Rolldown will also automatically watch this file for changes and
   * trigger a rebuild if it changes. Therefore, it is not necessary to call
   * {@linkcode PluginContext.addWatchFile | this.addWatchFile} for this file.
   */
  originalFileName?: string;
  source: AssetSource;
}

/**
 * Either a {@linkcode name} or a {@linkcode fileName} can be supplied.
 * If a {@linkcode fileName} is provided, it will be used unmodified as the name
 * of the generated file, throwing an error if this causes a conflict.
 * Otherwise, if a {@linkcode name} is supplied, this will be used as substitution
 * for `[name]` in the corresponding
 * {@linkcode OutputOptions.chunkFileNames | output.chunkFileNames} pattern, possibly
 * adding a unique number to the end of the file name to avoid conflicts.
 * If neither a {@linkcode name} nor {@linkcode fileName} is supplied, a default name will be used.
 *
 * @category Plugin APIs
 */
export interface EmittedChunk {
  type: 'chunk';
  name?: string;
  fileName?: string;
  /**
   * When provided, this will override
   * {@linkcode InputOptions.preserveEntrySignatures | preserveEntrySignatures} for this particular
   * chunk.
   */
  preserveSignature?: 'strict' | 'allow-extension' | 'exports-only' | false;
  /**
   * The module id of the entry point of the chunk.
   *
   * It will be passed through build hooks just like regular entry points,
   * starting with {@linkcode Plugin.resolveId | resolveId}.
   */
  id: string;
  /**
   * The value to be passed to {@linkcode Plugin.resolveId | resolveId}'s {@linkcode importer} parameter when resolving the entry point.
   * This is important to properly resolve relative paths. If it is not provided,
   * paths will be resolved relative to the current working directory.
   */
  importer?: string;
}

/** @category Plugin APIs */
export interface EmittedPrebuiltChunk {
  type: 'prebuilt-chunk';
  fileName: string;
  /**
   * A semantic name for the chunk. If not provided, `fileName` will be used.
   */
  name?: string;
  /**
   * The code of this chunk.
   */
  code: string;
  /**
   * The list of exported variable names from this chunk.
   *
   * This should be provided if the chunk exports any variables.
   */
  exports?: string[];
  /**
   * The corresponding source map for this chunk.
   */
  map?: SourceMap;
  sourcemapFileName?: string;
  /**
   * The module id of the facade module for this chunk, if any.
   */
  facadeModuleId?: string;
  /**
   * Whether this chunk corresponds to an entry point.
   */
  isEntry?: boolean;
  /**
   * Whether this chunk corresponds to a dynamic entry point.
   */
  isDynamicEntry?: boolean;
}

/** @inline @category Plugin APIs */
export type EmittedFile = EmittedAsset | EmittedChunk | EmittedPrebuiltChunk;

/** @category Plugin APIs */
export interface PluginContextResolveOptions {
  /**
   * The value for {@linkcode ResolveIdExtraOptions.kind | kind} passed to
   * {@linkcode Plugin.resolveId | resolveId} hooks.
   */
  kind?: BindingPluginContextResolveOptions['importKind'];
  /**
   * The value for {@linkcode ResolveIdExtraOptions.isEntry | isEntry} passed to
   * {@linkcode Plugin.resolveId | resolveId} hooks.
   *
   * @default `false` if there's an importer, `true` otherwise.
   */
  isEntry?: boolean;
  /**
   * Whether the {@linkcode Plugin.resolveId | resolveId} hook of the plugin from
   * which {@linkcode PluginContext.resolve | this.resolve} is called will be skipped
   * when resolving.
   *
   * {@include ./docs/plugin-context-resolve-skipself.md}
   *
   * @default true
   */
  skipSelf?: boolean;
  /**
   * Plugin-specific options.
   *
   * See [Custom resolver options section](https://rolldown.rs/apis/plugin-api/inter-plugin-communication#custom-resolver-options) for more details.
   */
  custom?: CustomPluginOptions;
}

/** @inline */
export type GetModuleInfo = (moduleId: string) => ModuleInfo | null;

/** @category Plugin APIs */
export interface PluginContext extends MinimalPluginContext {
  /**
   * Provides abstract access to the file system.
   */
  fs: RolldownFsModule;
  /**
   * Emits a new file that is included in the build output.
   * You can emit chunks, prebuilt chunks or assets.
   *
   * {@include ./docs/plugin-context-emitfile.md}
   *
   * @returns A `referenceId` for the emitted file that can be used in various places to reference the emitted file.
   */
  emitFile(file: EmittedFile): string;
  /**
   * Get the file name of a chunk or asset that has been emitted via
   * {@linkcode emitFile | this.emitFile}.
   *
   * @returns The file name of the emitted file. Relative to {@linkcode OutputOptions.dir | output.dir}.
   */
  getFileName(referenceId: string): string;
  /**
   * Get all module ids in the current module graph.
   *
   * @returns
   * An iterator of module ids. It can be iterated via
   * ```js
   * for (const moduleId of this.getModuleIds()) {
   *   // ...
   * }
   * ```
   * or converted into an array via `Array.from(this.getModuleIds())`.
   */
  getModuleIds(): IterableIterator<string>;
  /**
   * Get additional information about the module in question.
   *
   * {@include ./docs/plugin-context-getmoduleinfo.md}
   *
   * @returns Module information for that module. `null` if the module could not be found.
   * @group Methods
   */
  getModuleInfo: GetModuleInfo;
  /**
   * Adds additional files to be monitored in watch mode so that changes to these files will trigger rebuilds.
   *
   * {@include ./docs/plugin-context-addwatchfile.md}
   */
  addWatchFile(
    /**
     * The path to be monitored.
     *
     * This can be an absolute path to a file or directory or a path relative to the current working directory.
     */
    id: string,
  ): void;
  /**
   * Loads and parses the module corresponding to the given id, attaching additional
   * meta information to the module if provided. This will trigger the same
   * {@linkcode Plugin.load | load}, {@linkcode Plugin.transform | transform} and
   * {@linkcode Plugin.moduleParsed | moduleParsed} hooks as if the module was imported
   * by another module.
   *
   * {@include ./docs/plugin-context-load.md}
   */
  load(
    options: { id: string; resolveDependencies?: boolean } & Partial<PartialNull<ModuleOptions>>,
  ): Promise<ModuleInfo>;
  /**
   * Use Rolldown's internal parser to parse code to an [ESTree-compatible](https://github.com/estree/estree) AST.
   */
  parse(input: string, options?: ParserOptions | null): Program;
  /**
   * Resolve imports to module ids (i.e. file names) using the same plugins that Rolldown uses,
   * and determine if an import should be external.
   *
   * When calling this function from a {@linkcode Plugin.resolveId | resolveId} hook, you should
   * always check if it makes sense for you to pass along the
   * {@link PluginContextResolveOptions | options}.
   *
   * @returns
   * If `Promise<null>` is returned, the import could not be resolved by Rolldown or any plugin
   * but was not explicitly marked as external by the user.
   * If an absolute external id is returned that should remain absolute in the output either
   * via the
   * {@linkcode InputOptions.makeAbsoluteExternalsRelative | makeAbsoluteExternalsRelative}
   * option or by explicit plugin choice in the {@linkcode Plugin.resolveId | resolveId} hook,
   * `external` will be `"absolute"` instead of `true`.
   */
  resolve(
    source: string,
    importer?: string,
    options?: PluginContextResolveOptions,
  ): Promise<ResolvedId | null>;
}

export class PluginContextImpl extends MinimalPluginContextImpl {
  fs: RolldownFsModule = fsModule;
  getModuleInfo: GetModuleInfo;
  constructor(
    private outputOptions: OutputOptions,
    private context: BindingPluginContext,
    plugin: Plugin,
    private data: PluginContextData,
    private onLog: LogHandler,
    logLevel: LogLevelOption,
    watchMode: boolean,
    private currentLoadingModule?: string,
  ) {
    super(onLog, logLevel, plugin.name!, watchMode);
    this.getModuleInfo = (id: string) => this.data.getModuleInfo(id, context);
  }

  public async load(
    options: { id: string; resolveDependencies?: boolean } & Partial<PartialNull<ModuleOptions>>,
  ): Promise<ModuleInfo> {
    const id = options.id;
    if (id === this.currentLoadingModule) {
      this.onLog(LOG_LEVEL_WARN, logCycleLoading(this.pluginName, this.currentLoadingModule));
    }
    // resolveDependencies always true at rolldown
    const moduleInfo = this.data.getModuleInfo(id, this.context);
    if (moduleInfo && moduleInfo.code !== null /* module already parsed */) {
      return moduleInfo;
    }
    const rawOptions: ModuleOptions = {
      meta: options.meta || {},
      moduleSideEffects: options.moduleSideEffects || null,
      invalidate: false,
    };
    this.data.updateModuleOption(id, rawOptions);

    let loadPromise = this.data.loadModulePromiseMap.get(id);
    if (!loadPromise) {
      loadPromise = this.context
        .load(id, options.moduleSideEffects ?? undefined, options.packageJsonPath ?? undefined)
        .catch(() => {
          // avoid reusing the promise if it's an error
          // because the error may happen only in non-supported hooks (e.g. `buildStart` hook)
          this.data.loadModulePromiseMap.delete(id);
        });
      this.data.loadModulePromiseMap.set(id, loadPromise);
    }

    await loadPromise;
    return this.data.getModuleInfo(id, this.context)!;
  }

  public async resolve(
    source: string,
    importer?: string,
    options?: PluginContextResolveOptions,
  ): Promise<ResolvedId | null> {
    let receipt: number | undefined = undefined;
    if (options != null) {
      receipt = this.data.saveResolveOptions(options);
    }
    const vitePluginCustom = Object.entries(options?.custom ?? {}).reduce(
      (acc, [key, value]) => {
        if (key.startsWith('vite:')) {
          (acc ??= {})[key as keyof BindingVitePluginCustom] = value;
        }
        return acc;
      },
      undefined as BindingVitePluginCustom | undefined,
    );
    const res = await this.context.resolve(source, importer, {
      importKind: options?.kind,
      custom: receipt,
      isEntry: options?.isEntry,
      skipSelf: options?.skipSelf,
      vitePluginCustom,
    } satisfies Record<keyof BindingPluginContextResolveOptions, unknown>);
    if (receipt != null) {
      this.data.removeSavedResolveOptions(receipt);
    }

    if (res == null) return null;
    const info = this.data.getModuleOption(res.id) || ({} as ModuleOptions);
    return {
      ...res,
      external:
        res.external === 'relative'
          ? unreachable(`The PluginContext resolve result external couldn't be 'relative'`)
          : res.external,
      ...info,
      moduleSideEffects: info.moduleSideEffects ?? res.moduleSideEffects ?? null,
      packageJsonPath: res.packageJsonPath,
    };
  }

  public emitFile: PluginContext['emitFile'] = (file): string => {
    if (file.type === 'prebuilt-chunk') {
      return this.context.emitPrebuiltChunk({
        fileName: file.fileName,
        name: file.name,
        code: file.code,
        exports: file.exports,
        map: bindingifySourcemap(file.map),
        sourcemapFileName: file.sourcemapFileName,
        facadeModuleId: file.facadeModuleId,
        isEntry: file.isEntry,
        isDynamicEntry: file.isDynamicEntry,
      });
    }
    if (file.type === 'chunk') {
      return this.context.emitChunk({
        preserveEntrySignatures: bindingifyPreserveEntrySignatures(file.preserveSignature),
        ...file,
      });
    }
    const fnSanitizedFileName =
      file.fileName || typeof this.outputOptions.sanitizeFileName !== 'function'
        ? undefined
        : this.outputOptions.sanitizeFileName!(file.name || 'asset');
    const filename = file.fileName ? undefined : this.getAssetFileNames(file);
    return this.context.emitFile(
      {
        ...file,
        originalFileName: file.originalFileName || undefined,
        source: bindingAssetSource(file.source),
      },
      filename,
      fnSanitizedFileName,
    );
  };

  private getAssetFileNames(file: EmittedAsset): string | undefined {
    if (typeof this.outputOptions.assetFileNames === 'function') {
      return this.outputOptions.assetFileNames({
        type: 'asset',
        name: file.name,
        names: file.name ? [file.name] : [],
        originalFileName: file.originalFileName,
        originalFileNames: file.originalFileName ? [file.originalFileName] : [],
        source: file.source,
      });
    }
  }

  public getFileName(referenceId: string): string {
    return this.context.getFileName(referenceId);
  }

  public getModuleIds(): IterableIterator<string> {
    return this.data.getModuleIds(this.context);
  }

  public addWatchFile(id: string): void {
    this.context.addWatchFile(id);
  }

  public parse(input: string, options?: ParserOptions | null): Program {
    return parseAst(input, options);
  }
}

function _assert() {
  // adding implements to class disallows extending PluginContext by declaration merging
  // instead check that PluginContextImpl is assignable to PluginContext here
  type _ = TypeAssert<Extends<PluginContextImpl, PluginContext>>;
}
