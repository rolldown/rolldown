import type { RenderedChunk, PreRenderedChunk } from '../binding'
import { StringOrRegExp } from './utils'

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

export type SourcemapIgnoreListOption = (
  relativeSourcePath: string,
  sourcemapPath: string,
) => boolean

export type SourcemapPathTransformOption = (
  relativeSourcePath: string,
  sourcemapPath: string,
) => string

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
  entryFileNames?: string | ChunkFileNamesFunction
  chunkFileNames?: string | ChunkFileNamesFunction
  assetFileNames?: string
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
