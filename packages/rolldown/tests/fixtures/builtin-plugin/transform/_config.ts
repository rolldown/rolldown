import { stripAnsi } from 'consola/utils';
import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { viteTransformPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    plugins: [
      viteTransformPlugin({
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
