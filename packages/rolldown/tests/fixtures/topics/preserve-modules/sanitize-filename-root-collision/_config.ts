import path from 'node:path';
import type { OutputChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// `foo+bar` and `foo#bar` both sanitize to `foo_bar`. Only the module under the raw
// `preserveModulesRoot` gets the root stripped.
export default defineTest({
  sequential: true,
  config: {
    input: {
      entry: './entry.js',
    },
    output: {
      dir: 'dist',
      preserveModules: true,
      preserveModulesRoot: 'foo+bar',
    },
    plugins: [
      {
        name: 'virtual-sanitize-root-collision',
        resolveId(id, importer) {
          if (!importer) return null;
          const importerDir = path.dirname(importer);
          if (id === 'inside') {
            return path.join(importerDir, 'foo+bar', 'inside.js');
          }
          if (id === 'outside') {
            return path.join(importerDir, 'foo#bar', 'outside.js');
          }
        },
        load(id) {
          if (id.endsWith(`${path.sep}foo+bar${path.sep}inside.js`)) {
            return 'export const inside = "inside"; console.log(inside);';
          }
          if (id.endsWith(`${path.sep}foo#bar${path.sep}outside.js`)) {
            return 'export const outside = "outside"; console.log(outside);';
          }
        },
      },
    ],
  },
  afterTest: (output) => {
    const chunks = output.output.filter((item): item is OutputChunk => item.type === 'chunk');
    expect(Object.fromEntries(chunks.map((chunk) => [chunk.fileName, chunk.name]))).toStrictEqual({
      'entry.js': 'entry',
      'foo_bar/outside.js': 'foo_bar/outside',
      'inside.js': 'inside',
    });
  },
});
