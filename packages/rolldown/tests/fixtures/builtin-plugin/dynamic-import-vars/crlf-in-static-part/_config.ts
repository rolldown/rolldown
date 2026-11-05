import { join } from 'node:path';
import { defineTest } from 'rolldown-tests';
import { viteDynamicImportVarsPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    plugins: [
      {
        name: 'crlf-line-endings',
        transform(code) {
          return code.replaceAll('\n', '\r\n');
        },
      },
      viteDynamicImportVarsPlugin(),
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
