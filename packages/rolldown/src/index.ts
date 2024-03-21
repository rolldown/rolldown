import { RolldownOutput } from './objects/rolldown-output'
import type { InputOptions, RolldownPlugin } from './options/input-options'
import type { OutputOptions } from './options/output-options'
import type { RolldownOptions } from './types/rolldown-options'
import { defineConfig } from './utils/define-config'

export { rolldown, experimental_scan } from './rolldown'

export { defineConfig }

export type {
  RolldownOptions,
  RolldownOptions as RollupOptions,
  RolldownOutput,
  RolldownOutput as RollupOutput,
  InputOptions,
  OutputOptions,
  RolldownPlugin as Plugin,
}
