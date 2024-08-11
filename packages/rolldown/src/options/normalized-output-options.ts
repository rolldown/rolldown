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
  exports: 'auto' | 'named' | 'default' | 'none'
  sourcemap: boolean | 'inline' | 'hidden'
  sourcemapIgnoreList: SourcemapIgnoreListOption
  sourcemapPathTransform: SourcemapPathTransformOption | undefined
  banner: AddonFunction
  footer: AddonFunction
  intro: AddonFunction
  outro: AddonFunction
  esModule: boolean | 'if-default-prop'
  entryFileNames: string
  chunkFileNames: string
  assetFileNames: string
  name: string | undefined
}
