type MaybePromise<T> = T | Promise<T>
type Nullable<T> = T | null | undefined
type VoidNullable<T = void> = T | null | undefined | void
export class BindingPluginContext {
  resolve(
    specifier: string,
    importer: string | undefined | null,
    extraOptions: BindingPluginContextResolveOptions,
  ): void
}

export class Bundler {
  constructor(
    inputOptions: BindingInputOptions,
    outputOptions: BindingOutputOptions,
  )
  write(): Promise<BindingOutputs>
  generate(): Promise<BindingOutputs>
  scan(): Promise<void>
}

export interface BindingHookLoadOutput {
  code: string
  map?: string
}

export interface BindingHookRenderChunkOutput {
  code: string
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
  external?:
    | undefined
    | ((
        source: string,
        importer: string | undefined,
        isResolved: boolean,
      ) => boolean)
  input: Array<BindingInputItem>
  plugins: Array<BindingPluginOptions>
  resolve?: BindingResolveOptions
  cwd: string
}

export interface BindingOutputAsset {
  fileName: string
  source: string
}

export interface BindingOutputChunk {
  isEntry: boolean
  isDynamicEntry: boolean
  facadeModuleId?: string
  moduleIds: Array<string>
  exports: Array<string>
  fileName: string
  modules: Record<string, BindingRenderedModule>
  code: string
  map?: string
  sourcemapFileName?: string
}

export interface BindingOutputOptions {
  entryFileNames?: string
  chunkFileNames?: string
  banner?:
    | Nullable<string>
    | ((chunk: RenderedChunk) => MaybePromise<VoidNullable<string>>)
  dir?: string
  exports?: 'default' | 'named' | 'none' | 'auto'
  format?: 'es' | 'cjs'
  plugins: Array<BindingPluginOptions>
  sourcemap?: 'file' | 'inline' | 'hidden'
}

export interface BindingOutputs {
  chunks: Array<BindingOutputChunk>
  assets: Array<BindingOutputAsset>
}

export interface BindingPluginContextResolveOptions {
  importKind: string
}

export interface BindingPluginOptions {
  name: string
  buildStart?: (ctx: BindingPluginContext) => MaybePromise<VoidNullable>
  resolveId?: (
    specifier: string,
    importer: Nullable<string>,
    options: BindingHookResolveIdExtraOptions,
  ) => MaybePromise<VoidNullable<BindingHookResolveIdOutput>>
  load?: (id: string) => MaybePromise<VoidNullable<BindingHookLoadOutput>>
  transform?: (
    id: string,
    code: string,
  ) => MaybePromise<VoidNullable<BindingHookLoadOutput>>
  buildEnd?: (error: Nullable<string>) => MaybePromise<VoidNullable>
  renderChunk?: (
    code: string,
    chunk: RenderedChunk,
  ) => MaybePromise<VoidNullable<BindingHookRenderChunkOutput>>
  generateBundle?: (
    bundle: Outputs,
    isWrite: boolean,
  ) => MaybePromise<VoidNullable>
  writeBundle?: (bundle: Outputs) => MaybePromise<VoidNullable>
}

export interface BindingRenderedModule {
  code?: string
}

export interface BindingResolveOptions {
  alias?: Record<string, Array<string>>
  aliasFields?: Array<Array<string>>
  conditionNames?: Array<string>
  exportsFields?: Array<Array<string>>
  extensions?: Array<string>
  mainFields?: Array<string>
  mainFiles?: Array<string>
  modules?: Array<string>
  symlinks?: boolean
}

export interface RenderedChunk {
  isEntry: boolean
  isDynamicEntry: boolean
  facadeModuleId?: string
  moduleIds: Array<string>
  exports: Array<string>
  fileName: string
  modules: Record<string, BindingRenderedModule>
}
