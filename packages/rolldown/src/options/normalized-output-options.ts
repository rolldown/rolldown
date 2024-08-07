import type {
  SourcemapIgnoreListOption,
  SourcemapPathTransformOption,
} from '../rollup'
import type { OutputOptions } from './output-options'
import type { RolldownPlugin } from '../plugin'
import type { PreRenderedChunk, RenderedChunk } from '../binding'

export type InternalModuleFormat = 'es' | 'cjs' | 'iife'

type AddonFunction = (chunk: RenderedChunk) => string | Promise<string>
type ChunkFileNamesOption =
  | string
  | ((chunk: PreRenderedChunk) => string)
  | undefined

export interface NormalizedOutputOptions extends OutputOptions {
  plugins: RolldownPlugin[]
  dir: string | undefined
  format: InternalModuleFormat
  exports: 'auto' | 'named' | 'default' | 'none'
  sourcemap: boolean | 'inline' | 'hidden'
  sourcemapIgnoreList: SourcemapIgnoreListOption
  sourcemapPathTransform: SourcemapPathTransformOption | undefined
  banner: AddonFunction
  footer: AddonFunction
  intro: AddonFunction
  outro: AddonFunction
  esModule: boolean | 'if-default-prop'
  entryFileNames: ChunkFileNamesOption
  chunkFileNames: ChunkFileNamesOption
  assetFileNames: string
  name: string | undefined
}
