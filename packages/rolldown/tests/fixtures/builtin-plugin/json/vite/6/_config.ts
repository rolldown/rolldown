import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { viteJsonPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    input: 'main.js',
    plugins: [
      viteJsonPlugin({ namedExports: false, stringify: true, minify: true }),
      {
        name: 'test-plugin',
        async transform(code, id) {
          if (id.endsWith('data.json')) {
            await expect(code).toMatchFileSnapshot(
              path.resolve(import.meta.dirname, 'data.json.snap'),
            );
          }
        },
      },
    ],
  },
});
