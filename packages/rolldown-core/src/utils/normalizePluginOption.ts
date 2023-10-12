import type {
  InputOptions,
  OutputOptions,
  Plugin,
  OutputPlugin,
} from '../rollup-types'
import { asyncFlatten } from './async-flatten'

export const normalizePluginOption: {
  (plugins: InputOptions['plugins']): Promise<Plugin[]>
  (plugins: OutputOptions['plugins']): Promise<OutputPlugin[]>
  (plugins: unknown): Promise<any[]>
} = async (plugins: any) => (await asyncFlatten([plugins])).filter(Boolean)
