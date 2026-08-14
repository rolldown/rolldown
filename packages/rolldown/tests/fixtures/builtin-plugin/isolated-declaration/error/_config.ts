import { stripAnsi } from 'consola/utils';
import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { isolatedDeclarationPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    input: 'main.ts',
    plugins: [isolatedDeclarationPlugin()],
  },
  async catchError(err: any) {
    await expect(stripAnsi(err.toString())).toMatchFileSnapshot(
      path.resolve(import.meta.dirname, 'main.ts.snap'),
    );
  },
});
