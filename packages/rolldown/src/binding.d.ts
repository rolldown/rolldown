type MaybePromise<T> = T | Promise<T>
type Nullable<T> = T | null | undefined
type VoidNullable<T = void> = T | null | undefined | void
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
  isEntry: boolean
  get code(): string | null
}

export declare class BindingOutputAsset {
  get fileName(): string
  get source(): BindingAssetSource
  set source(source: BindingAssetSource)
  get name(): string | null
}

export declare class BindingOutputChunk {
  get isEntry(): boolean
  get isDynamicEntry(): boolean
  get facadeModuleId(): string | null
  get moduleIds(): Array<string>
  get exports(): Array<string>
  get fileName(): string
  get modules(): Record<string, BindingRenderedModule>
  get imports(): Array<string>
  set imports(imports: Array<string>)
  get dynamicImports(): Array<string>
  get code(): string
  set code(code: string)
  get map(): string | null
  set map(map: string)
  get sourcemapFileName(): string | null
  get preliminaryFileName(): string
  get name(): string
}

/** The `BindingOutputs` owner `Vec<Output>` the mutable reference, it avoid `Clone` at call `writeBundle/generateBundle` hook, and make it mutable. */
export declare class BindingOutputs {
  get chunks(): Array<BindingOutputChunk>
  get assets(): Array<BindingOutputAsset>
  delete(fileName: string): void
}

export declare class BindingPluginContext {
  resolve(specifier: string, importer?: string | undefined | null, extraOptions?: BindingPluginContextResolveOptions | undefined | null): Promise<BindingPluginContextResolvedId | null>
  emitFile(file: BindingEmittedAsset): string
  getFileName(referenceId: string): string
  getModuleInfo(moduleId: string): BindingModuleInfo | null
  getModuleIds(): Array<string> | null
}

export declare class BindingTransformPluginContext {
  inner(): BindingPluginContext
}

export declare class Bundler {
  constructor(inputOptions: BindingInputOptions, outputOptions: BindingOutputOptions, parallelPluginsRegistry?: ParallelJsPluginRegistry | undefined | null)
  write(): Promise<FinalBindingOutputs>
  generate(): Promise<FinalBindingOutputs>
  scan(): Promise<void>
}

/**
 * The `FinalBindingOutputs` is used at `write()` or `generate()`, it is similar to `BindingOutputs`, if using `BindingOutputs` has unexpected behavior.
 * TODO find a way to export it gracefully.
 */
export declare class FinalBindingOutputs {
  get chunks(): Array<BindingOutputChunk>
  get assets(): Array<BindingOutputAsset>
}

export declare class ParallelJsPluginRegistry {
  id: number
  workerCount: number
  constructor(workerCount: number)
}

export interface AliasItem {
  find: string
  replacements: Array<string>
}

export interface ArrowFunctionsBindingOptions {
  spec?: boolean
}

export interface BindingAssetSource {
  inner: string | Uint8Array
}

export interface BindingBuiltinPlugin {
  __name: BindingBuiltinPluginName
  options?: unknown
}

export declare enum BindingBuiltinPluginName {
  WasmPlugin = 0,
  GlobImportPlugin = 1,
  DynamicImportVarsPlugin = 2,
  ModulePreloadPolyfillPlugin = 3,
  ManifestPlugin = 4
}

export interface BindingEmittedAsset {
  name?: string
  fileName?: string
  source: BindingAssetSource
}

export interface BindingGlobImportPluginConfig {
  root?: string
  restoreQueryExtension?: boolean
}

export interface BindingHookLoadOutput {
  code: string
  sideEffects?: BindingHookSideEffects
  map?: BindingSourcemap
}

export interface BindingHookRenderChunkOutput {
  code: string
  map?: BindingSourcemap
}

export interface BindingHookResolveIdExtraOptions {
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
  logLevel?: BindingLogLevel
  onLog: (logLevel: 'debug' | 'warn' | 'info', log: BindingLog) => void
  cwd: string
  treeshake?: BindingTreeshake
  moduleTypes?: Record<string, string>
}

export interface BindingJsonSourcemap {
  file?: string
  mappings?: string
  sourceRoot?: string
  sources?: Array<string | undefined | null>
  sourcesContent?: Array<string | undefined | null>
  names?: Array<string>
}

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

export interface BindingModulePreloadPolyfillPluginConfig {
  skip?: boolean
}

export interface BindingOutputOptions {
  name?: string
  entryFileNames?: string
  chunkFileNames?: string
  assetFileNames?: string
  banner?: (chunk: RenderedChunk) => MaybePromise<VoidNullable<string>>
  dir?: string
  esModule?: 'always' | 'never' | 'if-default-prop'
  exports?: 'default' | 'named' | 'none' | 'auto'
  extend?: boolean
  footer?: (chunk: RenderedChunk) => MaybePromise<VoidNullable<string>>
  format?: 'es' | 'cjs' | 'iife'
  globals?: Record<string, string>
  intro?: (chunk: RenderedChunk) => MaybePromise<VoidNullable<string>>
  outro?: (chunk: RenderedChunk) => MaybePromise<VoidNullable<string>>
  plugins: (BindingBuiltinPlugin | BindingPluginOptions | undefined)[]
  sourcemap?: 'file' | 'inline' | 'hidden'
  sourcemapIgnoreList?: (source: string, sourcemapPath: string) => boolean
  sourcemapPathTransform?: (source: string, sourcemapPath: string) => string
  minify?: boolean
}

export interface BindingPluginContextResolvedId {
  id: string
  external: boolean
}

export interface BindingPluginContextResolveOptions {
  importKind?: 'import' | 'dynamic-import' | 'require-call'
  skipSelf?: boolean
}

export interface BindingPluginOptions {
  name: string
  buildStart?: (ctx: BindingPluginContext) => MaybePromise<VoidNullable>
  resolveId?: (ctx: BindingPluginContext, specifier: string, importer: Nullable<string>, options: BindingHookResolveIdExtraOptions) => MaybePromise<VoidNullable<BindingHookResolveIdOutput>>
  resolveDynamicImport?: (ctx: BindingPluginContext, specifier: string, importer: Nullable<string>) => MaybePromise<VoidNullable<BindingHookResolveIdOutput>>
  load?: (ctx: BindingPluginContext, id: string) => MaybePromise<VoidNullable<BindingHookLoadOutput>>
  transform?: (ctx:  BindingTransformPluginContext, id: string, code: string) => MaybePromise<VoidNullable<BindingHookTransformOutput>>
  moduleParsed?: (ctx: BindingPluginContext, module: BindingModuleInfo) => MaybePromise<VoidNullable>
  buildEnd?: (ctx: BindingPluginContext, error: Nullable<string>) => MaybePromise<VoidNullable>
  renderChunk?: (ctx: BindingPluginContext, code: string, chunk: RenderedChunk) => MaybePromise<VoidNullable<BindingHookRenderChunkOutput>>
  augmentChunkHash?: (ctx: BindingPluginContext, chunk: RenderedChunk) => MaybePromise<void | string>
  renderStart?: (ctx: BindingPluginContext) => void
  renderError?: (ctx: BindingPluginContext, error: string) => void
  generateBundle?: (ctx: BindingPluginContext, bundle: BindingOutputs, isWrite: boolean) => MaybePromise<VoidNullable>
  writeBundle?: (ctx: BindingPluginContext, bundle: BindingOutputs) => MaybePromise<VoidNullable>
  banner?: (ctx: BindingPluginContext, chunk: RenderedChunk) => void
  footer?: (ctx: BindingPluginContext, chunk: RenderedChunk) => void
  intro?: (ctx: BindingPluginContext, chunk: RenderedChunk) => void
  outro?: (ctx: BindingPluginContext, chunk: RenderedChunk) => void
}

export interface BindingPluginWithIndex {
  index: number
  plugin: BindingPluginOptions
}

export interface BindingRenderedModule {
  code?: string
}

export interface BindingResolveOptions {
  alias?: Array<AliasItem>
  aliasFields?: Array<Array<string>>
  conditionNames?: Array<string>
  exportsFields?: Array<Array<string>>
  extensions?: Array<string>
  mainFields?: Array<string>
  mainFiles?: Array<string>
  modules?: Array<string>
  symlinks?: boolean
  tsconfigFilename?: string
}

export interface BindingSourcemap {
  inner: string | BindingJsonSourcemap
}

export interface BindingTreeshake {
  moduleSideEffects: string
}

export interface Es2015BindingOptions {
  arrowFunction?: ArrowFunctionsBindingOptions
}

/** TypeScript Isolated Declarations for Standalone DTS Emit */
export declare function isolatedDeclaration(filename: string, sourceText: string): IsolatedDeclarationsResult

export interface IsolatedDeclarationsResult {
  sourceText: string
  errors: Array<string>
}

export interface ReactBindingOptions {
  runtime?: 'classic' | 'automatic'
  development?: boolean
  throwIfNamespace?: boolean
  pure?: boolean
  importSource?: string
  pragma?: string
  pragmaFrag?: string
  useBuiltIns?: boolean
  useSpread?: boolean
}

export declare function registerPlugins(id: number, plugins: Array<BindingPluginWithIndex>): void

export interface RenderedChunk {
  name: string
  isEntry: boolean
  isDynamicEntry: boolean
  facadeModuleId?: string
  moduleIds: Array<string>
  exports: Array<string>
  fileName: string
  modules: Record<string, BindingRenderedModule>
  imports: Array<string>
  dynamicImports: Array<string>
}

export interface Sourcemap {
  file?: string
  mappings?: string
  sourceRoot?: string
  sources?: Array<string | undefined | null>
  sourcesContent?: Array<string | undefined | null>
  names?: Array<string>
}

export declare function transform(filename: string, sourceText: string, options?: TransformOptions | undefined | null): TransformResult

export interface TransformOptions {
  sourceType?: 'script' | 'module' | 'unambiguous' | undefined
  /** Force jsx parsing, */
  jsx?: boolean
  typescript?: TypeScriptBindingOptions
  react?: ReactBindingOptions
  es2015?: Es2015BindingOptions
  /**
   * Enable Sourcemap
   *
   * * `true` to generate a sourcemap for the code and include it in the result object.
   *
   * Default: false
   */
  sourcemap?: boolean
}

export interface TransformResult {
  sourceText: string
  map?: Sourcemap
  errors: Array<string>
}

export interface TypeScriptBindingOptions {
  jsxPragma?: string
  jsxPragmaFrag?: string
  onlyRemoveTypeImports?: boolean
  allowNamespaces?: boolean
  allowDeclareFields?: boolean
}

