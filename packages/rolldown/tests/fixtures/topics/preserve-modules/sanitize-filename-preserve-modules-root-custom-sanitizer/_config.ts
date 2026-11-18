import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';
import { getOutputChunkNames } from '../../../../src/utils';

// The sanitizer turns `+` into `__`, so a sanitized id no longer starts with the root.
export default defineTest({
  config: {
    input: {
      index: './src/+libs/index.js',
    },
    output: {
      preserveModules: true,
      preserveModulesRoot: 'src/+libs',
      sanitizeFileName: (name) => name.replaceAll('+', '__'),
    },
  },
  afterTest: (output) => {
    expect(getOutputChunkNames(output).sort()).toStrictEqual(['helper.js', 'index.js']);
  },
});
