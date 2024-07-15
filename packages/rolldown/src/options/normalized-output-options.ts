import type {
  SourcemapIgnoreListOption,
  SourcemapPathTransformOption,
} from '../rollup'
import type { OutputOptions } from './output-options'
import type { RolldownPlugin } from '../plugin'
import type { RenderedChunk } from '../binding'

export type InternalModuleFormat = 'es' | 'cjs' | 'iife'

type AddonFunction = (chunk: RenderedChunk) => string | Promise<string>

export interface NormalizedOutputOptions extends OutputOptions {
  plugins: RolldownPlugin[]
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
  name: string | undefined
}
