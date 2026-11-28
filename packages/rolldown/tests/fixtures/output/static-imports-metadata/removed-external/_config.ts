import type { OutputChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    external: ['unused-external'],
    treeshake: { moduleSideEffects: (id) => id !== 'unused-external' },
    plugins: [
      {
        name: 'check-imports',
        renderChunk(code, chunk) {
          expect(code).not.toContain('unused-external');
          expect(chunk.imports).toStrictEqual([]);
        },
      },
    ],
  },
  afterTest(output) {
    expect((output.output[0] as OutputChunk).imports).toStrictEqual([]);
  },
});
