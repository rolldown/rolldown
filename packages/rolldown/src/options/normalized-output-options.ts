import { RolldownPlugin } from '..';
import type {
  BindingMinifyOptions,
  BindingNormalizedOptions,
} from '../binding';
import type {
  SourcemapIgnoreListOption,
  SourcemapPathTransformOption,
} from '../types/misc';
import { bindingifySourcemapIgnoreList } from '../utils/bindingify-output-options';
import type {
  AddonFunction,
  AssetFileNamesFunction,
  ChunkFileNamesFunction,
  GlobalsFunction,
  OutputOptions,
} from './output-options';

export type InternalModuleFormat = 'es' | 'cjs' | 'iife' | 'umd' | 'app';

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
  hashCharacters: 'base64' | 'base36' | 'hex';
  sourcemapDebugIds: boolean;
  sourcemapIgnoreList: SourcemapIgnoreListOption;
  sourcemapPathTransform: SourcemapPathTransformOption | undefined;
  minify: false | BindingMinifyOptions;
  comments: 'none' | 'preserve-legal';
  polyfillRequire: boolean;
  plugins: RolldownPlugin[];
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

  get format(): 'es' | 'cjs' | 'app' | 'iife' | 'umd' {
    return this.inner.format;
  }

  get exports(): 'default' | 'named' | 'none' | 'auto' {
    return this.inner.exports;
  }

  get sourcemap(): boolean | 'inline' | 'hidden' {
    return this.inner.sourcemap;
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

  get hashCharacters(): 'base64' | 'base36' | 'hex' {
    return this.inner.hashCharacters;
  }

  get sourcemapDebugIds(): boolean {
    return this.inner.sourcemapDebugIds;
  }

  get sourcemapIgnoreList(): SourcemapIgnoreListOption {
    return bindingifySourcemapIgnoreList(
      this.outputOptions.sourcemapIgnoreList,
    );
  }

  get sourcemapPathTransform(): SourcemapPathTransformOption | undefined {
    return this.outputOptions.sourcemapPathTransform;
  }

  get minify(): false | BindingMinifyOptions {
    return this.inner.minify;
  }

  get comments(): 'none' | 'preserve-legal' {
    return this.inner.comments;
  }

  get polyfillRequire(): boolean {
    return this.inner.polyfillRequire;
  }

  get plugins(): RolldownPlugin[] {
    return this.normalizedOutputPlugins;
  }
}

function normalizeAddon(value?: string | AddonFunction) {
  if (typeof value === 'function') {
    return value;
  }
  return () => value || '';
}
