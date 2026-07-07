import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';
import { getOutputChunkNames } from '../../../../src/utils';

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
    if (process.platform === 'win32') {
      return;
    }

    const chunkFileNames = getOutputChunkNames(output);
    expect(chunkFileNames).toContain('index.js');
    expect(chunkFileNames).toContain('helper.js');
    for (const name of chunkFileNames) {
      expect(name).not.toContain('+');
      expect(name).not.toMatch(/libs\//);
    }
  },
});
