import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const virtualId = path.join(import.meta.dirname, 'a:b.js');

export default defineTest({
  sequential: true,
  config: {
    input: './entry.js',
    output: {
      dir: 'dist',
      preserveModules: true,
      preserveModulesRoot: '.',
    },
    plugins: [
      {
        name: 'virtual-colon-filename',
        resolveId(id) {
          if (id === 'virtual') {
            return virtualId;
          }
        },
        load(id) {
          if (id === virtualId) {
            return 'export const value = 1;';
          }
        },
        generateBundle(_, bundle) {
          // Check before writing the drive-relative filename on Windows.
          expect(Object.keys(bundle).sort()).toStrictEqual(['a_b.js', 'entry.js']);
        },
      },
    ],
  },
});
