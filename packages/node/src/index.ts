import type { InputOptions } from './options/input-options'
import type { OutputOptions } from './options/output-options'
import type { PluginOptions } from '@rolldown/node-binding'
import { RolldownOutput } from './utils'

export { rolldown } from './rolldown'

interface RollupOptions extends InputOptions {
  // This is included for compatibility with config files but ignored by rollup.rollup
  output?: OutputOptions | OutputOptions[]
}

// export types from rolldown
export type {
  RollupOptions,
  InputOptions,
  OutputOptions,
  PluginOptions as Plugin,
  RolldownOutput as RollupOutput,
}
