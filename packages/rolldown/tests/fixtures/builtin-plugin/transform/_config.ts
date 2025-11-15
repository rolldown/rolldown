import { stripAnsi } from 'consola/utils';
import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { transformPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [
      transformPlugin({
        root: __dirname,
        jsxRefreshInclude: [/.abc$/],
        transformOptions: {
          jsx: {
            throwIfNamespace: true,
          },
        },
      }),
    ],
  },
  async catchError(err: any) {
    await expect(stripAnsi(err.toString())).toMatchFileSnapshot(
      path.resolve(import.meta.dirname, 'main.js.snap'),
    );
  },
});
