import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const calls: string[] = [];

export default defineTest({
  sequential: true,
  config: {
    input: './main.js',
    experimental: {
      importDefer: true,
    },
    plugins: [
      {
        name: 'test-plugin',
        resolveId(id, _importer, options) {
          if (id === './dep.js') {
            calls.push(`${options.kind}:${options.phase}`);
          }
        },
      },
    ],
  },
  afterTest: () => {
    // An import and a deferred import of the same specifier are resolved separately.
    expect(calls.sort()).toStrictEqual([
      'dynamic-import:defer',
      'import-statement:defer',
      'import-statement:evaluation',
    ]);
  },
});
