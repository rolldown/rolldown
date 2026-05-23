import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';
import { getOutputChunkNames } from '../../../../src/utils';

export default defineTest({
  config: {
    input: {
      index: './src/index.js',
    },
    output: {
      preserveModules: true,
      preserveModulesRoot: 'src',
    },
  },
  afterTest: (output) => {
    if (process.platform === 'win32') {
      return;
    }

    const chunkFileNames = getOutputChunkNames(output);
    expect(chunkFileNames).toContain('_module.js');
    expect(chunkFileNames).not.toContain('+module.js');
  },
});
