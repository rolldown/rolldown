import type { RolldownPlugin } from '..';
import type { BindingNormalizedOptions } from '../binding.cjs';
import { lazyProp } from '../decorators/lazy';
import type {
  SourcemapIgnoreListOption,
  SourcemapPathTransformOption,
} from '../types/misc';
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

type PathsFunction = (id: string) => string;

export type InternalModuleFormat = 'es' | 'cjs' | 'iife' | 'umd';

export interface NormalizedOutputOptions {
  name: string | undefined;
  file: string | undefined;
  dir: string | undefined;
  entryFileNames: string | ChunkFileNamesFunction;
  chunkFileNames: string | ChunkFileNamesFunction;
  assetFileNames: string | AssetFileNamesFunction;
  format: InternalModuleFormat;
  exports: NonNullable<OutputOptions['exports']>;
  sourcemap: boolean | 'inline' | 'hidden';
  sourcemapBaseUrl: string | undefined;
  cssEntryFileNames: string | ChunkFileNamesFunction;
  cssChunkFileNames: string | ChunkFileNamesFunction;
  inlineDynamicImports: boolean;
  dynamicImportInCjs: boolean;
  externalLiveBindings: boolean;
  banner: AddonFunction;
  footer: AddonFunction;
  intro: AddonFunction;
  outro: AddonFunction;
  esModule: boolean | 'if-default-prop';
  extend: boolean;
  globals: Record<string, string> | GlobalsFunction;
  paths: Record<string, string> | PathsFunction | undefined;
  hashCharacters: 'base64' | 'base36' | 'hex';
  sourcemapDebugIds: boolean;
  sourcemapIgnoreList:
    | boolean
    | SourcemapIgnoreListOption
    | StringOrRegExp
    | undefined;
  sourcemapPathTransform: SourcemapPathTransformOption | undefined;
  minify: false | MinifyOptions | 'dce-only';
  legalComments: 'none' | 'inline';
  polyfillRequire: boolean;
  plugins: RolldownPlugin[];
  preserveModules: boolean;
  virtualDirname: string;
  preserveModulesRoot?: string;
  topLevelVar?: boolean;
  minifyInternalExports?: boolean;
}

export class NormalizedOutputOptionsImpl extends PlainObjectLike
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
    return this.inner.cssEntryFilenames ||
      this.outputOptions.cssEntryFileNames!;
  }

  @lazyProp
  get cssChunkFileNames(): string | ChunkFileNamesFunction {
    return this.inner.cssChunkFilenames ||
      this.outputOptions.cssChunkFileNames!;
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
  get inlineDynamicImports(): boolean {
    return this.inner.inlineDynamicImports;
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
  get sourcemapIgnoreList():
    | boolean
    | SourcemapIgnoreListOption
    | StringOrRegExp
    | undefined
  {
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
