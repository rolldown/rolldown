import { join } from 'node:path';
import { defineTest } from 'rolldown-tests';
import { viteDynamicImportVarsPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    plugins: [
      viteDynamicImportVarsPlugin({
        async resolver(id) {
          if (id.startsWith('@quoted/')) {
            return join(import.meta.dirname, 'dir', id.replace('@quoted/', 'a"b.`${literal}.'));
          }
        },
      }),
    ],
  },
  async afterTest(output) {
    for (const chunk of output.output) {
      if (chunk.type === 'chunk') {
        await expect(chunk.code).toMatchFileSnapshot(join(import.meta.dirname, 'main.js.snap'));
      }
    }
  },
});
