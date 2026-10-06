import vm from 'node:vm';

import { rolldown } from 'rolldown';
import { describe, expect, test } from 'vitest';

const names = ['', 'a-b', 'a_b', 'a"b', String.raw`a\nb`, 'for', 'eval', 'arguments'];

describe.each([false, 'dce-only', true] as const)('minify: %s', (minify) => {
  describe.each(['es', 'cjs'] as const)('format: %s', (format) => {
    test.each(['shimMissingExports', 'empty'] as const)(
      'preserves arbitrary export names with %s',
      async (mode) => {
        const imports = names.map((name, i) => `${JSON.stringify(name)} as shim${i}`).join(', ');
        const exports = names.map((name, i) => `shim${i} as ${JSON.stringify(name)}`).join(', ');
        const bundle = await rolldown({
          input: 'entry',
          shimMissingExports: mode === 'shimMissingExports',
          plugins: [
            {
              name: 'virtual',
              resolveId: (id) => id,
              load(id) {
                if (id === 'empty') {
                  return { code: 'export {};', moduleType: mode === 'empty' ? 'empty' : 'js' };
                }
                return `
                  import { ${imports} } from 'empty';
                  const _ = 1, a_b = 2;
                  export const locals = [_, a_b];
                  export { ${exports} };
                `;
              },
            },
          ],
        });
        try {
          const { output } = await bundle.generate({ format, minify });
          const chunk = output[0];
          expect(chunk.type).toBe('chunk');
          if (chunk.type !== 'chunk') return;

          let namespace: Record<string, unknown>;
          if (format === 'es') {
            namespace = await import(`data:text/javascript,${encodeURIComponent(chunk.code)}`);
          } else {
            const context = { exports: {} };
            vm.runInNewContext(chunk.code, context);
            namespace = context.exports;
          }
          expect(Object.keys(namespace).sort()).toEqual([...names, 'locals'].sort());
          expect({ ...namespace }).toEqual({
            ...Object.fromEntries(names.map((name) => [name, undefined])),
            locals: [1, 2],
          });
        } finally {
          await bundle.close();
        }
      },
    );
  });
});
