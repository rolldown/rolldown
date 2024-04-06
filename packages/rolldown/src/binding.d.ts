type MaybePromise<T> = T | Promise<T>
type Nullable<T> = T | null | undefined
type VoidNullable<T = void> = T | null | undefined | void
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
  get code(): string
  get map(): string | null
  get sourcemapFileName(): string | null
}

export class BindingOutputs {
  get chunks(): Array<BindingOutputChunk>
  get assets(): Array<BindingOutputAsset>
}

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

export interface BindingOutputOptions {
  entryFileNames?: string
  chunkFileNames?: string
  banner?:
    | Nullable<string>
    | ((chunk: RenderedChunk) => MaybePromise<VoidNullable<string>>)
  dir?: string
  exports?: 'default' | 'named' | 'none' | 'auto'
  footer?:
    | Nullable<string>
    | ((chunk: RenderedChunk) => MaybePromise<VoidNullable<string>>)
  format?: 'es' | 'cjs'
  plugins: Array<BindingPluginOptions>
  sourcemap?: 'file' | 'inline' | 'hidden'
}

export interface BindingPluginContextResolveOptions {
  importKind: string
}

export interface BindingPluginOptions {
  name: string
  buildStart?: BuildStartHookOption
  resolveId?: ResolveIdHookOption
  load?: LoadOption
  transform?: TransformOption
  buildEnd?: BuildEndHookOption
  renderChunk?: RenderChunkOption
  generateBundle?: GenerateBundleOption
  writeBundle?: WriteBundleOption
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

export interface BuildEndHookOption {
  handler: (error: Nullable<string>) => MaybePromise<VoidNullable>
  order?: 'pre' | 'post' | null
  sequential?: boolean
}

export interface BuildStartHookOption {
  handler: (ctx: BindingPluginContext) => MaybePromise<VoidNullable>
  order?: 'pre' | 'post' | null
  sequential?: boolean
}

export interface GenerateBundleOption {
  handler: (
    bundle: BindingOutputs,
    isWrite: boolean,
  ) => MaybePromise<VoidNullable>
  order?: 'pre' | 'post' | null
}

export interface HookOption {
  order?: 'pre' | 'post' | null
  sequential?: boolean
}

export interface LoadOption {
  handler: (id: string) => MaybePromise<VoidNullable<BindingHookLoadOutput>>
  order?: 'pre' | 'post' | null
}

export interface RenderChunkOption {
  handler: (
    code: string,
    chunk: RenderedChunk,
  ) => MaybePromise<VoidNullable<BindingHookRenderChunkOutput>>
  order?: 'pre' | 'post' | null
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

export interface ResolveIdHookOption {
  handler: (
    specifier: string,
    importer: Nullable<string>,
    options: BindingHookResolveIdExtraOptions,
  ) => MaybePromise<VoidNullable<BindingHookResolveIdOutput>>
  order?: 'pre' | 'post' | null
}

export interface TransformOption {
  handler: (
    id: string,
    code: string,
  ) => MaybePromise<VoidNullable<BindingHookLoadOutput>>
  order?: 'pre' | 'post' | null
}

export interface WriteBundleOption {
  handler: (bundle: BindingOutputs) => MaybePromise<VoidNullable>
  order?: 'pre' | 'post' | null
  sequential?: boolean
}
