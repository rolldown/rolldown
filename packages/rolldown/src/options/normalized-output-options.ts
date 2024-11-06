import type { SourcemapIgnoreListOption } from '../rollup'
import type { AddonFunction, OutputOptions } from '../types/output-options'
import type { RolldownPlugin } from '../plugin'

export type InternalModuleFormat = 'es' | 'cjs' | 'iife' | 'umd'

export interface NormalizedOutputOptions extends OutputOptions {
  plugins: RolldownPlugin[]
  format: InternalModuleFormat
  exports: 'auto' | 'named' | 'default' | 'none'
  sourcemap: boolean | 'inline' | 'hidden'
  sourcemapIgnoreList: SourcemapIgnoreListOption
  banner: AddonFunction
  footer: AddonFunction
  intro: AddonFunction
  outro: AddonFunction
  esModule: boolean | 'if-default-prop'
  assetFileNames: string
  inlineDynamicImports: boolean
}
