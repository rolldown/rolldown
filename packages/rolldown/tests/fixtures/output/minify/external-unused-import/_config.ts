import { defineTest } from 'rolldown-tests';
import { assert, expect } from 'vitest';

export default defineTest({
  config: {
    external: ["external"],
    output: {
      minify: true,
    },
    plugins: [
      {
        name: 'test-plugin',
        renderChunk(code) {
          return code.replace('SOMETHING', 'false    ');
        },
      },
    ],
  },
  afterTest: async (output) => {
    for (const o of output.output) {
      assert(o.type === 'chunk');
      expect(o.code).not.toContain('unused');
    }
  },
});
