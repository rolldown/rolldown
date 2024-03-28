import { RolldownOutput, RolldownOutputChunk } from './types/rolldown-output'
import type { InputOptions } from './options/input-options'
import type { OutputOptions } from './options/output-options'
import type { RolldownOptions } from './types/rolldown-options'
import type { Plugin } from './plugin'
import { defineConfig } from './utils/define-config'
import { rolldown, experimental_scan } from './rolldown'

export { defineConfig, rolldown, experimental_scan }

export type {
  RolldownOutputChunk,
  RolldownOptions,
  RolldownOutput,
  InputOptions,
  OutputOptions,
  Plugin,
}

// Exports for compatibility

export type { RolldownOutput as RollupOutput, RolldownOptions as RollupOptions }
