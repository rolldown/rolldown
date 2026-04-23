import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

// Regression test for the case where another plugin marks a specifier with
// a copy-configured extension as `external` during `resolveId`. The copy
// plugin must honor the external flag and not attempt to read the path from
// disk.
//
// The motivating real-world scenario is `rolldown-plugin-dts`, which treats
// TypeScript ambient-module glob specifiers like `typeof import("*.jpg")`
// as external. Before the fix the copy plugin would try to read `*.jpg`
// and fail with `Failed to read copy module *.jpg: No such file or
// directory`.
test('copy moduleType honors `external: true` from other plugins', async () => {
  const ENTRY = 'virtual:entry';
  const GLOB_SPECIFIER = '*.txt';

  const bundle = await rolldown({
    input: ENTRY,
    cwd: process.cwd(),
    moduleTypes: {
      '.txt': 'copy',
    },
    plugins: [
      {
        name: 'virtual',
        resolveId(source) {
          if (source === ENTRY) return { id: ENTRY };
          if (source === GLOB_SPECIFIER) {
            return { id: GLOB_SPECIFIER, external: true };
          }
          return null;
        },
        load(id) {
          if (id === ENTRY) {
            return `import '${GLOB_SPECIFIER}';\nexport {};`;
          }
          return null;
        },
      },
    ],
  });

  const { output } = await bundle.generate({ format: 'esm' });
  await bundle.close();

  const chunk = output.find((o) => o.type === 'chunk');
  expect(chunk).toBeDefined();
  expect(chunk!.code).toContain(GLOB_SPECIFIER);
});
