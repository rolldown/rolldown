import type { OutputOptions, OutputPlugin } from '../rollup-types'
import type { InputOptions } from '../options/input-options'
import { asyncFlatten } from './async-flatten'
import type { ParallelPlugin, Plugin } from '../plugin'

export const normalizePluginOption: {
  (plugins: InputOptions['plugins']): Promise<(ParallelPlugin | Plugin)[]>
  (plugins: OutputOptions['plugins']): Promise<OutputPlugin[]>
  (plugins: unknown): Promise<any[]>
} = async (plugins: any) => (await asyncFlatten([plugins])).filter(Boolean)
