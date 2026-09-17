import path from 'node:path';
import vm from 'node:vm';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const polyfill = path.join(import.meta.dirname, 'polyfill.js');

export default defineTest({
  config: {
    input: './entry.js',
    output: { format: 'iife' },
    plugins: [
      {
        name: 'runtime-esm-polyfill',
        transform(code, id) {
          if (id === '\0rolldown/runtime.js') {
            return `import ${JSON.stringify(polyfill)};\n${code}`;
          }
        },
      },
    ],
  },
  afterTest(output) {
    const chunk = output.output.find((item) => item.type === 'chunk')!;
    const objectBeforePolyfill = Object.create(Object);
    objectBeforePolyfill.create = undefined;
    const result = vm.runInNewContext(`${chunk.code}\n__rolldown10906Value;`, {
      Object: objectBeforePolyfill,
      originalCreate: Object.create,
    });
    expect(result.value).toBe(1);
  },
});
