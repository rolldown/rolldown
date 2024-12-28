import type { StringOrRegExp } from '../types/utils'
import type { PreRenderedChunk } from '../binding'
import {
  SourcemapIgnoreListOption,
  SourcemapPathTransformOption,
} from '../types/misc'
import { RolldownOutputPluginOption } from '../plugin'
import { RenderedChunk } from '../types/rolldown-output'

export type ModuleFormat =
  | 'es'
  | 'cjs'
  | 'esm'
  | 'module'
  | 'commonjs'
  | 'iife'
  | 'umd'
  | 'experimental-app'

export type AddonFunction = (chunk: RenderedChunk) => string | Promise<string>

export type ChunkFileNamesFunction = (chunkInfo: PreRenderedChunk) => string

export type GlobalsFunction = (name: string) => string

export type ESTarget =
  | 'ES2015'
  | 'ES2016'
  | 'ES2017'
  | 'ES2018'
  | 'ES2019'
  | 'ES2020'
  | 'ES2021'
  | 'ES2022'
  | 'ES2023'
  | 'ES2024'
  | 'ESNext'

export interface OutputOptions {
  dir?: string
  file?: string
  exports?: 'auto' | 'named' | 'default' | 'none'
  hashCharacters?: 'base64' | 'base36' | 'hex'
  /**
   * Expected format of generated code.
   * - `'es'`, `'esm'` and `'module'` are the same format, all stand for ES module.
   * - `'cjs'` and `'commonjs'` are the same format, all stand for CommonJS module.
   * - `'iife'` stands for [Immediately Invoked Function Expression](https://developer.mozilla.org/en-US/docs/Glossary/IIFE).
   * - `'umd'` stands for [Universal Module Definition](https://github.com/umdjs/umd).
   *
   * @default 'esm'
   */
  format?: ModuleFormat
  sourcemap?: boolean | 'inline' | 'hidden'
  sourcemapIgnoreList?: boolean | SourcemapIgnoreListOption
  sourcemapPathTransform?: SourcemapPathTransformOption
  banner?: string | AddonFunction
  footer?: string | AddonFunction
  intro?: string | AddonFunction
  outro?: string | AddonFunction
  extend?: boolean
  esModule?: boolean | 'if-default-prop'
  assetFileNames?: string
  entryFileNames?: string | ChunkFileNamesFunction
  chunkFileNames?: string | ChunkFileNamesFunction
  cssEntryFileNames?: string | ChunkFileNamesFunction
  cssChunkFileNames?: string | ChunkFileNamesFunction
  minify?: boolean
  name?: string
  globals?: Record<string, string> | GlobalsFunction
  externalLiveBindings?: boolean
  inlineDynamicImports?: boolean
  advancedChunks?: {
    minSize?: number
    minShareCount?: number
    groups?: {
      name: string
      test?: StringOrRegExp
      priority?: number
      minSize?: number
      minShareCount?: number
    }[]
  }
  /**
   * Control comments in the output.
   *
   * - `none`: no comments
   * - `preserve-legal`: preserve comments that contain `@license`, `@preserve` or starts with `//!` `/*!`
   */
  comments?: 'none' | 'preserve-legal'
  plugins?: RolldownOutputPluginOption
  polyfillRequire?: boolean
  target?: ESTarget
}

interface OverwriteOutputOptionsForCli {
  banner?: string
  footer?: string
  intro?: string
  outro?: string
  esModule?: boolean
  globals?: Record<string, string>
  advancedChunks?: {
    minSize?: number
    minShareCount?: number
  }
  target?: string
}

export type OutputCliOptions = Omit<
  OutputOptions,
  | keyof OverwriteOutputOptionsForCli
  | 'sourcemapIgnoreList'
  | 'sourcemapPathTransform'
> &
  OverwriteOutputOptionsForCli
