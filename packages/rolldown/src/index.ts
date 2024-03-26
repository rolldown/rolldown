import { RolldownOutput } from './objects/rolldown-output'
import type { InputOptions, RolldownPlugin } from './options/input-options'
import type { OutputOptions } from './options/output-options'
import type { RolldownOptions } from './types/rolldown-options'
import { defineConfig } from './utils/define-config'
import { rolldown, experimental_scan } from './rolldown'

export { defineConfig, rolldown, experimental_scan }

export type {
  RolldownOptions,
  RolldownOutput,
  InputOptions,
  OutputOptions,
  RolldownPlugin as Plugin,
}

// Exports for compatibility

export type { RolldownOutput as RollupOutput, RolldownOptions as RollupOptions }
