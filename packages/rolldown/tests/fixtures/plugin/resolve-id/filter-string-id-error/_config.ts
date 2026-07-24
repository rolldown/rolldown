import { id, include } from '@rolldown/pluginutils';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: './main.js',
    plugins: [
      {
        name: 'test-plugin',
        resolveId: {
          // A string `id` is compiled to a glob, which is meaningless for the
          // `resolveId` specifier (it's raw import text, not a path); it must be a
          // RegExp. This should throw at plugin-normalization time.
          filter: [include(id('**/main.js'))],
          handler(_id) {
            return null;
          },
        },
      },
    ],
  },
  catchError(e: any) {
    expect(e).toBeInstanceOf(Error);
    expect(e.message).toMatchInlineSnapshot(
      `"A string \`id\` filter is not supported for the \`resolveId\` hook, because its \`id\` is the import specifier rather than a resolved path. Use a RegExp instead."`,
    );
  },
});
