import { unsupported } from '../utils/misc'
import type { BindingNormalizedOptions } from '../binding'
import { ChunkFileNamesFunction, OutputOptions } from './output-options'

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
}

function mapFunctionOption<T>(
  option: T | undefined,
  name: string,
): T | ReturnType<typeof unsupported> {
  return typeof option === 'undefined'
    ? unsupported(
        `You should not take \`NormalizedOutputOptions#${name}\` and call it directly`,
      )
    : option
}

// TODO: I guess we make these getters enumerable so it act more like a plain object
export class NormalizedOutputOptionsImpl implements NormalizedOutputOptions {
  inner: BindingNormalizedOptions

  constructor(inner: BindingNormalizedOptions) {
    this.inner = inner
  }

  get dir() {
    return this.inner.dir ?? undefined
  }

  get entryFileNames() {
    return mapFunctionOption(this.inner.entryFilenames, 'entryFileNames')
  }

  get chunkFileNames() {
    return mapFunctionOption(this.inner.chunkFilenames, 'chunkFileNames')
  }

  get assetFileNames() {
    return this.inner.assetFilenames
  }

  get format() {
    return this.inner.format
  }

  get exports() {
    return this.inner.exports
  }

  get sourcemap() {
    return this.inner.sourcemap
  }

  get cssEntryFileNames() {
    return mapFunctionOption(this.inner.cssEntryFilenames, 'cssEntryFileNames')
  }

  get cssChunkFileNames() {
    return mapFunctionOption(this.inner.cssChunkFilenames, 'cssChunkFileNames')
  }

  get shimMissingExports() {
    return this.inner.shimMissingExports
  }

  get name() {
    return this.inner.name ?? undefined
  }

  get file() {
    return this.inner.file ?? undefined
  }

  get inlineDynamicImports() {
    return this.inner.inlineDynamicImports
  }

  get externalLiveBindings() {
    return this.inner.externalLiveBindings
  }

  get banner() {
    return mapFunctionOption(this.inner.banner, 'banner') ?? undefined
  }

  get footer() {
    return mapFunctionOption(this.inner.footer, 'footer') ?? undefined
  }

  get intro() {
    return mapFunctionOption(this.inner.intro, 'intro') ?? undefined
  }

  get outro() {
    return mapFunctionOption(this.inner.outro, 'outro') ?? undefined
  }

  get esModule() {
    return this.inner.esModule
  }
}
