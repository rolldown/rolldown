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
            calls.push(options.kind);
          }
        },
      },
    ],
  },
  afterTest: () => {
    // An import and a deferred import of the same specifier share one `resolveId` call. In the
    // spec, both get the same module, because `ModuleRequestsKeyEqual` compares only the specifier
    // and the attributes. `import()` and `import.defer()` share a second call, because their kind
    // is `dynamic-import`.
    expect(calls.sort()).toStrictEqual(['dynamic-import', 'import-statement']);
  },
});
