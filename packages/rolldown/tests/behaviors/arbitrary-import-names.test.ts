import vm from 'node:vm';

import { originalPositionFor, TraceMap } from '@jridgewell/trace-mapping';
import type { Plugin } from 'rolldown';
import { rolldown } from 'rolldown';
import { describe, expect, test } from 'vitest';

const names = ['', 'a-b', 'a"b', String.raw`a\nb`, 'for'];

function virtualPlugin(modules: Record<string, string>): Plugin {
  return {
    name: 'virtual',
    resolveId(id) {
      if (id in modules) return id;
    },
    load(id) {
      return modules[id];
    },
  };
}

describe.each([false, 'dce-only', true] as const)('minify: %s', (minify) => {
  describe.each(['cjs', 'iife', 'umd'] as const)('format: %s', (format) => {
    test.each(['external', 'commonjs'] as const)(
      'reads live properties and calls imports from %s',
      async (dependencyType) => {
        const imports = names.map((name, i) => `${JSON.stringify(name)} as value${i}`).join(', ');
        const calls = names.map((_, i) => `value${i}()`).join(', ');
        const bundle = await rolldown({
          input: 'entry',
          external: dependencyType === 'external' ? ['dep'] : [],
          plugins: [
            virtualPlugin({
              entry: `import { ${imports} } from 'dep'; globalThis.read = () => [${calls}];`,
              dep: 'module.exports = globalThis.dependency;',
            }),
          ],
        });
        try {
          const { output } = await bundle.generate({
            format,
            minify,
            globals: { dep: 'dependency' },
          });
          const chunk = output[0];
          expect(chunk.type).toBe('chunk');
          if (chunk.type !== 'chunk') return;

          const dependency: Record<string, (this: unknown) => number> = {};
          const context = {
            dependency,
            require: () => dependency,
            read: undefined as (() => number[]) | undefined,
          };
          names.forEach((name, i) => {
            dependency[name] = function () {
              return this === undefined ? i : -1;
            };
          });
          vm.runInNewContext(chunk.code, context);
          expect(context.read!()).toEqual(names.map((_, i) => i));
          names.forEach((name, i) => {
            dependency[name] = function () {
              return this === undefined ? i + 10 : -1;
            };
          });
          expect(context.read!()).toEqual(names.map((_, i) => i + 10));
        } finally {
          await bundle.close();
        }
      },
    );
  });

  test('re-exports arbitrary CommonJS properties as ES module bindings', async () => {
    const exports = names.map((name) => JSON.stringify(name)).join(', ');
    const values = Object.fromEntries(names.map((name, i) => [name, i]));
    const bundle = await rolldown({
      input: 'entry',
      plugins: [
        virtualPlugin({
          entry: `export { ${exports} } from 'dep';`,
          dep: `module.exports = ${JSON.stringify(values)};`,
        }),
      ],
    });
    try {
      const { output } = await bundle.generate({ format: 'es', minify });
      const chunk = output[0];
      expect(chunk.type).toBe('chunk');
      if (chunk.type !== 'chunk') return;
      const namespace = await import(`data:text/javascript,${encodeURIComponent(chunk.code)}`);
      expect({ ...namespace }).toEqual(values);
    } finally {
      await bundle.close();
    }
  });

  test('reads arbitrary properties from a shared CommonJS chunk', async () => {
    const bundle = await rolldown({
      input: ['entry-a', 'entry-b'],
      plugins: [
        virtualPlugin({
          'entry-a':
            "import { value, update } from 'shared'; export const read = () => value; export const increment = () => update();",
          'entry-b':
            "import { value, update } from 'shared'; export const read = () => value; export const increment = () => update();",
          shared: 'export { "a-b" as value, update } from "dep";',
          dep: 'exports["a-b"] = 42; exports.update = () => { exports["a-b"]++; };',
        }),
      ],
    });
    try {
      const { output } = await bundle.generate({ format: 'cjs', minify });
      const chunks = output.filter((item) => item.type === 'chunk');
      expect(chunks.some((chunk) => !chunk.isEntry)).toBe(true);
      const cache = new Map<string, Record<string, unknown>>();
      const load = (id: string): Record<string, unknown> => {
        const fileName = id.replace(/^\.\//, '');
        if (cache.has(fileName)) return cache.get(fileName)!;
        const exports = {};
        cache.set(fileName, exports);
        const chunk = chunks.find((chunk) => chunk.fileName === fileName)!;
        vm.runInNewContext(`(function(exports, require) { ${chunk.code}\n })`)(exports, load);
        return exports;
      };
      const entries = chunks.filter((chunk) => chunk.isEntry).map((chunk) => load(chunk.fileName));
      expect(entries.map((entry) => (entry.read as () => number)())).toEqual([42, 42]);
      (entries[0].increment as () => void)();
      expect(entries.map((entry) => (entry.read as () => number)())).toEqual([43, 43]);
    } finally {
      await bundle.close();
    }
  });
});

test('maps computed import reads and calls to their original references', async () => {
  const source = [
    'import { "a-b" as fn } from "dep";',
    'globalThis.read = () => fn;',
    'globalThis.call = () => fn();',
  ];
  const bundle = await rolldown({
    input: 'entry',
    external: ['dep'],
    plugins: [virtualPlugin({ entry: source.join('\n') })],
  });
  try {
    const { output } = await bundle.generate({ format: 'cjs', sourcemap: true });
    const chunk = output[0];
    expect(chunk.type).toBe('chunk');
    if (chunk.type !== 'chunk') return;
    const map = new TraceMap(chunk.map!.toString());
    const accesses = [...chunk.code.matchAll(/dep\["a-b"\]/g)];
    expect(accesses).toHaveLength(2);
    accesses.forEach((access, index) => {
      const lines = chunk.code.slice(0, access.index).split('\n');
      expect(
        originalPositionFor(map, { line: lines.length, column: lines.at(-1)!.length }),
      ).toEqual({
        source: expect.stringMatching(/entry$/),
        line: index + 2,
        column: source[index + 1].indexOf('fn'),
        name: 'fn',
      });
    });
  } finally {
    await bundle.close();
  }
});
