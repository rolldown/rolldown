import { defineTest } from 'rolldown-tests';
import { assert } from 'vitest';

export default defineTest({
  config: {
    input: ['./main.js'],
    output: {
      manualChunks(id) {
        if (id.includes('shared-codec')) return 'static-utils';
        if (id.includes('heavy')) return 'heavy';
      },
    },
  },

  afterTest(output) {
    const chunks = output.output.filter((o) => o.type === 'chunk');
    const chunkNames = chunks.map((c) => c.fileName);
    assert(chunkNames.some((name) => name.includes('static-utils')));
  },
});
