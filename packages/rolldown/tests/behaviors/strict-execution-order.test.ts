import type { OutputChunk, Plugin } from 'rolldown';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

test('keeps live external imports in shared strict entries', async () => {
  const modules: Record<string, string> = {
    'entry-a': `import { value } from 'first'; console.log(value); export { read } from 'first';`,
    'entry-b': `import { value } from 'second'; console.log(value); export { read } from 'second';`,
    first: `export { value, read } from 'shared';`,
    second: `export { value, read } from 'shared';`,
    shared: `export { value, read } from 'leaf';`,
    leaf: `import { first, second } from 'dep'; export const value = first(second); export function read() { return value; }`,
  };

  const plugin: Plugin = {
    name: 'virtual',
    resolveId(id) {
      if (id in modules) return id;
    },
    load(id) {
      return modules[id];
    },
  };

  const bundle = await rolldown({
    input: {
      a: 'entry-a',
      b: 'entry-b',
    },
    external: ['dep'],
    platform: 'browser',
    plugins: [plugin],
    treeshake: {
      moduleSideEffects: false,
    },
  });
  const result = await bundle.generate({
    format: 'es',
    strictExecutionOrder: true,
    minify: false,
  });
  await bundle.close();

  const shared = result.output.find(
    (chunk): chunk is OutputChunk => chunk.type === 'chunk' && chunk.code.includes('first(second)'),
  );

  expect(shared).toBeDefined();
  expect(shared!.code).toContain('import { first, second } from "dep";');
});
