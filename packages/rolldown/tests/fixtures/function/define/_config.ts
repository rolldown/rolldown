import nodePath from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    define: {
      'process.env.NODE_ENV': '"production"',
    },
    external: ['node:assert'],
  },
  async afterTest(output) {
    await expect(output.output[0].code).toMatchFileSnapshot(
      nodePath.join(import.meta.dirname, 'output.snap'),
    );
  },
});
