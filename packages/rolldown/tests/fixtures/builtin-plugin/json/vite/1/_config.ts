import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { viteJsonPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    input: 'main.js',
    plugins: [
      viteJsonPlugin({ namedExports: true, stringify: 'auto' }),
      {
        name: 'test-plugin',
        async transform(code, id) {
          if (id.endsWith('.json')) {
            await expect(code).toMatchFileSnapshot(
              path.resolve(import.meta.dirname, `${path.basename(id)}.snap`),
            );
          }
        },
      },
    ],
  },
});
