import { asyncFlatten } from './async-flatten'
import type { RolldownPlugin, RolldownOutputPlugin } from '../plugin'
import type { InputOptions } from '../options/input-options'
import type { OutputOptions } from '../options/output-options'

export const normalizePluginOption: {
  (plugins: InputOptions['plugins']): Promise<RolldownPlugin[]>
  (plugins: OutputOptions['plugins']): Promise<RolldownOutputPlugin[]>
  (plugins: unknown): Promise<any[]>
} = async (plugins: any) => (await asyncFlatten([plugins])).filter(Boolean)
