type MaybePromise<T> = T | Promise<T>
export class BindingPluginContext {}

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

export interface BindingPluginOptions {
  name: string
  buildStart?: (ctx: BindingPluginContext) => MaybePromise<void>
  resolveId?: (
    specifier: string,
    importer?: string,
    options?: BindingHookResolveIdExtraOptions,
  ) => MaybePromise<undefined | BindingHookResolveIdOutput>
  load?: (id: string) => MaybePromise<undefined | BindingHookLoadOutput>
  transform?: (
    id: string,
    code: string,
  ) => MaybePromise<undefined | BindingHookLoadOutput>
  buildEnd?: (error?: string) => MaybePromise<void>
  renderChunk?: (
    code: string,
    chunk: RenderedChunk,
  ) => MaybePromise<undefined | BindingHookRenderChunkOutput>
  generateBundle?: (bundle: Outputs, isWrite: boolean) => MaybePromise<void>
  writeBundle?: (bundle: Outputs) => MaybePromise<void>
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
