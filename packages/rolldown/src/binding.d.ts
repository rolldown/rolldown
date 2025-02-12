type MaybePromise<T> = T | Promise<T>
type Nullable<T> = T | null | undefined
type VoidNullable<T = void> = T | null | undefined | void
export type BindingStringOrRegex = string | RegExp

export declare class BindingBundleEndEventData {
  output: string
  duration: number
}

export declare class BindingCallableBuiltinPlugin {
  constructor(plugin: BindingBuiltinPlugin)
  resolveId(id: string, importer?: string | undefined | null, options?: BindingHookJsResolveIdOptions | undefined | null): Promise<BindingHookJsResolveIdOutput | null>
  load(id: string): Promise<BindingHookJsLoadOutput | null>
  watchChange(path: string, event: BindingJsWatchChangeEvent): Promise<void>
}

export declare class BindingError {
  kind: string
  message: string
}

export declare class BindingLog {
  code: string
  message: string
}

export declare class BindingModuleInfo {
  id: string
  importers: Array<string>
  dynamicImporters: Array<string>
  importedIds: Array<string>
  dynamicallyImportedIds: Array<string>
  exports: Array<string>
  isEntry: boolean
  get code(): string | null
}

export declare class BindingNormalizedOptions {
  get input(): Array<string> | Record<string, string>
  get cwd(): string | null
  get platform(): 'node' | 'browser' | 'neutral'
  get shimMissingExports(): boolean
  get name(): string | null
  get cssEntryFilenames(): string | undefined
  get cssChunkFilenames(): string | undefined
  get entryFilenames(): string | undefined
  get chunkFilenames(): string | undefined
  get assetFilenames(): string | undefined
  get dir(): string | null
  get file(): string | null
  get format(): 'es' | 'cjs' | 'app' | 'iife' | 'umd'
  get exports(): 'default' | 'named' | 'none' | 'auto'
  get esModule(): boolean | 'if-default-prop'
  get inlineDynamicImports(): boolean
  get sourcemap(): boolean | 'inline' | 'hidden'
  get banner(): string | undefined | null | undefined
  get footer(): string | undefined | null | undefined
  get intro(): string | undefined | null | undefined
  get outro(): string | undefined | null | undefined
  get externalLiveBindings(): boolean
  get extend(): boolean
  get globals(): Record<string, string> | undefined
  get hashCharacters(): 'base64' | 'base36' | 'hex'
  get sourcemapDebugIds(): boolean
  get minify(): false | BindingMinifyOptions
  get polyfillRequire(): boolean
  get comments(): 'none' | 'preserve-legal'
}

export declare class BindingOutputAsset {
  get fileName(): string
  get originalFileName(): string | null
  get originalFileNames(): Array<string>
  get source(): BindingAssetSource
  get name(): string | null
  get names(): Array<string>
}

export declare class BindingOutputChunk {
  get isEntry(): boolean
  get isDynamicEntry(): boolean
  get facadeModuleId(): string | null
  get moduleIds(): Array<string>
  get exports(): Array<string>
  get fileName(): string
  get modules(): BindingModules
  get imports(): Array<string>
  get dynamicImports(): Array<string>
  get code(): string
  get map(): string | null
  get sourcemapFileName(): string | null
  get preliminaryFileName(): string
  get name(): string
}

export declare class BindingOutputs {
  get chunks(): Array<BindingOutputChunk>
  get assets(): Array<BindingOutputAsset>
  get errors(): Array<Error | BindingError>
}

export declare class BindingPluginContext {
  load(specifier: string, sideEffects: BindingHookSideEffects | undefined, fn: () => void): Promise<void>
  resolve(specifier: string, importer?: string | undefined | null, extraOptions?: BindingPluginContextResolveOptions | undefined | null): Promise<BindingPluginContextResolvedId | null>
  emitFile(file: BindingEmittedAsset, assetFilename?: string | undefined | null, fnSanitizedFileName?: string | undefined | null): string
  emitChunk(file: BindingEmittedChunk): string
  getFileName(referenceId: string): string
  getModuleInfo(moduleId: string): BindingModuleInfo | null
  getModuleIds(): Array<string>
  addWatchFile(file: string): void
}

export declare class BindingRenderedModule {
  get code(): string | null
}

export declare class BindingTransformPluginContext {
  getCombinedSourcemap(): string
  inner(): BindingPluginContext
}

export declare class BindingWatcher {
  constructor(options: Array<BindingBundlerOptions>, notifyOption?: BindingNotifyOption | undefined | null)
  close(): Promise<void>
  start(listener: (data: BindingWatcherEvent) => void): Promise<void>
}

export declare class BindingWatcherChangeData {
  path: string
  kind: string
}

export declare class BindingWatcherEvent {
  eventKind(): string
  watchChangeData(): BindingWatcherChangeData
  bundleEndData(): BindingBundleEndEventData
  bundleEventKind(): string
  errors(): Array<Error | BindingError>
}

export declare class Bundler {
  constructor(option: BindingBundlerOptions)
  write(): Promise<BindingOutputs>
  generate(): Promise<BindingOutputs>
  scan(): Promise<BindingOutputs>
  close(): Promise<void>
  get closed(): boolean
}

export declare class MagicString {
  /** Get source text from utf8 offset. */
  getSourceText(start: number, end: number): string
  /** Get 0-based line and column number from utf8 offset. */
  getLineColumnNumber(offset: number): LineColumn
  /** Get UTF16 byte offset from UTF8 byte offset. */
  getUtf16ByteOffset(offset: number): number
  length(): number
  toString(): string
  hasChanged(): boolean
  append(input: string): this
  appendLeft(index: number, input: string): this
  appendRight(index: number, input: string): this
  indent(): this
  prepend(input: string): this
  prependLeft(index: number, input: string): this
  prependRight(index: number, input: string): this
  relocate(start: number, end: number, to: number): this
  remove(start: number, end: number): this
  generateMap(options?: Partial<GenerateDecodedMapOptions>): {
    toString: () => string;
    toUrl: () => string;
    toMap: () => {
      file?: string
      mappings: string
      names: Array<string>
      sourceRoot?: string
      sources: Array<string>
      sourcesContent?: Array<string>
      version: number
      x_google_ignoreList?: Array<number>
    }
  }
}

export declare class ParallelJsPluginRegistry {
  id: number
  workerCount: number
  constructor(workerCount: number)
}

export declare class ParseResult {
  get program(): import("@oxc-project/types").Program
  get module(): EcmaScriptModule
  get comments(): Array<Comment>
  get errors(): Array<OxcError>
  get magicString(): MagicString
}

export declare class RenderedChunk {
  get name(): string
  get isEntry(): boolean
  get isDynamicEntry(): boolean
  get facadeModuleId(): string | null
  get moduleIds(): Array<string>
  get exports(): Array<string>
  get fileName(): string
  get modules(): BindingModules
  get imports(): Array<string>
  get dynamicImports(): Array<string>
}

export interface AliasItem {
  find: string
  replacements: Array<string>
}

export interface ArrowFunctionsOptions {
  /**
   * This option enables the following:
   * * Wrap the generated function in .bind(this) and keeps uses of this inside the function as-is, instead of using a renamed this.
   * * Add a runtime check to ensure the functions are not instantiated.
   * * Add names to arrow functions.
   *
   * @default false
   */
  spec?: boolean
}

export interface BindingAdvancedChunksOptions {
  minSize?: number
  minShareCount?: number
  groups?: Array<BindingMatchGroup>
  maxSize?: number
  minModuleSize?: number
  maxModuleSize?: number
}

export interface BindingAliasPluginAlias {
  find: BindingStringOrRegex
  replacement: string
}

export interface BindingAliasPluginConfig {
  entries: Array<BindingAliasPluginAlias>
}

export interface BindingAssetSource {
  inner: string | Uint8Array
}

export interface BindingBuildImportAnalysisPluginConfig {
  preloadCode: string
  insertPreload: boolean
  optimizeModulePreloadRelativePaths: boolean
  renderBuiltUrl: boolean
  isRelativeBase: boolean
}

export interface BindingBuiltinPlugin {
  __name: BindingBuiltinPluginName
  options?: unknown
}

export type BindingBuiltinPluginName =  'builtin:wasm-helper'|
'builtin:import-glob'|
'builtin:dynamic-import-vars'|
'builtin:module-preload-polyfill'|
'builtin:manifest'|
'builtin:load-fallback'|
'builtin:transform'|
'builtin:wasm-fallback'|
'builtin:alias'|
'builtin:json'|
'builtin:build-import-analysis'|
'builtin:replace'|
'builtin:vite-resolve'|
'builtin:module-federation';

export interface BindingBundlerOptions {
  inputOptions: BindingInputOptions
  outputOptions: BindingOutputOptions
  parallelPluginsRegistry?: ParallelJsPluginRegistry
}

export interface BindingChecksOptions {
  circularDependency?: boolean
}

export interface BindingDeferSyncScanData {
  /** ModuleId */
  id: string
  sideEffects?: BindingHookSideEffects
}

export interface BindingEmittedAsset {
  name?: string
  fileName?: string
  originalFileName?: string
  source: BindingAssetSource
}

export interface BindingEmittedChunk {
  name?: string
  fileName?: string
  id: string
  importer?: string
}

export interface BindingExperimentalOptions {
  strictExecutionOrder?: boolean
  disableLiveBindings?: boolean
  viteMode?: boolean
  resolveNewUrlToAsset?: boolean
  hmr?: boolean
}

export interface BindingGeneralHookFilter {
  include?: Array<BindingStringOrRegex>
  exclude?: Array<BindingStringOrRegex>
}

export interface BindingGlobImportPluginConfig {
  root?: string
  restoreQueryExtension?: boolean
}

export interface BindingHookJsLoadOutput {
  code: string
  map?: string
  sideEffects: boolean | 'no-treeshake'
}

export interface BindingHookJsResolveIdOptions {
  scan?: boolean
}

export interface BindingHookJsResolveIdOutput {
  id: string
  external?: boolean
  sideEffects: boolean | 'no-treeshake'
}

export interface BindingHookLoadOutput {
  code: string
  sideEffects?: BindingHookSideEffects
  map?: BindingSourcemap
  moduleType?: string
}

export interface BindingHookRenderChunkOutput {
  code: string
  map?: BindingSourcemap
}

export interface BindingHookResolveIdExtraArgs {
  custom?: number
  isEntry: boolean
  kind: 'import' | 'dynamic-import' | 'require-call'
}

export interface BindingHookResolveIdOutput {
  id: string
  external?: boolean
  sideEffects?: BindingHookSideEffects
}

export declare enum BindingHookSideEffects {
  True = 0,
  False = 1,
  NoTreeshake = 2
}

export interface BindingHookTransformOutput {
  code?: string
  sideEffects?: BindingHookSideEffects
  map?: BindingSourcemap
  moduleType?: string
}

export interface BindingInjectImportNamed {
  tagNamed: true
  imported: string
  alias?: string
  from: string
}

export interface BindingInjectImportNamespace {
  tagNamespace: true
  alias: string
  from: string
}

export interface BindingInputItem {
  name?: string
  import: string
}

export interface BindingInputOptions {
  external?: undefined | ((source: string, importer: string | undefined, isResolved: boolean) => boolean)
  input: Array<BindingInputItem>
  plugins: (BindingBuiltinPlugin | BindingPluginOptions | undefined)[]
  resolve?: BindingResolveOptions
  shimMissingExports?: boolean
  platform?: 'node' | 'browser' | 'neutral'
  logLevel: BindingLogLevel
  onLog: (logLevel: 'debug' | 'warn' | 'info', log: BindingLog) => void
  cwd: string
  treeshake?: BindingTreeshake
  moduleTypes?: Record<string, string>
  define?: Array<[string, string]>
  dropLabels?: Array<string>
  inject?: Array<BindingInjectImportNamed | BindingInjectImportNamespace>
  experimental?: BindingExperimentalOptions
  profilerNames?: boolean
  jsx?: BindingJsx
  watch?: BindingWatchOption
  keepNames?: boolean
  checks?: BindingChecksOptions
  deferSyncScanData?: undefined | (() => BindingDeferSyncScanData[])
}

export interface BindingJsonPluginConfig {
  stringify?: BindingJsonPluginStringify
  isBuild?: boolean
  namedExports?: boolean
}

export type BindingJsonPluginStringify =
  boolean | string

export interface BindingJsonSourcemap {
  file?: string
  mappings?: string
  sourceRoot?: string
  sources?: Array<string | undefined | null>
  sourcesContent?: Array<string | undefined | null>
  names?: Array<string>
}

export interface BindingJsWatchChangeEvent {
  event: string
}

export type BindingJsx =
  | { type: 'Disable' }
  | { type: 'Preserve' }
  | { type: 'Enable', field0: JsxOptions }

export declare enum BindingLogLevel {
  Silent = 0,
  Warn = 1,
  Info = 2,
  Debug = 3
}

export interface BindingManifestPluginConfig {
  root: string
  outPath: string
}

export interface BindingMatchGroup {
  name: string
  test?: BindingStringOrRegex
  priority?: number
  minSize?: number
  minShareCount?: number
  minModuleSize?: number
  maxModuleSize?: number
  maxSize?: number
}

export interface BindingMfManifest {
  filePath?: string
  disableAssetsAnalyze?: boolean
  fileName?: string
}

export interface BindingMinifyOptions {
  mangle: boolean
  compress: boolean
  removeWhitespace: boolean
}

export interface BindingModuleFederationPluginOption {
  name: string
  filename?: string
  exposes?: Record<string, string>
  remotes?: Array<BindingRemote>
  shared?: Record<string, BindingShared>
  runtimePlugins?: Array<string>
  manifest?: BindingMfManifest
}

export interface BindingModulePreloadPolyfillPluginConfig {
  skip?: boolean
}

export interface BindingModules {
  values: Array<BindingRenderedModule>
  keys: Array<string>
}

export interface BindingModuleSideEffectsRule {
  test?: RegExp | undefined
  sideEffects: boolean
  external?: boolean | undefined
}

export interface BindingNotifyOption {
  pollInterval?: number
  compareContents?: boolean
}

export interface BindingOutputOptions {
  name?: string
  assetFileNames?: string | ((chunk: BindingPreRenderedAsset) => string)
  entryFileNames?: string | ((chunk: PreRenderedChunk) => string)
  chunkFileNames?: string | ((chunk: PreRenderedChunk) => string)
  cssEntryFileNames?: string | ((chunk: PreRenderedChunk) => string)
  cssChunkFileNames?: string | ((chunk: PreRenderedChunk) => string)
  sanitizeFileName?: boolean | ((name: string) => string)
  banner?: (chunk: RenderedChunk) => MaybePromise<VoidNullable<string>>
  dir?: string
  file?: string
  esModule?: boolean | 'if-default-prop'
  exports?: 'default' | 'named' | 'none' | 'auto'
  extend?: boolean
  externalLiveBindings?: boolean
  footer?: (chunk: RenderedChunk) => MaybePromise<VoidNullable<string>>
  format?: 'es' | 'cjs' | 'iife' | 'umd' | 'app'
  globals?: Record<string, string> | ((name: string) => string)
  hashCharacters?: 'base64' | 'base36' | 'hex'
  inlineDynamicImports?: boolean
  intro?: (chunk: RenderedChunk) => MaybePromise<VoidNullable<string>>
  outro?: (chunk: RenderedChunk) => MaybePromise<VoidNullable<string>>
  plugins: (BindingBuiltinPlugin | BindingPluginOptions | undefined)[]
  sourcemap?: 'file' | 'inline' | 'hidden'
  sourcemapIgnoreList?: (source: string, sourcemapPath: string) => boolean
  sourcemapDebugIds?: boolean
  sourcemapPathTransform?: (source: string, sourcemapPath: string) => string
  minify?: boolean | 'dce-only' | BindingMinifyOptions
  advancedChunks?: BindingAdvancedChunksOptions
  comments?: 'none' | 'preserve-legal'
  polyfillRequire?: boolean
  target?: string
}

export interface BindingPluginContextResolvedId {
  id: string
  external: boolean
}

export interface BindingPluginContextResolveOptions {
  importKind?: 'import' | 'dynamic-import' | 'require-call'
  skipSelf?: boolean
  custom?: number
}

export interface BindingPluginHookMeta {
  order?: BindingPluginOrder
}

export interface BindingPluginOptions {
  name: string
  buildStart?: (ctx: BindingPluginContext, opts: BindingNormalizedOptions) => MaybePromise<VoidNullable>
  buildStartMeta?: BindingPluginHookMeta
  resolveId?: (ctx: BindingPluginContext, specifier: string, importer: Nullable<string>, options: BindingHookResolveIdExtraArgs) => MaybePromise<VoidNullable<BindingHookResolveIdOutput>>
  resolveIdMeta?: BindingPluginHookMeta
  resolveIdFilter?: BindingGeneralHookFilter
  resolveDynamicImport?: (ctx: BindingPluginContext, specifier: string, importer: Nullable<string>) => MaybePromise<VoidNullable<BindingHookResolveIdOutput>>
  resolveDynamicImportMeta?: BindingPluginHookMeta
  load?: (ctx: BindingPluginContext, id: string) => MaybePromise<VoidNullable<BindingHookLoadOutput>>
  loadMeta?: BindingPluginHookMeta
  loadFilter?: BindingGeneralHookFilter
  transform?: (ctx:  BindingTransformPluginContext, id: string, code: string, module_type: BindingTransformHookExtraArgs) => MaybePromise<VoidNullable<BindingHookTransformOutput>>
  transformMeta?: BindingPluginHookMeta
  transformFilter?: BindingTransformHookFilter
  moduleParsed?: (ctx: BindingPluginContext, module: BindingModuleInfo) => MaybePromise<VoidNullable>
  moduleParsedMeta?: BindingPluginHookMeta
  buildEnd?: (ctx: BindingPluginContext, error?: (Error | BindingError)[]) => MaybePromise<VoidNullable>
  buildEndMeta?: BindingPluginHookMeta
  renderChunk?: (ctx: BindingPluginContext, code: string, chunk: RenderedChunk, opts: BindingNormalizedOptions) => MaybePromise<VoidNullable<BindingHookRenderChunkOutput>>
  renderChunkMeta?: BindingPluginHookMeta
  augmentChunkHash?: (ctx: BindingPluginContext, chunk: RenderedChunk) => MaybePromise<void | string>
  augmentChunkHashMeta?: BindingPluginHookMeta
  renderStart?: (ctx: BindingPluginContext, opts: BindingNormalizedOptions) => void
  renderStartMeta?: BindingPluginHookMeta
  renderError?: (ctx: BindingPluginContext, error: (Error | BindingError)[]) => void
  renderErrorMeta?: BindingPluginHookMeta
  generateBundle?: (ctx: BindingPluginContext, bundle: BindingOutputs, isWrite: boolean, opts: BindingNormalizedOptions) => MaybePromise<VoidNullable<JsChangedOutputs>>
  generateBundleMeta?: BindingPluginHookMeta
  writeBundle?: (ctx: BindingPluginContext, bundle: BindingOutputs, opts: BindingNormalizedOptions) => MaybePromise<VoidNullable<JsChangedOutputs>>
  writeBundleMeta?: BindingPluginHookMeta
  closeBundle?: (ctx: BindingPluginContext) => MaybePromise<VoidNullable>
  closeBundleMeta?: BindingPluginHookMeta
  watchChange?: (ctx: BindingPluginContext, path: string, event: string) => MaybePromise<VoidNullable>
  watchChangeMeta?: BindingPluginHookMeta
  closeWatcher?: (ctx: BindingPluginContext) => MaybePromise<VoidNullable>
  closeWatcherMeta?: BindingPluginHookMeta
  banner?: (ctx: BindingPluginContext, chunk: RenderedChunk) => void
  bannerMeta?: BindingPluginHookMeta
  footer?: (ctx: BindingPluginContext, chunk: RenderedChunk) => void
  footerMeta?: BindingPluginHookMeta
  intro?: (ctx: BindingPluginContext, chunk: RenderedChunk) => void
  introMeta?: BindingPluginHookMeta
  outro?: (ctx: BindingPluginContext, chunk: RenderedChunk) => void
  outroMeta?: BindingPluginHookMeta
}

export declare enum BindingPluginOrder {
  Pre = 0,
  Post = 1
}

export interface BindingPluginWithIndex {
  index: number
  plugin: BindingPluginOptions
}

export interface BindingPreRenderedAsset {
  names: Array<string>
  originalFileNames: Array<string>
  source: BindingAssetSource
}

export interface BindingRemote {
  type?: string
  entry: string
  name: string
  entryGlobalName?: string
  shareScope?: string
}

export interface BindingReplacePluginConfig {
  values: Record<string, string>
  delimiters?: [string, string]
  preventAssignment?: boolean
  objectGuards?: boolean
  sourcemap?: boolean
}

export interface BindingResolveOptions {
  alias?: Array<AliasItem>
  aliasFields?: Array<Array<string>>
  conditionNames?: Array<string>
  exportsFields?: Array<Array<string>>
  extensions?: Array<string>
  extensionAlias?: Array<ExtensionAliasItem>
  mainFields?: Array<string>
  mainFiles?: Array<string>
  modules?: Array<string>
  symlinks?: boolean
  tsconfigFilename?: string
}

export interface BindingShared {
  version?: string
  shareScope?: string
  singleton?: boolean
  requiredVersion?: string
  strictVersion?: boolean
}

export interface BindingSourcemap {
  inner: string | BindingJsonSourcemap
}

export interface BindingTransformHookExtraArgs {
  moduleType: string
}

export interface BindingTransformHookFilter {
  code?: BindingGeneralHookFilter
  moduleType?: Array<string>
  id?: BindingGeneralHookFilter
}

export interface BindingTransformPluginConfig {
  include?: Array<BindingStringOrRegex>
  exclude?: Array<BindingStringOrRegex>
  jsxInject?: string
  reactRefresh?: boolean
  target?: string
  browserslist?: string
}

export interface BindingTreeshake {
  moduleSideEffects: boolean | BindingModuleSideEffectsRule[] | ((id: string, is_external: boolean) => boolean | undefined)
  annotations?: boolean
}

export interface BindingViteResolvePluginConfig {
  resolveOptions: BindingViteResolvePluginResolveOptions
  environmentConsumer: string
  environmentName: string
  external: true | string[]
  noExternal: true | Array<string | RegExp>
  dedupe: Array<string>
  finalizeBareSpecifier?: (resolvedId: string, rawId: string, importer: string | null | undefined) => VoidNullable<string>
  finalizeOtherSpecifiers?: (resolvedId: string, rawId: string) => VoidNullable<string>
  runtime: string
}

export interface BindingViteResolvePluginResolveOptions {
  isBuild: boolean
  isProduction: boolean
  asSrc: boolean
  preferRelative: boolean
  isRequire?: boolean
  root: string
  scan: boolean
  mainFields: Array<string>
  conditions: Array<string>
  externalConditions: Array<string>
  extensions: Array<string>
  tryIndex: boolean
  tryPrefix?: string
  preserveSymlinks: boolean
}

export interface BindingWatchOption {
  skipWrite?: boolean
  include?: Array<BindingStringOrRegex>
  exclude?: Array<BindingStringOrRegex>
}

export interface Comment {
  type: 'Line' | 'Block'
  value: string
  start: number
  end: number
}

export interface CompilerAssumptions {
  ignoreFunctionLength?: boolean
  noDocumentAll?: boolean
  objectRestNoSymbols?: boolean
  pureGetters?: boolean
  setPublicClassFields?: boolean
}

export interface DecoratorOptions {
  /**
   * Enables experimental support for decorators, which is a version of decorators that predates the TC39 standardization process.
   *
   * Decorators are a language feature which hasn’t yet been fully ratified into the JavaScript specification.
   * This means that the implementation version in TypeScript may differ from the implementation in JavaScript when it it decided by TC39.
   *
   * @see https://www.typescriptlang.org/tsconfig/#experimentalDecorators
   * @default false
   */
  legacy?: boolean
}

export interface DynamicImport {
  start: number
  end: number
  moduleRequest: Span
}

export interface EcmaScriptModule {
  /**
   * Has ESM syntax.
   *
   * i.e. `import` and `export` statements, and `import.meta`.
   *
   * Dynamic imports `import('foo')` are ignored since they can be used in non-ESM files.
   */
  hasModuleSyntax: boolean
  /** Import statements. */
  staticImports: Array<StaticImport>
  /** Export statements. */
  staticExports: Array<StaticExport>
  /** Dynamic import expressions. */
  dynamicImports: Array<DynamicImport>
  /** Span positions` of `import.meta` */
  importMetas: Array<Span>
}

export interface ErrorLabel {
  message?: string
  start: number
  end: number
}

export interface Es2015Options {
  /** Transform arrow functions into function expressions. */
  arrowFunction?: ArrowFunctionsOptions
}

export interface ExportExportName {
  kind: ExportExportNameKind
  name?: string
  start?: number
  end?: number
}

export type ExportExportNameKind = /** `export { name } */
'Name'|
/** `export default expression` */
'Default'|
/** `export * from "mod" */
'None';

export interface ExportImportName {
  kind: ExportImportNameKind
  name?: string
  start?: number
  end?: number
}

export type ExportImportNameKind = /** `export { name } */
'Name'|
/** `export * as ns from "mod"` */
'All'|
/** `export * from "mod"` */
'AllButDefault'|
/** Does not have a specifier. */
'None';

export interface ExportLocalName {
  kind: ExportLocalNameKind
  name?: string
  start?: number
  end?: number
}

export type ExportLocalNameKind = /** `export { name } */
'Name'|
/** `export default expression` */
'Default'|
/**
 * If the exported value is not locally accessible from within the module.
 * `export default function () {}`
 */
'None';

export interface ExtensionAliasItem {
  target: string
  replacements: Array<string>
}

export interface GenerateDecodedMapOptions {
  /** The filename of the file containing the original source. */
  source?: string
  /** Whether to include the original content in the map's `sourcesContent` array. */
  includeContent: boolean
  /** Whether the mapping should be high-resolution. */
  hires: boolean | 'boundary'
}

export type HelperMode = /**
 * Runtime mode (default): Helper functions are imported from a runtime package.
 *
 * Example:
 *
 * ```js
 * import helperName from "@babel/runtime/helpers/helperName";
 * helperName(...arguments);
 * ```
 */
'Runtime'|
/**
 * External mode: Helper functions are accessed from a global `babelHelpers` object.
 *
 * Example:
 *
 * ```js
 * babelHelpers.helperName(...arguments);
 * ```
 */
'External';

export interface Helpers {
  mode?: HelperMode
}

export interface ImportName {
  kind: ImportNameKind
  name?: string
  start?: number
  end?: number
}

export type ImportNameKind = /** `import { x } from "mod"` */
'Name'|
/** `import * as ns from "mod"` */
'NamespaceObject'|
/** `import defaultExport from "mod"` */
'Default';

/** TypeScript Isolated Declarations for Standalone DTS Emit */
export declare function isolatedDeclaration(filename: string, sourceText: string, options?: IsolatedDeclarationsOptions | undefined | null): IsolatedDeclarationsResult

export interface IsolatedDeclarationsOptions {
  /**
   * Do not emit declarations for code that has an @internal annotation in its JSDoc comment.
   * This is an internal compiler option; use at your own risk, because the compiler does not check that the result is valid.
   *
   * Default: `false`
   *
   * See <https://www.typescriptlang.org/tsconfig/#stripInternal>
   */
  stripInternal?: boolean
  sourcemap?: boolean
}

export interface IsolatedDeclarationsResult {
  code: string
  map?: SourceMap
  errors: Array<OxcError>
}

export interface JsChangedOutputs {
  chunks: Array<JsOutputChunk>
  assets: Array<JsOutputAsset>
  deleted: Array<string>
}

export interface JsOutputAsset {
  names: Array<string>
  originalFileNames: Array<string>
  filename: string
  source: BindingAssetSource
}

export interface JsOutputChunk {
  name: string
  isEntry: boolean
  isDynamicEntry: boolean
  facadeModuleId?: string
  moduleIds: Array<string>
  exports: Array<string>
  filename: string
  modules: Record<string, BindingRenderedModule>
  imports: Array<string>
  dynamicImports: Array<string>
  code: string
  map?: BindingSourcemap
  sourcemapFilename?: string
  preliminaryFilename: string
}

/**
 * Configure how TSX and JSX are transformed.
 *
 * @see {@link https://babeljs.io/docs/babel-plugin-transform-react-jsx#options}
 */
export interface JsxOptions {
  /**
   * Decides which runtime to use.
   *
   * - 'automatic' - auto-import the correct JSX factories
   * - 'classic' - no auto-import
   *
   * @default 'automatic'
   */
  runtime?: 'classic' | 'automatic'
  /**
   * Emit development-specific information, such as `__source` and `__self`.
   *
   * @default false
   *
   * @see {@link https://babeljs.io/docs/babel-plugin-transform-react-jsx-development}
   */
  development?: boolean
  /**
   * Toggles whether or not to throw an error if an XML namespaced tag name
   * is used.
   *
   * Though the JSX spec allows this, it is disabled by default since React's
   * JSX does not currently have support for it.
   *
   * @default true
   */
  throwIfNamespace?: boolean
  /**
   * Enables `@babel/plugin-transform-react-pure-annotations`.
   *
   * It will mark top-level React method calls as pure for tree shaking.
   *
   * @see {@link https://babeljs.io/docs/en/babel-plugin-transform-react-pure-annotations}
   *
   * @default true
   */
  pure?: boolean
  /**
   * Replaces the import source when importing functions.
   *
   * @default 'react'
   */
  importSource?: string
  /**
   * Replace the function used when compiling JSX expressions. It should be a
   * qualified name (e.g. `React.createElement`) or an identifier (e.g.
   * `createElement`).
   *
   * Only used for `classic` {@link runtime}.
   *
   * @default 'React.createElement'
   */
  pragma?: string
  /**
   * Replace the component used when compiling JSX fragments. It should be a
   * valid JSX tag name.
   *
   * Only used for `classic` {@link runtime}.
   *
   * @default 'React.Fragment'
   */
  pragmaFrag?: string
  /**
   * When spreading props, use `Object.assign` directly instead of an extend helper.
   *
   * Only used for `classic` {@link runtime}.
   *
   * @default false
   */
  useBuiltIns?: boolean
  /**
   * When spreading props, use inline object with spread elements directly
   * instead of an extend helper or Object.assign.
   *
   * Only used for `classic` {@link runtime}.
   *
   * @default false
   */
  useSpread?: boolean
  /**
   * Enable React Fast Refresh .
   *
   * Conforms to the implementation in {@link https://github.com/facebook/react/tree/v18.3.1/packages/react-refresh}
   *
   * @default false
   */
  refresh?: boolean | ReactRefreshOptions
}

export interface LineColumn {
  line: number
  column: number
}

export interface OverwriteOptions {
  contentOnly: boolean
}

export interface OxcError {
  severity: Severity
  message: string
  labels: Array<ErrorLabel>
  helpMessage?: string
}

/**
 * Parse asynchronously.
 *
 * Note: This function can be slower than `parseSync` due to the overhead of spawning a thread.
 */
export declare function parseAsync(filename: string, sourceText: string, options?: ParserOptions | undefined | null): Promise<ParseResult>

export interface ParserOptions {
  sourceType?: 'script' | 'module' | 'unambiguous' | undefined
  /** Treat the source text as `js`, `jsx`, `ts`, or `tsx`. */
  lang?: 'js' | 'jsx' | 'ts' | 'tsx'
  /**
   * Emit `ParenthesizedExpression` in AST.
   *
   * If this option is true, parenthesized expressions are represented by
   * (non-standard) `ParenthesizedExpression` nodes that have a single `expression` property
   * containing the expression inside parentheses.
   *
   * Default: true
   */
  preserveParens?: boolean
  /**
   * Default: false
   * @experimental Only for internal usage on Rolldown and Vite.
   */
  convertSpanUtf16?: boolean
}

/** Parse synchronously. */
export declare function parseSync(filename: string, sourceText: string, options?: ParserOptions | undefined | null): ParseResult

/**
 * Parse without returning anything.
 *
 * This is for benchmark purposes such as measuring napi communication overhead.
 */
export declare function parseWithoutReturn(filename: string, sourceText: string, options?: ParserOptions | undefined | null): void

export interface PreRenderedChunk {
  name: string
  isEntry: boolean
  isDynamicEntry: boolean
  facadeModuleId?: string
  moduleIds: Array<string>
  exports: Array<string>
}

export interface ReactRefreshOptions {
  /**
   * Specify the identifier of the refresh registration variable.
   *
   * @default `$RefreshReg$`.
   */
  refreshReg?: string
  /**
   * Specify the identifier of the refresh signature variable.
   *
   * @default `$RefreshSig$`.
   */
  refreshSig?: string
  emitFullSignatures?: boolean
}

export declare function registerPlugins(id: number, plugins: Array<BindingPluginWithIndex>): void

export type Severity =  'Error'|
'Warning'|
'Advice';

export interface SourceMap {
  file?: string
  mappings: string
  names: Array<string>
  sourceRoot?: string
  sources: Array<string>
  sourcesContent?: Array<string>
  version: number
  x_google_ignoreList?: Array<number>
}

export interface SourceMapOptions {
  includeContent?: boolean
  source?: string
  hires?: boolean
}

export interface Span {
  start: number
  end: number
}

export interface StaticExport {
  start: number
  end: number
  entries: Array<StaticExportEntry>
}

export interface StaticExportEntry {
  start: number
  end: number
  moduleRequest?: ValueSpan
  /** The name under which the desired binding is exported by the module`. */
  importName: ExportImportName
  /** The name used to export this binding by this module. */
  exportName: ExportExportName
  /** The name that is used to locally access the exported value from within the importing module. */
  localName: ExportLocalName
}

export interface StaticImport {
  /** Start of import statement. */
  start: number
  /** End of import statement. */
  end: number
  /**
   * Import source.
   *
   * ```js
   * import { foo } from "mod";
   * //                   ^^^
   * ```
   */
  moduleRequest: ValueSpan
  /**
   * Import specifiers.
   *
   * Empty for `import "mod"`.
   */
  entries: Array<StaticImportEntry>
}

export interface StaticImportEntry {
  /**
   * The name under which the desired binding is exported by the module.
   *
   * ```js
   * import { foo } from "mod";
   * //       ^^^
   * import { foo as bar } from "mod";
   * //       ^^^
   * ```
   */
  importName: ImportName
  /**
   * The name that is used to locally access the imported value from within the importing module.
   * ```js
   * import { foo } from "mod";
   * //       ^^^
   * import { foo as bar } from "mod";
   * //              ^^^
   * ```
   */
  localName: ValueSpan
  /**
   * Whether this binding is for a TypeScript type-only import.
   *
   * `true` for the following imports:
   * ```ts
   * import type { foo } from "mod";
   * import { type foo } from "mod";
   * ```
   */
  isType: boolean
}

/**
 * Transpile a JavaScript or TypeScript into a target ECMAScript version.
 *
 * @param filename The name of the file being transformed. If this is a
 * relative path, consider setting the {@link TransformOptions#cwd} option..
 * @param sourceText the source code itself
 * @param options The options for the transformation. See {@link
 * TransformOptions} for more information.
 *
 * @returns an object containing the transformed code, source maps, and any
 * errors that occurred during parsing or transformation.
 */
export declare function transform(filename: string, sourceText: string, options?: TransformOptions | undefined | null): TransformResult

/**
 * Options for transforming a JavaScript or TypeScript file.
 *
 * @see {@link transform}
 */
export interface TransformOptions {
  sourceType?: 'script' | 'module' | 'unambiguous' | undefined
  /** Treat the source text as `js`, `jsx`, `ts`, or `tsx`. */
  lang?: 'js' | 'jsx' | 'ts' | 'tsx'
  /**
   * The current working directory. Used to resolve relative paths in other
   * options.
   */
  cwd?: string
  /**
   * Enable source map generation.
   *
   * When `true`, the `sourceMap` field of transform result objects will be populated.
   *
   * @default false
   *
   * @see {@link SourceMap}
   */
  sourcemap?: boolean
  /** Set assumptions in order to produce smaller output. */
  assumptions?: CompilerAssumptions
  /** Configure how TypeScript is transformed. */
  typescript?: TypeScriptOptions
  /** Configure how TSX and JSX are transformed. */
  jsx?: 'preserve' | JsxOptions
  /**
   * Sets the target environment for the generated JavaScript.
   *
   * The lowest target is `es2015`.
   *
   * Example:
   *
   * * 'es2015'
   * * ['es2020', 'chrome58', 'edge16', 'firefox57', 'node12', 'safari11']
   *
   * @default `esnext` (No transformation)
   *
   * @see [esbuild#target](https://esbuild.github.io/api/#target)
   */
  target?: string | Array<string>
  /** Behaviour for runtime helpers. */
  helpers?: Helpers
  /** Define Plugin */
  define?: Record<string, string>
  /** Inject Plugin */
  inject?: Record<string, string | [string, string]>
  /** Decorator plugin */
  decorator?: DecoratorOptions
}

export interface TransformResult {
  /**
   * The transformed code.
   *
   * If parsing failed, this will be an empty string.
   */
  code: string
  /**
   * The source map for the transformed code.
   *
   * This will be set if {@link TransformOptions#sourcemap} is `true`.
   */
  map?: SourceMap
  /**
   * The `.d.ts` declaration file for the transformed code. Declarations are
   * only generated if `declaration` is set to `true` and a TypeScript file
   * is provided.
   *
   * If parsing failed and `declaration` is set, this will be an empty string.
   *
   * @see {@link TypeScriptOptions#declaration}
   * @see [declaration tsconfig option](https://www.typescriptlang.org/tsconfig/#declaration)
   */
  declaration?: string
  /**
   * Declaration source map. Only generated if both
   * {@link TypeScriptOptions#declaration declaration} and
   * {@link TransformOptions#sourcemap sourcemap} are set to `true`.
   */
  declarationMap?: SourceMap
  /**
   * Helpers used.
   *
   * @internal
   *
   * Example:
   *
   * ```text
   * { "_objectSpread": "@babel/runtime/helpers/objectSpread2" }
   * ```
   */
  helpersUsed: Record<string, string>
  /**
   * Parse and transformation errors.
   *
   * Oxc's parser recovers from common syntax errors, meaning that
   * transformed code may still be available even if there are errors in this
   * list.
   */
  errors: Array<OxcError>
}

export interface TypeScriptOptions {
  jsxPragma?: string
  jsxPragmaFrag?: string
  onlyRemoveTypeImports?: boolean
  allowNamespaces?: boolean
  allowDeclareFields?: boolean
  /**
   * Also generate a `.d.ts` declaration file for TypeScript files.
   *
   * The source file must be compliant with all
   * [`isolatedDeclarations`](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-5.html#isolated-declarations)
   * requirements.
   *
   * @default false
   */
  declaration?: IsolatedDeclarationsOptions
  /**
   * Rewrite or remove TypeScript import/export declaration extensions.
   *
   * - When set to `rewrite`, it will change `.ts`, `.mts`, `.cts` extensions to `.js`, `.mjs`, `.cjs` respectively.
   * - When set to `remove`, it will remove `.ts`/`.mts`/`.cts`/`.tsx` extension entirely.
   * - When set to `true`, it's equivalent to `rewrite`.
   * - When set to `false` or omitted, no changes will be made to the extensions.
   *
   * @default false
   */
  rewriteImportExtensions?: 'rewrite' | 'remove' | boolean
}

export interface ValueSpan {
  value: string
  start: number
  end: number
}
