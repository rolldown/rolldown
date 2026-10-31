import vm from 'node:vm';

import type { Plugin } from 'rolldown';
import { rolldown } from 'rolldown';
import { describe, expect, test } from 'vitest';

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
  describe.each(['cjs', 'umd-cjs', 'umd-amd'] as const)('loader: %s', (loader) => {
    test.each(['a"b', "a'b", String.raw`a\nb`, 'a\nb'])(
      'preserves external dependency paths containing %j',
      async (name) => {
        const dependencies = {
          'default-dep': 10,
          'named-dep': { value: 20 },
          'side-dep': {},
        };
        const paths = Object.fromEntries(
          Object.keys(dependencies).map((id) => [id, `${name}/${id}`]),
        );
        const values = new Map(
          Object.entries(dependencies).map(([id, value]) => [paths[id], value]),
        );
        const bundle = await rolldown({
          input: 'entry',
          external: Object.keys(dependencies),
          plugins: [
            virtualPlugin({
              entry: `
                import defaultValue from 'default-dep';
                import { value as namedValue } from 'named-dep';
                import 'side-dep';
                export const result = defaultValue + namedValue;
              `,
            }),
          ],
        });
        try {
          const { output } = await bundle.generate({
            format: loader === 'cjs' ? 'cjs' : 'umd',
            minify,
            name: 'Bundle',
            paths,
            globals: Object.fromEntries(Object.keys(dependencies).map((id) => [id, 'Dependency'])),
          });
          const chunk = output[0];
          expect(chunk.type).toBe('chunk');
          if (chunk.type !== 'chunk') return;

          const exports = {};
          const loaded: string[] = [];
          const require = (id: string) => {
            loaded.push(id);
            expect(values.has(id)).toBe(true);
            return values.get(id);
          };
          if (loader === 'umd-amd') {
            const define = (ids: string[], factory: (...args: unknown[]) => void) => {
              factory(...ids.map((id) => (id === 'exports' ? exports : require(id))));
            };
            define.amd = true;
            vm.runInNewContext(chunk.code, { define });
          } else {
            vm.runInNewContext(chunk.code, { exports, module: { exports }, require });
          }
          expect(loaded.sort()).toEqual(Object.values(paths).sort());
          expect(exports).toMatchObject({ result: 30 });
        } finally {
          await bundle.close();
        }
      },
    );
  });

  test.each(['a"b', String.raw`a\nb`])(
    'escapes an external star re-export path %j',
    async (path) => {
      const bundle = await rolldown({
        input: 'entry',
        external: ['dep'],
        plugins: [virtualPlugin({ entry: "export * from 'dep';" })],
      });
      try {
        const { output } = await bundle.generate({ format: 'cjs', minify, paths: { dep: path } });
        const chunk = output[0];
        expect(chunk.type).toBe('chunk');
        if (chunk.type !== 'chunk') return;
        const exports = {};
        const loaded: string[] = [];
        vm.runInNewContext(chunk.code, {
          exports,
          require(id: string) {
            loaded.push(id);
            return { value: 42 };
          },
        });
        expect(loaded).toEqual([path]);
        expect(exports).toMatchObject({ value: 42 });
      } finally {
        await bundle.close();
      }
    },
  );

  test('loads a shared CommonJS chunk whose file name contains a quote', async () => {
    const bundle = await rolldown({
      input: ['entry-a', 'entry-b'],
      plugins: [
        virtualPlugin({
          'entry-a': "import { value } from 'shared'; export const result = value + 1;",
          'entry-b': "import { value } from 'shared'; export const result = value + 2;",
          shared: 'export const value = globalThis.sharedValue;',
        }),
      ],
    });
    try {
      const { output } = await bundle.generate({
        format: 'cjs',
        minify,
        chunkFileNames: "shared'chunk.js",
      });
      const chunks = output.filter((item) => item.type === 'chunk');
      expect(chunks.some((chunk) => chunk.fileName === "shared'chunk.js")).toBe(true);
      const cache = new Map<string, Record<string, unknown>>();
      const load = (id: string): Record<string, unknown> => {
        const fileName = id.replace(/^\.\//, '');
        if (cache.has(fileName)) return cache.get(fileName)!;
        const exports = {};
        cache.set(fileName, exports);
        const chunk = chunks.find((chunk) => chunk.fileName === fileName)!;
        vm.runInNewContext(`(function(exports, require) { ${chunk.code}\n })`, { sharedValue: 40 })(
          exports,
          load,
        );
        return exports;
      };
      expect(load('entry-a.js').result).toBe(41);
      expect(load('entry-b.js').result).toBe(42);
    } finally {
      await bundle.close();
    }
  });
});
