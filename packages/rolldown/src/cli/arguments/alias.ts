import { InputOption } from '../../options/input-options'
import { OutputOptions } from '../../options/output-options'

export interface OptionConfig {
  abbreviation?: string
  description: string
  defaultTrue?: boolean
}

export const alias: Partial<
  Record<keyof InputOption & OutputOptions, OptionConfig>
> = {
  dir: {
    abbreviation: 'd',
    description: 'The directory to output files',
  },
  external: {
    abbreviation: 'e',
    description: 'Modules to exclude in the bundle (comma separated)',
  },
  format: {
    abbreviation: 'f',
    description:
      'The format of the generated bundle (accept esm, cjs, iife, amd, umd, system)',
  },
  name: {
    abbreviation: 'n',
    description: 'The name of the generated bundle (for iife and umd format)',
  },
  globals: {
    abbreviation: 'g',
    description:
      'Global variables to replace with (comma separated with list of module-id:global-key)',
  },
  sourcemap: {
    abbreviation: 's',
    description: 'Generate sourcemap',
  },
  minify: {
    abbreviation: 'm',
    description: 'Minify the generated bundle',
  },
  treeshake: {
    description: 'Disable treeshaking',
    defaultTrue: true,
  },
}
