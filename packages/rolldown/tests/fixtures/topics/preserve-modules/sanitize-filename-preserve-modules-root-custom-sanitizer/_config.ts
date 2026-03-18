import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';
import { getOutputChunkNames } from '../../../../src/utils';

// Tests that when stripping the preserveModulesRoot prefix from the sanitized absolute
// filename, we use the length of the *sanitized* root (not the original root).
// A custom sanitizeFileName that expands '+' to '__' makes the sanitized root longer
// than the original, so using the original length would slice at the wrong position.
export default defineTest({
  config: {
    input: {
      index: './src/+libs/index.js',
    },
    output: {
      preserveModules: true,
      preserveModulesRoot: 'src/+libs',
      // Replace '+' with '__' — this makes the sanitized preserveModulesRoot two bytes
      // longer than the original, exposing the off-by-one if we use original length.
      sanitizeFileName: (name) => name.replaceAll('+', '__'),
    },
  },
  afterTest: (output) => {
    if (process.platform !== 'win32') {
      const chunkFileNames = getOutputChunkNames(output);
      // Both chunks should appear with the sanitized prefix stripped:
      //   - 'index.js'  (not '__libs/index.js' or 's/__libs/index.js')
      //   - 'helper.js' (not '__libs/helper.js' or 's/__libs/helper.js')
      expect(chunkFileNames).toContain('index.js');
      expect(chunkFileNames).toContain('helper.js');
      // Ensure no leftover prefix from incorrect length arithmetic
      for (const name of chunkFileNames) {
        expect(name).not.toMatch(/libs\//);
      }
    }
  },
});
