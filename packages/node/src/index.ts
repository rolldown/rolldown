import type { InputOptions, RolldownPlugin } from './options/input-options'
import type { OutputOptions } from './options/output-options'
import { RolldownOutput } from './utils'

export { rolldown, experimental_scan } from './rolldown'

interface RollupOptions extends InputOptions {
  // This is included for compatibility with config files but ignored by rollup.rollup
  output?: OutputOptions | OutputOptions[]
}

// export types from rolldown
export type {
  RollupOptions,
  InputOptions,
  OutputOptions,
  RolldownPlugin as Plugin,
  RolldownOutput as RollupOutput,
}
