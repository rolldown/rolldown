import type { InputCliOptions } from '../../options/input-options';
import type { OutputCliOptions } from '../../options/output-options';

export interface CliOptions extends InputCliOptions, OutputCliOptions {
  config?: string | boolean;
  help?: boolean;
  version?: boolean;
  watch?: boolean;
  environment?: string | string[];
  silent?: boolean;
}

export interface OptionConfig {
  abbreviation?: string;
  description?: string;
  default?: string | boolean;
  hint?: string;
  reverse?: boolean;
  /**
   * Whether this option requires a value.
   * If true, an error will be thrown if the option is used without a value.
   */
  requireValue?: boolean;
}

export const alias: Partial<Record<keyof CliOptions, OptionConfig>> = {
  config: {
    abbreviation: 'c',
    hint: 'filename',
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
    requireValue: true,
  },
  file: {
    abbreviation: 'o',
    requireValue: true,
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
  preserveEntrySignatures: {
    default: 'strict',
    reverse: true,
  },
  moduleTypes: {
    hint: 'types',
  },
};
