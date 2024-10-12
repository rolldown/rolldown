import { CliOptions } from './schema'

export interface OptionConfig {
  abbreviation?: string
  description?: string
  default?: string | boolean
  hint?: string
}

export const alias: Partial<Record<keyof CliOptions, OptionConfig>> = {
  config: {
    abbreviation: 'c',
    hint: 'filename',
    default: 'rolldown.config.js',
  },
  help: {
    abbreviation: 'h',
  },
  version: {
    abbreviation: 'v',
  },
  watch: {
    abbreviation: 'w',
  },
  dir: {
    abbreviation: 'd',
  },
  external: {
    abbreviation: 'e',
  },
  format: {
    abbreviation: 'f',
  },
  name: {
    abbreviation: 'n',
  },
  globals: {
    abbreviation: 'g',
  },
  sourcemap: {
    abbreviation: 's',
    default: false,
  },
  minify: {
    abbreviation: 'm',
  },
  platform: {
    abbreviation: 'p',
  },
  assetFileNames: {
    hint: 'name',
  },
  chunkFileNames: {
    hint: 'name',
  },
  entryFileNames: {
    hint: 'name',
  },
  externalLiveBindings: {
    default: true,
  },
  treeshake: {
    default: true,
  },
  moduleTypes: {
    hint: 'types',
  },
}
