import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';
import { getOutputChunkNames } from '../../../../src/utils';

// Test that sanitizeFileName is applied to chunk filenames when preserveModulesRoot is set.
// When a module's absolute path starts with the preserveModulesRoot, the relative portion
// (used as chunk name) must also be sanitized. This was a bug where the unsanitized path
// was used instead of the sanitized one.
// See: https://github.com/rolldown/rolldown/issues/7554
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
    if (process.platform !== 'win32') {
      const chunkFileNames = getOutputChunkNames(output);
      // The '+module.js' file should be sanitized to '_module.js'
      expect(chunkFileNames).toContain('_module.js');
      expect(chunkFileNames).not.toContain('+module.js');
    }
  },
});
