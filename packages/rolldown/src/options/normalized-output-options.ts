import type { RolldownPlugin } from '..';
import type { BindingNormalizedOptions } from '../binding.cjs';
import { lazyProp } from '../decorators/lazy';
import type { SourcemapIgnoreListOption, SourcemapPathTransformOption } from '../types/misc';
import { PlainObjectLike } from '../types/plain-object-like';
import type { StringOrRegExp } from '../types/utils';
import type {
  AddonFunction,
  AssetFileNamesFunction,
  ChunkFileNamesFunction,
  GlobalsFunction,
  MinifyOptions,
  OutputOptions,
} from './output-options';
// oxlint-disable-next-line no-unused-vars -- this is used in JSDoc links
import type { ModuleFormat } from './output-options';

type PathsFunction = (id: string) => string;

/**
 * A normalized version of {@linkcode ModuleFormat}.
 * @category Plugin APIs
 */
export type InternalModuleFormat = 'es' | 'cjs' | 'iife' | 'umd';

/** @category Plugin APIs */
export interface NormalizedOutputOptions {
  /** @see {@linkcode OutputOptions.name | name} */
  name: string | undefined;
  /** @see {@linkcode OutputOptions.file | file} */
  file: string | undefined;
  /** @see {@linkcode OutputOptions.dir | dir} */
  dir: string | undefined;
  /** @see {@linkcode OutputOptions.entryFileNames | entryFileNames} */
  entryFileNames: string | ChunkFileNamesFunction;
  /** @see {@linkcode OutputOptions.chunkFileNames | chunkFileNames} */
  chunkFileNames: string | ChunkFileNamesFunction;
  /** @see {@linkcode OutputOptions.assetFileNames | assetFileNames} */
  assetFileNames: string | AssetFileNamesFunction;
  /** @see {@linkcode OutputOptions.format | format} */
  format: InternalModuleFormat;
  /** @see {@linkcode OutputOptions.exports | exports} */
  exports: NonNullable<OutputOptions['exports']>;
  /** @see {@linkcode OutputOptions.sourcemap | sourcemap} */
  sourcemap: boolean | 'inline' | 'hidden';
  /** @see {@linkcode OutputOptions.sourcemapBaseUrl | sourcemapBaseUrl} */
  sourcemapBaseUrl: string | undefined;
  /** @see {@linkcode OutputOptions.cssEntryFileNames | cssEntryFileNames} */
  cssEntryFileNames: string | ChunkFileNamesFunction;
  /** @see {@linkcode OutputOptions.cssChunkFileNames | cssChunkFileNames} */
  cssChunkFileNames: string | ChunkFileNamesFunction;
  /** @see {@linkcode OutputOptions.codeSplitting | codeSplitting} */
  codeSplitting: boolean;
  /** @deprecated Use `codeSplitting` instead. */
  inlineDynamicImports: boolean;
  /** @see {@linkcode OutputOptions.dynamicImportInCjs | dynamicImportInCjs} */
  dynamicImportInCjs: boolean;
  /** @see {@linkcode OutputOptions.externalLiveBindings | externalLiveBindings} */
  externalLiveBindings: boolean;
  /** @see {@linkcode OutputOptions.banner | banner} */
  banner: AddonFunction;
  /** @see {@linkcode OutputOptions.footer | footer} */
  footer: AddonFunction;
  /** @see {@linkcode OutputOptions.postBanner | postBanner} */
  postBanner: AddonFunction;
  /** @see {@linkcode OutputOptions.postFooter | postFooter} */
  postFooter: AddonFunction;
  /** @see {@linkcode OutputOptions.intro | intro} */
  intro: AddonFunction;
  /** @see {@linkcode OutputOptions.outro | outro} */
  outro: AddonFunction;
  /** @see {@linkcode OutputOptions.esModule | esModule} */
  esModule: boolean | 'if-default-prop';
  /** @see {@linkcode OutputOptions.extend | extend} */
  extend: boolean;
  /** @see {@linkcode OutputOptions.globals | globals} */
  globals: Record<string, string> | GlobalsFunction;
  /** @see {@linkcode OutputOptions.paths | paths} */
  paths: Record<string, string> | PathsFunction | undefined;
  /** @see {@linkcode OutputOptions.hashCharacters | hashCharacters} */
  hashCharacters: 'base64' | 'base36' | 'hex';
  /** @see {@linkcode OutputOptions.sourcemapDebugIds | sourcemapDebugIds} */
  sourcemapDebugIds: boolean;
  /** @see {@linkcode OutputOptions.sourcemapIgnoreList | sourcemapIgnoreList} */
  sourcemapIgnoreList: boolean | SourcemapIgnoreListOption | StringOrRegExp | undefined;
  /** @see {@linkcode OutputOptions.sourcemapPathTransform | sourcemapPathTransform} */
  sourcemapPathTransform: SourcemapPathTransformOption | undefined;
  /** @see {@linkcode OutputOptions.minify | minify} */
  minify: false | MinifyOptions | 'dce-only';
  /** @see {@linkcode OutputOptions.legalComments | legalComments} */
  legalComments: 'none' | 'inline';
  /** @see {@linkcode OutputOptions.polyfillRequire | polyfillRequire} */
  polyfillRequire: boolean;
  /** @see {@linkcode OutputOptions.plugins | plugins} */
  plugins: RolldownPlugin[];
  /** @see {@linkcode OutputOptions.preserveModules | preserveModules} */
  preserveModules: boolean;
  /** @see {@linkcode OutputOptions.virtualDirname | virtualDirname} */
  virtualDirname: string;
  /** @see {@linkcode OutputOptions.preserveModulesRoot | preserveModulesRoot} */
  preserveModulesRoot?: string;
  /** @see {@linkcode OutputOptions.topLevelVar | topLevelVar} */
  topLevelVar?: boolean;
  /** @see {@linkcode OutputOptions.minifyInternalExports | minifyInternalExports} */
  minifyInternalExports?: boolean;
}

export class NormalizedOutputOptionsImpl
  extends PlainObjectLike
  implements NormalizedOutputOptions
{
  constructor(
    private inner: BindingNormalizedOptions,
    private outputOptions: OutputOptions,
    private normalizedOutputPlugins: RolldownPlugin[],
  ) {
    super();
  }

  @lazyProp
  get dir(): string | undefined {
    return this.inner.dir ?? undefined;
  }

  @lazyProp
  get entryFileNames(): string | ChunkFileNamesFunction {
    return this.inner.entryFilenames || this.outputOptions.entryFileNames!;
  }

  @lazyProp
  get chunkFileNames(): string | ChunkFileNamesFunction {
    return this.inner.chunkFilenames || this.outputOptions.chunkFileNames!;
  }

  @lazyProp
  get assetFileNames(): string | AssetFileNamesFunction {
    return this.inner.assetFilenames || this.outputOptions.assetFileNames!;
  }

  @lazyProp
  get format(): 'es' | 'cjs' | 'iife' | 'umd' {
    return this.inner.format;
  }

  @lazyProp
  get exports(): 'default' | 'named' | 'none' | 'auto' {
    return this.inner.exports;
  }

  @lazyProp
  get sourcemap(): boolean | 'inline' | 'hidden' {
    return this.inner.sourcemap;
  }

  @lazyProp
  get sourcemapBaseUrl(): string | undefined {
    return this.inner.sourcemapBaseUrl ?? undefined;
  }

  @lazyProp
  get cssEntryFileNames(): string | ChunkFileNamesFunction {
    return this.inner.cssEntryFilenames || this.outputOptions.cssEntryFileNames!;
  }

  @lazyProp
  get cssChunkFileNames(): string | ChunkFileNamesFunction {
    return this.inner.cssChunkFilenames || this.outputOptions.cssChunkFileNames!;
  }

  @lazyProp
  get shimMissingExports(): boolean {
    return this.inner.shimMissingExports;
  }

  @lazyProp
  get name(): string | undefined {
    return this.inner.name ?? undefined;
  }

  @lazyProp
  get file(): string | undefined {
    return this.inner.file ?? undefined;
  }

  @lazyProp
  get codeSplitting(): boolean {
    return this.inner.codeSplitting;
  }

  /**
   * @deprecated Use `codeSplitting` instead.
   */
  @lazyProp
  get inlineDynamicImports(): boolean {
    return !this.inner.codeSplitting;
  }

  @lazyProp
  get dynamicImportInCjs(): boolean {
    return this.inner.dynamicImportInCjs;
  }

  @lazyProp
  get externalLiveBindings(): boolean {
    return this.inner.externalLiveBindings;
  }

  @lazyProp
  get banner(): AddonFunction {
    return normalizeAddon(this.outputOptions.banner);
  }

  @lazyProp
  get footer(): AddonFunction {
    return normalizeAddon(this.outputOptions.footer);
  }

  @lazyProp
  get postBanner(): AddonFunction {
    return normalizeAddon(this.outputOptions.postBanner);
  }

  @lazyProp
  get postFooter(): AddonFunction {
    return normalizeAddon(this.outputOptions.postFooter);
  }

  @lazyProp
  get intro(): AddonFunction {
    return normalizeAddon(this.outputOptions.intro);
  }

  @lazyProp
  get outro(): AddonFunction {
    return normalizeAddon(this.outputOptions.outro);
  }

  @lazyProp
  get esModule(): boolean | 'if-default-prop' {
    return this.inner.esModule;
  }

  @lazyProp
  get extend(): boolean {
    return this.inner.extend;
  }

  @lazyProp
  get globals(): Record<string, string> | GlobalsFunction {
    return this.inner.globals || this.outputOptions.globals!;
  }

  @lazyProp
  get paths(): Record<string, string> | PathsFunction | undefined {
    return this.outputOptions.paths;
  }

  @lazyProp
  get hashCharacters(): 'base64' | 'base36' | 'hex' {
    return this.inner.hashCharacters;
  }

  @lazyProp
  get sourcemapDebugIds(): boolean {
    return this.inner.sourcemapDebugIds;
  }

  @lazyProp
  get sourcemapIgnoreList(): boolean | SourcemapIgnoreListOption | StringOrRegExp | undefined {
    return this.outputOptions.sourcemapIgnoreList;
  }

  @lazyProp
  get sourcemapPathTransform(): SourcemapPathTransformOption | undefined {
    return this.outputOptions.sourcemapPathTransform;
  }

  @lazyProp
  get minify(): false | MinifyOptions | 'dce-only' {
    let ret = this.inner.minify;
    if (typeof ret === 'object' && ret !== null) {
      // Omit some properties that are not needed in the output
      delete ret['codegen'];
      delete ret['module'];
      delete ret['sourcemap'];
    }
    return ret;
  }

  @lazyProp
  get legalComments(): 'none' | 'inline' {
    return this.inner.legalComments;
  }

  @lazyProp
  get polyfillRequire(): boolean {
    return this.inner.polyfillRequire;
  }

  @lazyProp
  get plugins(): RolldownPlugin[] {
    return this.normalizedOutputPlugins;
  }

  @lazyProp
  get preserveModules(): boolean {
    return this.inner.preserveModules;
  }

  @lazyProp
  get preserveModulesRoot(): string | undefined {
    return this.inner.preserveModulesRoot;
  }

  @lazyProp
  get virtualDirname(): string {
    return this.inner.virtualDirname;
  }

  @lazyProp
  get topLevelVar(): boolean {
    return this.inner.topLevelVar ?? false;
  }

  @lazyProp
  get minifyInternalExports(): boolean {
    return this.inner.minifyInternalExports ?? false;
  }
}

function normalizeAddon(value?: string | AddonFunction) {
  if (typeof value === 'function') {
    return value;
  }
  return () => value || '';
}
