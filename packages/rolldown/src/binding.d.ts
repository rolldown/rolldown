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
  get dynamicImports(): Array<string>
  get code(): string
  get map(): string | null
  get sourcemapFileName(): string | null
}

export class BindingOutputs {
  get chunks(): Array<BindingOutputChunk>
  get assets(): Array<BindingOutputAsset>
}

export class BindingPluginContext {
  resolve(specifier: string, importer: string | undefined | null, extraOptions: BindingPluginContextResolveOptions): void
}

export class Bundler {
  constructor(inputOptions: BindingInputOptions, outputOptions: BindingOutputOptions, parallelPluginsRegistry?: ParallelJsPluginRegistry | undefined | null)
  write(): Promise<BindingOutputs>
  generate(): Promise<BindingOutputs>
  scan(): Promise<void>
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
  resolveId?: (specifier: string, importer: Nullable<string>, options: BindingHookResolveIdExtraOptions) => MaybePromise<VoidNullable<BindingHookResolveIdOutput>>
  resolveDynamicImport?: (specifier: string, importer: Nullable<string>) => MaybePromise<VoidNullable<BindingHookResolveIdOutput>>
  load?: (id: string) => MaybePromise<VoidNullable<BindingHookLoadOutput>>
  transform?: (id: string, code: string) => MaybePromise<VoidNullable<BindingHookLoadOutput>>
  moduleParsed?: (ctx: BindingPluginContext, module: BindingModuleInfo) => MaybePromise<VoidNullable>
  buildEnd?: (error: Nullable<string>) => MaybePromise<VoidNullable>
  renderChunk?: (code: string, chunk: RenderedChunk) => MaybePromise<VoidNullable<BindingHookRenderChunkOutput>>
  renderStart?: () => void
  renderError?: (error: string) => void
  generateBundle?: (bundle: BindingOutputs, isWrite: boolean) => MaybePromise<VoidNullable>
  writeBundle?: (bundle: BindingOutputs) => MaybePromise<VoidNullable>
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

