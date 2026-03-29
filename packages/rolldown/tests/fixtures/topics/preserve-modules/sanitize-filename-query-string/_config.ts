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
      preserveModulesRoot: '.',
      entryFileNames(chunkInfo) {
        return chunkInfo.name + '.js';
      },
    },
    plugins: [
      {
        name: 'virtual-query-string',
        resolveId(id, importer) {
          if (id === 'virtual') {
            // Simulate a Vue SFC virtual module with query parameters
            return path.dirname(importer!) + '/Comp.vue?vue&type=script&setup=true&lang.ts';
          }
        },
        load(id) {
          if (id.includes('Comp.vue?vue')) {
            return 'export const foo = 1; console.log(foo)';
          }
        },
      },
    ],
  },
  afterTest: (output) => {
    const chunks = output.output.filter((item): item is OutputChunk => item.type === 'chunk');
    expect(chunks.length).toBeGreaterThanOrEqual(2);
    for (const chunk of chunks) {
      expect(chunk.fileName).not.toContain('?');
      expect(chunk.fileName).not.toContain('&');
      expect(chunk.name).not.toContain('?');
      expect(chunk.name).not.toContain('&');
    }
  },
});
