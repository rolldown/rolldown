import type { RolldownPlugin } from '..';
import type { BindingNormalizedOptions } from '../binding.cjs';
import { lazyProp } from '../decorators/lazy';
import type { SourcemapIgnoreListOption, SourcemapPathTransformOption } from '../types/misc';
import { getLazyFields, PlainObjectLike } from '../types/plain-object-like';
import type { StringOrRegExp } from '../types/utils';
import type { BindingCell } from '../utils/threadless-free';
import type {
  AddonFunction,
  AssetFileNamesFunction,
  ChunkFileNamesFunction,
  CommentsOptions,
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
  /** @see {@linkcode OutputOptions.charset | charset} */
  charset: NonNullable<OutputOptions['charset']>;
  /** @see {@linkcode OutputOptions.entryFileNames | entryFileNames} */
  entryFileNames: string | ChunkFileNamesFunction;
  /** @see {@linkcode OutputOptions.sourcemapFileNames | sourcemapFileNames} */
  sourcemapFileNames: string | ChunkFileNamesFunction | undefined;
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
  /** @see {@linkcode OutputOptions.sourcemapExcludeSources | sourcemapExcludeSources} */
  sourcemapExcludeSources: boolean;
  /** @see {@linkcode OutputOptions.sourcemapIgnoreList | sourcemapIgnoreList} */
  sourcemapIgnoreList: boolean | SourcemapIgnoreListOption | StringOrRegExp | undefined;
  /** @see {@linkcode OutputOptions.sourcemapPathTransform | sourcemapPathTransform} */
  sourcemapPathTransform: SourcemapPathTransformOption | undefined;
  /** @see {@linkcode OutputOptions.minify | minify} */
  minify: false | MinifyOptions | 'dce-only';
  /**
   * @deprecated Use `comments.legal` instead.
   * @see {@linkcode OutputOptions.legalComments | legalComments}
   */
  legalComments: 'none' | 'inline';
  /** @see {@linkcode OutputOptions.comments | comments} */
  comments: Required<CommentsOptions>;
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

// Lazy fields whose getter reads the ORIGINAL user `outputOptions` object and
// nothing from the native box. Reading the user's config can execute user code
// (accessors, Proxies), so `materializeBoxBackedFields()` must skip these; they
// survive the box release precisely because they never touch it.
const USER_BACKED_LAZY_FIELDS: ReadonlySet<string> = new Set([
  'banner',
  'footer',
  'postBanner',
  'postFooter',
  'intro',
  'outro',
  'paths',
  'sourcemapIgnoreList',
  'sourcemapPathTransform',
]);

// Lazy fields shaped `nativeValue || userValue`: the native side is falsy
// exactly when the user supplied a function, so the public getter would reach
// the user object. `materializeBoxBackedFields()` pre-warms only their
// `boxRead` component and leaves the user fallback lazy.
const MIXED_LAZY_FIELDS: ReadonlySet<string> = new Set([
  'entryFileNames',
  'chunkFileNames',
  'assetFileNames',
  'sourcemapFileNames',
  'globals',
]);

export class NormalizedOutputOptionsImpl
  extends PlainObjectLike
  implements NormalizedOutputOptions
{
  // Caches each mixed getter's native-box component so the getter stays
  // readable after the box is released without ever re-reading the user's
  // original object from the release path.
  private readonly boxReads: Map<string, unknown>;

  // Empty after `releaseBinding()`.
  private readonly bindingCell: BindingCell<BindingNormalizedOptions>;

  constructor(
    inner: BindingNormalizedOptions,
    private outputOptions: OutputOptions,
    private normalizedOutputPlugins: RolldownPlugin[],
  ) {
    super();
    this.bindingCell = { value: inner };
    this.boxReads = new Map();
  }

  // Not `#private` members: getters run with a Proxy or derived object as
  // `this`, which fails a `#private` brand check. `materializeBoxBackedFields()`
  // caches every value read from the box before the release, so no getter
  // reaches this then.
  private binding(): BindingNormalizedOptions {
    const inner = this.bindingCell.value;
    if (inner === undefined) {
      throw new Error('These normalized output options have already released their binding object');
    }
    return inner;
  }

  private boxRead<T>(key: string, read: () => T): T {
    const cache = this.boxReads;
    if (!cache.has(key)) {
      cache.set(key, read());
    }
    return cache.get(key) as T;
  }

  /**
   * Evaluate and cache every value read from the native box, and nothing read
   * from the user's `outputOptions`, so the box can be released without
   * running user code.
   */
  materializeBoxBackedFields(): void {
    for (const field of getLazyFields(this)) {
      if (USER_BACKED_LAZY_FIELDS.has(field) || MIXED_LAZY_FIELDS.has(field)) {
        continue;
      }
      // property access is enough to evaluate and cache the lazy field
      void (this as Record<string, any>)[field];
    }
    this.boxRead('entryFilenames', () => this.binding().entryFilenames);
    this.boxRead('chunkFilenames', () => this.binding().chunkFilenames);
    this.boxRead('assetFilenames', () => this.binding().assetFilenames);
    this.boxRead('sourcemapFilenames', () => this.binding().sourcemapFilenames);
    this.boxRead('globals', () => this.binding().globals);
  }

  /**
   * Drop the binding wrapper once its box is released: the wrapper's methods
   * keep the binding instance that made it alive, so options a plugin keeps
   * would keep that instance's memory too.
   */
  releaseBinding(): void {
    this.bindingCell.value = undefined;
  }

  @lazyProp
  get dir(): string | undefined {
    return this.binding().dir ?? undefined;
  }

  @lazyProp
  get charset(): NonNullable<OutputOptions['charset']> {
    return this.binding().charset;
  }

  @lazyProp
  get entryFileNames(): string | ChunkFileNamesFunction {
    return (
      this.boxRead('entryFilenames', () => this.binding().entryFilenames) ||
      this.outputOptions.entryFileNames!
    );
  }

  @lazyProp
  get chunkFileNames(): string | ChunkFileNamesFunction {
    return (
      this.boxRead('chunkFilenames', () => this.binding().chunkFilenames) ||
      this.outputOptions.chunkFileNames!
    );
  }

  @lazyProp
  get assetFileNames(): string | AssetFileNamesFunction {
    return (
      this.boxRead('assetFilenames', () => this.binding().assetFilenames) ||
      this.outputOptions.assetFileNames!
    );
  }

  @lazyProp
  get format(): 'es' | 'cjs' | 'iife' | 'umd' {
    return this.binding().format;
  }

  @lazyProp
  get exports(): 'default' | 'named' | 'none' | 'auto' {
    return this.binding().exports;
  }

  @lazyProp
  get sourcemap(): boolean | 'inline' | 'hidden' {
    return this.binding().sourcemap;
  }

  @lazyProp
  get sourcemapFileNames(): string | ChunkFileNamesFunction | undefined {
    return (
      this.boxRead('sourcemapFilenames', () => this.binding().sourcemapFilenames) ||
      this.outputOptions.sourcemapFileNames
    );
  }
  @lazyProp
  get sourcemapBaseUrl(): string | undefined {
    return this.binding().sourcemapBaseUrl ?? undefined;
  }

  @lazyProp
  get shimMissingExports(): boolean {
    return this.binding().shimMissingExports;
  }

  @lazyProp
  get name(): string | undefined {
    return this.binding().name ?? undefined;
  }

  @lazyProp
  get file(): string | undefined {
    return this.binding().file ?? undefined;
  }

  @lazyProp
  get codeSplitting(): boolean {
    return this.binding().codeSplitting;
  }

  /**
   * @deprecated Use `codeSplitting` instead.
   */
  @lazyProp
  get inlineDynamicImports(): boolean {
    return !this.binding().codeSplitting;
  }

  @lazyProp
  get dynamicImportInCjs(): boolean {
    return this.binding().dynamicImportInCjs;
  }

  @lazyProp
  get externalLiveBindings(): boolean {
    return this.binding().externalLiveBindings;
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
    return this.binding().esModule;
  }

  @lazyProp
  get extend(): boolean {
    return this.binding().extend;
  }

  @lazyProp
  get globals(): Record<string, string> | GlobalsFunction {
    return this.boxRead('globals', () => this.binding().globals) || this.outputOptions.globals!;
  }

  @lazyProp
  get paths(): Record<string, string> | PathsFunction | undefined {
    return this.outputOptions.paths;
  }

  @lazyProp
  get hashCharacters(): 'base64' | 'base36' | 'hex' {
    return this.binding().hashCharacters;
  }

  @lazyProp
  get sourcemapDebugIds(): boolean {
    return this.binding().sourcemapDebugIds;
  }

  @lazyProp
  get sourcemapExcludeSources(): boolean {
    return this.binding().sourcemapExcludeSources;
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
    let ret = this.binding().minify;
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
    return this.binding().legalComments;
  }

  @lazyProp
  get comments(): Required<CommentsOptions> {
    const c = this.binding().comments;
    return {
      legal: c.legal ?? true,
      annotation: c.annotation ?? true,
      jsdoc: c.jsdoc ?? true,
    };
  }

  @lazyProp
  get polyfillRequire(): boolean {
    return this.binding().polyfillRequire;
  }

  @lazyProp
  get plugins(): RolldownPlugin[] {
    return this.normalizedOutputPlugins;
  }

  @lazyProp
  get preserveModules(): boolean {
    return this.binding().preserveModules;
  }

  @lazyProp
  get preserveModulesRoot(): string | undefined {
    return this.binding().preserveModulesRoot;
  }

  @lazyProp
  get virtualDirname(): string {
    return this.binding().virtualDirname;
  }

  @lazyProp
  get topLevelVar(): boolean {
    return this.binding().topLevelVar ?? false;
  }

  @lazyProp
  get minifyInternalExports(): boolean {
    return this.binding().minifyInternalExports ?? false;
  }
}

function normalizeAddon(value?: string | AddonFunction) {
  if (typeof value === 'function') {
    return value;
  }
  return () => value || '';
}
