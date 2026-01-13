import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { viteDynamicImportVarsPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    plugins: [
      viteDynamicImportVarsPlugin({
        async resolver(id) {
          return id
            .replace('@', path.resolve(import.meta.dirname, './mods/'))
            .replace('#', path.resolve(import.meta.dirname, '../../'));
        },
        isV2: { sourcemap: false },
      }),
    ],
  },
  async afterTest(output) {
    for (const chunk of output.output) {
      if (chunk.type === 'chunk') {
        await expect(chunk.code).toMatchFileSnapshot(
          path.resolve(import.meta.dirname, 'main.js.snap'),
        );
      }
    }
  },
});
