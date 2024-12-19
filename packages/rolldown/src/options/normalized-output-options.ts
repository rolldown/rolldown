import { unsupported } from '../utils/misc'
import type { BindingNormalizedOptions } from '../binding'
import type {
  SourcemapIgnoreListOption,
  SourcemapPathTransformOption,
} from '../types/misc'
import type {
  ChunkFileNamesFunction,
  GlobalsFunction,
  OutputOptions,
} from './output-options'

export type InternalModuleFormat = 'es' | 'cjs' | 'iife' | 'umd' | 'app'

export interface NormalizedOutputOptions {
  name: string | undefined
  file: string | undefined
  dir: string | undefined
  entryFileNames: string | ChunkFileNamesFunction
  chunkFileNames: string | ChunkFileNamesFunction
  assetFileNames: string
  format: InternalModuleFormat
  exports: NonNullable<OutputOptions['exports']>
  sourcemap: boolean | 'inline' | 'hidden'
  cssEntryFileNames: string | ChunkFileNamesFunction
  cssChunkFileNames: string | ChunkFileNamesFunction
  inlineDynamicImports: boolean
  externalLiveBindings: boolean
  banner: OutputOptions['banner']
  footer: OutputOptions['footer']
  intro: OutputOptions['intro']
  outro: OutputOptions['outro']
  esModule: boolean | 'if-default-prop'
  extend: boolean
  globals: Record<string, string> | GlobalsFunction
  hashCharacters: 'base64' | 'base36' | 'hex'
  sourcemapDebugIds: boolean
  sourcemapIgnoreList: SourcemapIgnoreListOption | undefined
  sourcemapPathTransform: SourcemapPathTransformOption | undefined
  minify: boolean
  comments: 'none' | 'preserve-legal'
  polyfillRequire: boolean
}

function mapFunctionOption<T>(
  option: T | undefined,
  name: string,
): T | (() => never) {
  return typeof option === 'undefined'
    ? () => {
        unsupported(
          `You should not take \`NormalizedOutputOptions#${name}\` and call it directly`,
        )
      }
    : option
}

type UnsupportedFnRet = () => never

// TODO: I guess we make these getters enumerable so it act more like a plain object
export class NormalizedOutputOptionsImpl implements NormalizedOutputOptions {
  inner: BindingNormalizedOptions

  constructor(inner: BindingNormalizedOptions) {
    this.inner = inner
  }

  get dir(): string | undefined {
    return this.inner.dir ?? undefined
  }

  get entryFileNames(): string | UnsupportedFnRet {
    return mapFunctionOption(this.inner.entryFilenames, 'entryFileNames')
  }

  get chunkFileNames(): string | UnsupportedFnRet {
    return mapFunctionOption(this.inner.chunkFilenames, 'chunkFileNames')
  }

  get assetFileNames(): string {
    return this.inner.assetFilenames
  }

  get format(): 'es' | 'cjs' | 'app' | 'iife' | 'umd' {
    return this.inner.format
  }

  get exports(): 'default' | 'named' | 'none' | 'auto' {
    return this.inner.exports
  }

  get sourcemap(): boolean | 'inline' | 'hidden' {
    return this.inner.sourcemap
  }

  get cssEntryFileNames(): string | UnsupportedFnRet {
    return mapFunctionOption(this.inner.cssEntryFilenames, 'cssEntryFileNames')
  }

  get cssChunkFileNames(): string | UnsupportedFnRet {
    return mapFunctionOption(this.inner.cssChunkFilenames, 'cssChunkFileNames')
  }

  get shimMissingExports(): boolean {
    return this.inner.shimMissingExports
  }

  get name(): string | undefined {
    return this.inner.name ?? undefined
  }

  get file(): string | undefined {
    return this.inner.file ?? undefined
  }

  get inlineDynamicImports(): boolean {
    return this.inner.inlineDynamicImports
  }

  get externalLiveBindings(): boolean {
    return this.inner.externalLiveBindings
  }

  get banner(): string | UnsupportedFnRet | undefined {
    return mapFunctionOption(this.inner.banner, 'banner') ?? undefined
  }

  get footer(): string | UnsupportedFnRet | undefined {
    return mapFunctionOption(this.inner.footer, 'footer') ?? undefined
  }

  get intro(): string | UnsupportedFnRet | undefined {
    return mapFunctionOption(this.inner.intro, 'intro') ?? undefined
  }

  get outro(): string | UnsupportedFnRet | undefined {
    return mapFunctionOption(this.inner.outro, 'outro') ?? undefined
  }

  get esModule(): boolean | 'if-default-prop' {
    return this.inner.esModule
  }

  get extend(): boolean {
    return this.inner.extend
  }

  get globals(): Record<string, string> | UnsupportedFnRet {
    return mapFunctionOption(this.inner.globals, 'globals')
  }

  get hashCharacters(): 'base64' | 'base36' | 'hex' {
    return this.inner.hashCharacters
  }

  get sourcemapDebugIds(): boolean {
    return this.inner.sourcemapDebugIds
  }

  get sourcemapIgnoreList(): UnsupportedFnRet | undefined {
    return mapFunctionOption(void 0, 'sourcemapIgnoreList')
  }

  get sourcemapPathTransform(): UnsupportedFnRet | undefined {
    return mapFunctionOption(void 0, 'sourcemapPathTransform')
  }

  get minify(): boolean {
    return this.inner.minify
  }

  get comments(): 'none' | 'preserve-legal' {
    return this.inner.comments
  }

  get polyfillRequire(): boolean {
    return this.inner.polyfillRequire
  }
}
