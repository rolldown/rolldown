import vm from 'node:vm';

import type { Plugin } from 'rolldown';
import { rolldown } from 'rolldown';
import { describe, expect, test } from 'vitest';

function entryPlugin(code: string): Plugin {
  return {
    name: 'virtual',
    resolveId: (id) => id,
    load: () => code,
  };
}

const names = ['a"b', String.raw`a\nb`, 'a\nb'];

describe.each([false, 'dce-only', true] as const)('minify: %s', (minify) => {
  describe.each(['iife', 'umd'] as const)('format: %s', (format) => {
    describe.each([false, true])('extend: %s', (extend) => {
      test.each(names)('preserves the namespace segment %j', async (name) => {
        const bundle = await rolldown({
          input: 'entry',
          plugins: [entryPlugin('export const value = 42;')],
        });
        try {
          const { output } = await bundle.generate({
            format,
            minify,
            extend,
            name: `Root.${name}.Leaf`,
          });
          const chunk = output[0];
          expect(chunk.type).toBe('chunk');
          if (chunk.type !== 'chunk') return;

          const fresh = { Root: undefined as Record<string, { Leaf: object }> | undefined };
          vm.runInNewContext(chunk.code, fresh);
          expect(fresh.Root![name].Leaf).toMatchObject({ value: 42 });

          const leaf = { existing: true };
          const context = { Root: { [name]: { Leaf: leaf } } };
          vm.runInNewContext(chunk.code, context);
          expect(context.Root[name].Leaf).toMatchObject(
            extend ? { existing: true, value: 42 } : { value: 42 },
          );
          expect(context.Root[name].Leaf === leaf).toBe(extend);
        } finally {
          await bundle.close();
        }
      });
    });
  });

  test.each(names)('reads a UMD global dependency through the segment %j', async (name) => {
    const bundle = await rolldown({
      input: 'entry',
      external: ['dep'],
      plugins: [entryPlugin("import { value } from 'dep'; globalThis.result = value;")],
    });
    try {
      const { output } = await bundle.generate({
        format: 'umd',
        minify,
        globals: { dep: `Dependencies.${name}.nested` },
      });
      const chunk = output[0];
      expect(chunk.type).toBe('chunk');
      if (chunk.type !== 'chunk') return;
      const context = {
        Dependencies: { [name]: { nested: { value: 42 } } },
        result: undefined,
      };
      vm.runInNewContext(chunk.code, context);
      expect(context.result).toBe(42);
    } finally {
      await bundle.close();
    }
  });

  test('extends a top-level IIFE namespace containing quotes and backslashes', async () => {
    const name = String.raw`a"b\nc`;
    const bundle = await rolldown({
      input: 'entry',
      plugins: [entryPlugin('export const value = 42;')],
    });
    try {
      const { output } = await bundle.generate({ format: 'iife', minify, name, extend: true });
      const chunk = output[0];
      expect(chunk.type).toBe('chunk');
      if (chunk.type !== 'chunk') return;
      const namespace = { existing: true };
      const context = { [name]: namespace };
      vm.runInNewContext(chunk.code, context);
      expect(context[name]).toBe(namespace);
      expect(namespace).toMatchObject({ existing: true, value: 42 });
    } finally {
      await bundle.close();
    }
  });
});
