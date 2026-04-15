import type { OutputChunk } from 'rolldown';
import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

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
        name: 'virtual-sanitize-root-collision-default',
        resolveId(id, importer) {
          if (!importer) return null;

          const importerDir = path.dirname(importer);
          if (id === 'inside') {
            return path.join(importerDir, 'foo+bar', 'inside.js');
          }
          if (id === 'outside') {
            return path.join(importerDir, 'foo#bar', 'outside.js');
          }
          return null;
        },
        load(id) {
          if (id.includes(`${path.sep}foo+bar${path.sep}inside.js`)) {
            return 'export const inside = "inside"';
          }
          if (id.includes(`${path.sep}foo#bar${path.sep}outside.js`)) {
            return 'export const outside = "outside"';
          }
          return null;
        },
      },
    ],
  },
  afterTest: (output) => {
    if (process.platform === 'win32') {
      return;
    }

    const chunks = output.output.filter((item): item is OutputChunk => item.type === 'chunk');
    expect(chunks.map((chunk) => chunk.fileName)).toContain('inside.js');
    expect(chunks.map((chunk) => chunk.fileName)).toContain('foo_bar/outside.js');
    expect(chunks.map((chunk) => chunk.fileName)).not.toContain('outside.js');

    const outsideChunk = chunks.find((chunk) => chunk.fileName === 'foo_bar/outside.js');
    expect(outsideChunk?.name).toBe('foo_bar/outside');
  },
});
