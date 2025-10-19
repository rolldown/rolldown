import type { RolldownPlugin } from '..';
import type { BindingNormalizedOptions } from '../binding';
import type {
  SourcemapIgnoreListOption,
  SourcemapPathTransformOption,
} from '../types/misc';
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

// TODO: I guess we make these getters enumerable so it act more like a plain object
export class NormalizedOutputOptionsImpl implements NormalizedOutputOptions {
  constructor(
    private inner: BindingNormalizedOptions,
    private outputOptions: OutputOptions,
    private normalizedOutputPlugins: RolldownPlugin[],
  ) {}

  get dir(): string | undefined {
    return this.inner.dir ?? undefined;
  }

  get entryFileNames(): string | ChunkFileNamesFunction {
    return this.inner.entryFilenames || this.outputOptions.entryFileNames!;
  }

  get chunkFileNames(): string | ChunkFileNamesFunction {
    return this.inner.chunkFilenames || this.outputOptions.chunkFileNames!;
  }

  get assetFileNames(): string | AssetFileNamesFunction {
    return this.inner.assetFilenames || this.outputOptions.assetFileNames!;
  }

  get format(): 'es' | 'cjs' | 'iife' | 'umd' {
    return this.inner.format;
  }

  get exports(): 'default' | 'named' | 'none' | 'auto' {
    return this.inner.exports;
  }

  get sourcemap(): boolean | 'inline' | 'hidden' {
    return this.inner.sourcemap;
  }

  get sourcemapBaseUrl(): string | undefined {
    return this.inner.sourcemapBaseUrl ?? undefined;
  }

  get cssEntryFileNames(): string | ChunkFileNamesFunction {
    return this.inner.cssEntryFilenames ||
      this.outputOptions.cssEntryFileNames!;
  }

  get cssChunkFileNames(): string | ChunkFileNamesFunction {
    return this.inner.cssChunkFilenames ||
      this.outputOptions.cssChunkFileNames!;
  }

  get shimMissingExports(): boolean {
    return this.inner.shimMissingExports;
  }

  get name(): string | undefined {
    return this.inner.name ?? undefined;
  }

  get file(): string | undefined {
    return this.inner.file ?? undefined;
  }

  get inlineDynamicImports(): boolean {
    return this.inner.inlineDynamicImports;
  }

  get externalLiveBindings(): boolean {
    return this.inner.externalLiveBindings;
  }

  get banner(): AddonFunction {
    return normalizeAddon(this.outputOptions.banner);
  }

  get footer(): AddonFunction {
    return normalizeAddon(this.outputOptions.footer);
  }

  get intro(): AddonFunction {
    return normalizeAddon(this.outputOptions.intro);
  }

  get outro(): AddonFunction {
    return normalizeAddon(this.outputOptions.outro);
  }

  get esModule(): boolean | 'if-default-prop' {
    return this.inner.esModule;
  }

  get extend(): boolean {
    return this.inner.extend;
  }

  get globals(): Record<string, string> | GlobalsFunction {
    return this.inner.globals || this.outputOptions.globals!;
  }

  get paths(): Record<string, string> | PathsFunction | undefined {
    return this.outputOptions.paths;
  }

  get hashCharacters(): 'base64' | 'base36' | 'hex' {
    return this.inner.hashCharacters;
  }

  get sourcemapDebugIds(): boolean {
    return this.inner.sourcemapDebugIds;
  }

  get sourcemapIgnoreList():
    | boolean
    | SourcemapIgnoreListOption
    | StringOrRegExp
    | undefined
  {
    return this.outputOptions.sourcemapIgnoreList;
  }

  get sourcemapPathTransform(): SourcemapPathTransformOption | undefined {
    return this.outputOptions.sourcemapPathTransform;
  }

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

  get legalComments(): 'none' | 'inline' {
    return this.inner.legalComments;
  }

  get polyfillRequire(): boolean {
    return this.inner.polyfillRequire;
  }

  get plugins(): RolldownPlugin[] {
    return this.normalizedOutputPlugins;
  }

  get preserveModules(): boolean {
    return this.inner.preserveModules;
  }

  get preserveModulesRoot(): string | undefined {
    return this.inner.preserveModulesRoot;
  }

  get virtualDirname(): string {
    return this.inner.virtualDirname;
  }

  get topLevelVar(): boolean {
    return this.inner.topLevelVar ?? false;
  }

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
