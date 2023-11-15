import type { InputOptions } from './options/input-options'
import type { OutputOptions } from './options/output-options'
export { rolldown } from './rolldown'

interface RollupOptions extends InputOptions {
	// This is included for compatibility with config files but ignored by rollup.rollup
	output?: OutputOptions | OutputOptions[];
}

// export types from rolldown
export type { RollupOptions, InputOptions, OutputOptions }

// export types from rollup
export type { RollupOutput, Plugin } from 'rollup'
