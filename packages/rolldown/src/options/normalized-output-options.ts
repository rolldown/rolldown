import type {
  SourcemapIgnoreListOption,
  SourcemapPathTransformOption,
} from '../rollup'
import type { OutputOptions } from './output-options'
import type { Plugin, ParallelPlugin } from '../plugin'
import type { RenderedChunk } from '../binding'

type InternalModuleFormat = 'es' | 'cjs'

type AddonFunction = (chunk: RenderedChunk) => string | Promise<string>

export interface NormalizedOutputOptions extends OutputOptions {
  plugins: (Plugin | ParallelPlugin)[]
  dir: string | undefined
  format: InternalModuleFormat
  exports: 'named'
  sourcemap: boolean | 'inline' | 'hidden'
  sourcemapIgnoreList: SourcemapIgnoreListOption
  sourcemapPathTransform: SourcemapPathTransformOption | undefined
  banner: AddonFunction
  footer: AddonFunction
  entryFileNames: string
  chunkFileNames: string
  assetFileNames: string
}
