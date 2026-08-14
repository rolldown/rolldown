import { defineTest } from 'rolldown-tests';
import { getOutputFileNames } from 'rolldown-tests/utils';
import { isolatedDeclarationPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    input: 'main.ts',
    plugins: [
      isolatedDeclarationPlugin({
        stripInternal: true,
      }),
    ],
  },
  async afterTest(output) {
    expect(getOutputFileNames(output)).toMatchInlineSnapshot(`
      [
        "foo.d.ts",
        "main.d.ts",
        "main.js",
      ]
    `);
  },
});
