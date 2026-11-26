import nodePath from 'node:path';
import { defineTest } from 'rolldown-tests';
import { injectPlugin } from 'rolldown/plugins';

export default defineTest({
  config: {
    input: 'main.ts',
    external: ['node:assert'],
    plugins: [
      injectPlugin({ useKey: [nodePath.join(import.meta.dirname, 'composable.ts'), 'useKey'] }),
    ],
  },
  afterTest: async () => {
    await import('./_test.mjs');
  },
});
