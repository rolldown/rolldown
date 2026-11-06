import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const entry = 'virtual:posts/helpers';
const virtual = `\0${entry}`;

// `project/tsconfig.json` fails to load. A `\0` importer is a virtual module with no location, so
// resolving its imports must not use a tsconfig, and the `/` in the id checks that `./dep.js`
// resolves from cwd rather than from `<cwd>/\0virtual:posts/`.
export default defineTest({
  config: {
    cwd: path.join(import.meta.dirname, 'project'),
    input: { main: entry },
    plugins: [
      {
        name: 'virtual-entry',
        resolveId(source) {
          if (source === entry) {
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
