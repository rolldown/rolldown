import { CliOptions } from './schema'

export interface OptionConfig {
  abbreviation?: string
  description?: string
  default?: string | boolean
  hint?: string
  reverse?: boolean
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
  file: {
    abbreviation: 'o',
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
    default: true,
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
    reverse: true,
  },
  treeshake: {
    default: true,
    reverse: true,
  },
  moduleTypes: {
    hint: 'types',
  },
}
