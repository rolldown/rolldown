import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const virtual = '\0virtual:entry';

// `project/tsconfig.json` fails to load. A `\0` importer is not a file, so resolving its imports
// must not search for a tsconfig.
export default defineTest({
  config: {
    cwd: path.join(import.meta.dirname, 'project'),
    input: { main: 'virtual:entry' },
    plugins: [
      {
        name: 'virtual-entry',
        resolveId(source) {
          if (source === 'virtual:entry') {
            return virtual;
          }
        },
        load(id) {
          if (id === virtual) {
            return `export { default } from './dep.js';`;
          }
        },
      },
    ],
  },
  afterTest: (output) => {
    expect(output.output[0].code).toContain('"dep"');
  },
});
