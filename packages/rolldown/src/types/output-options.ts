import type { StringOrRegExp } from './utils'
import type { RenderedChunk, PreRenderedChunk } from '../binding'
import {
  SourcemapIgnoreListOption,
  SourcemapPathTransformOption,
} from '../rollup'

export type ModuleFormat =
  | 'es'
  | 'cjs'
  | 'esm'
  | 'module'
  | 'commonjs'
  | 'iife'
  | 'umd'

export type AddonFunction = (chunk: RenderedChunk) => string | Promise<string>

export type ChunkFileNamesFunction = (chunkInfo: PreRenderedChunk) => string

export interface OutputOptions {
  dir?: string
  file?: string
  exports?: 'auto' | 'named' | 'default' | 'none'
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
  globals?: Record<string, string>
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
}

interface OverwriteOutputOptionsForCli {
  banner?: string
  footer?: string
  intro?: string
  outro?: string
  esModule?: boolean
  advancedChunks?: {
    minSize?: number
    minShareCount?: number
  }
}

export type OutputCliOptions = Omit<
  OutputOptions,
  | keyof OverwriteOutputOptionsForCli
  | 'sourcemapIgnoreList'
  | 'sourcemapPathTransform'
> &
  OverwriteOutputOptionsForCli
