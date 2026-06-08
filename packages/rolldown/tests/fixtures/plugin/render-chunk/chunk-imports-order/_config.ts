import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        renderChunk(_code, chunk) {
          if (chunk.fileName === 'main.js') {
            expect(chunk.imports).toEqual(['z.js', 'a.js']);
          }
        },
      },
    ],
    output: {
      preserveModules: true,
    },
  },
});
