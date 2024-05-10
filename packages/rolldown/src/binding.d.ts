type MaybePromise<T> = T | Promise<T>
type Nullable<T> = T | null | undefined
type VoidNullable<T = void> = T | null | undefined | void
export class BindingModuleInfo {
  id: string
  importers: Array<string>
  dynamicImporters: Array<string>
  importedIds: Array<string>
  dynamicallyImportedIds: Array<string>
  isEntry: boolean
  get code(): string | null
}

export class BindingOutputAsset {
  get fileName(): string
  get source(): string
  set source(source: string)
}

export class BindingOutputChunk {
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
}

/** The `BindingOutputs` owner `Vec<Output>` the mutable reference, it avoid `Clone` at call `writeBundle/generateBundle` hook, and make it mutable. */
export class BindingOutputs {
  get chunks(): Array<BindingOutputChunk>
  get assets(): Array<BindingOutputAsset>
}

export class BindingPluginContext {
  resolve(specifier: string, importer: string | undefined | null, extraOptions: BindingPluginContextResolveOptions): void
}

export class Bundler {
  constructor(inputOptions: BindingInputOptions, outputOptions: BindingOutputOptions, parallelPluginsRegistry?: ParallelJsPluginRegistry | undefined | null)
  write(): Promise<FinalBindingOutputs>
  generate(): Promise<FinalBindingOutputs>
  scan(): Promise<void>
}

/**
 * The `FinalBindingOutputs` is used at `write()` or `generate()`, it is similar to `BindingOutputs`, if using `BindingOutputs` has unexpected behavior.
 * TODO find a way to export it gracefully.
 */
export class FinalBindingOutputs {
  get chunks(): Array<BindingOutputChunk>
  get assets(): Array<BindingOutputAsset>
}

export class ParallelJsPluginRegistry {
  id: number
  workerCount: number
  constructor(workerCount: number)
}

export interface AliasItem {
  find: string
  replacements: Array<string>
}

export interface BindingHookLoadOutput {
  code: string
  map?: string
}

export interface BindingHookRenderChunkOutput {
  code: string
  map?: string
}

export interface BindingHookResolveIdExtraOptions {
  isEntry: boolean
  kind: string
}

export interface BindingHookResolveIdOutput {
  id: string
  external?: boolean
}

export interface BindingInputItem {
  name?: string
  import: string
}

export interface BindingInputOptions {
  external?: undefined | ((source: string, importer: string | undefined, isResolved: boolean) => boolean)
  input: Array<BindingInputItem>
  plugins: Array<BindingPluginOrParallelJsPluginPlaceholder>
  resolve?: BindingResolveOptions
  shimMissingExports?: boolean
  platform?: 'node' | 'browser' | 'neutral'
  logLevel?: 'silent' | 'error' | 'warn' | 'info'
  cwd: string
}

export interface BindingOutputOptions {
  entryFileNames?: string
  chunkFileNames?: string
  banner?: (chunk: RenderedChunk) => MaybePromise<VoidNullable<string>>
  dir?: string
  exports?: 'default' | 'named' | 'none' | 'auto'
  footer?: (chunk: RenderedChunk) => MaybePromise<VoidNullable<string>>
  format?: 'es' | 'cjs'
  plugins: Array<BindingPluginOrParallelJsPluginPlaceholder>
  sourcemap?: 'file' | 'inline' | 'hidden'
  sourcemapIgnoreList?: (source: string, sourcemapPath: string) => boolean
  sourcemapPathTransform?: (source: string, sourcemapPath: string) => string
}

export interface BindingPluginContextResolveOptions {
  importKind: string
}

export interface BindingPluginOptions {
  name: string
  buildStart?: (ctx: BindingPluginContext) => MaybePromise<VoidNullable>
  resolveId?: (ctx: BindingPluginContext, specifier: string, importer: Nullable<string>, options: BindingHookResolveIdExtraOptions) => MaybePromise<VoidNullable<BindingHookResolveIdOutput>>
  resolveDynamicImport?: (ctx: BindingPluginContext, specifier: string, importer: Nullable<string>) => MaybePromise<VoidNullable<BindingHookResolveIdOutput>>
  load?: (ctx: BindingPluginContext, id: string) => MaybePromise<VoidNullable<BindingHookLoadOutput>>
  transform?: (ctx: BindingPluginContext, id: string, code: string) => MaybePromise<VoidNullable<BindingHookLoadOutput>>
  moduleParsed?: (ctx: BindingPluginContext, module: BindingModuleInfo) => MaybePromise<VoidNullable>
  buildEnd?: (ctx: BindingPluginContext, error: Nullable<string>) => MaybePromise<VoidNullable>
  renderChunk?: (ctx: BindingPluginContext, code: string, chunk: RenderedChunk) => MaybePromise<VoidNullable<BindingHookRenderChunkOutput>>
  renderStart?: (ctx: BindingPluginContext) => void
  renderError?: (ctx: BindingPluginContext, error: string) => void
  generateBundle?: (ctx: BindingPluginContext, bundle: BindingOutputs, isWrite: boolean) => MaybePromise<VoidNullable>
  writeBundle?: (ctx: BindingPluginContext, bundle: BindingOutputs) => MaybePromise<VoidNullable>
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

export function registerPlugins(id: number, plugins: PluginsInSingleWorker): void

export interface RenderedChunk {
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

