import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';
import { getOutputChunkNames } from '../../../../src/utils';

// A custom sanitizeFileName that expands '+' to '__' makes the sanitized root longer
// than the original, exposing byte-offset mismatches in preserveModulesRoot stripping.
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
    // preserveModulesRoot stripping relies on absolute path prefixes,
    // which differ on Windows — skip until cross-platform support is added.
    if (process.platform !== 'win32') {
      const chunkFileNames = getOutputChunkNames(output);
      expect(chunkFileNames).toContain('index.js');
      expect(chunkFileNames).toContain('helper.js');
      // Verify no unsanitized '+' leaked into output filenames
      for (const name of chunkFileNames) {
        expect(name).not.toContain('+');
        expect(name).not.toMatch(/libs\//);
      }
    }
  },
});
