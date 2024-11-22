import { asyncFlatten } from './async-flatten'
import type { RolldownPlugin } from '../plugin'
import type { InputOptions } from '../types/input-options'
import type { OutputOptions, OutputPlugin } from '../rollup-types'

export const normalizePluginOption: {
  (plugins: InputOptions['plugins']): Promise<RolldownPlugin[]>
  (plugins: OutputOptions['plugins']): Promise<OutputPlugin[]>
  (plugins: unknown): Promise<any[]>
} = async (plugins: any) => (await asyncFlatten([plugins])).filter(Boolean)
