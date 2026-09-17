import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const polyfill = path.join(import.meta.dirname, 'polyfill.cjs');

export default defineTest({
  config: {
    input: { first: './first.js', second: './second.js' },
    plugins: [
      {
        name: 'runtime-polyfill',
        transform(code, id) {
          if (id === '\0rolldown/runtime.js') {
            return `import ${JSON.stringify(polyfill)};\n${code}`;
          }
        },
      },
    ],
  },
  async afterTest(output) {
    const chunks = output.output.filter((item) => item.type === 'chunk');
    expect(
      chunks.some(
        (chunk) =>
          chunk.code.includes('__commonJSMin') &&
          chunk.code.includes('__rolldown10906PolyfillCalls'),
      ),
    ).toBe(true);
    globalThis.__rolldown10906PolyfillCalls = 0;
    await import('./dist/first.js' as string);
    await import('./dist/second.js' as string);
    expect(globalThis.__rolldown10906PolyfillCalls).toBe(1);
  },
});

declare global {
  var __rolldown10906PolyfillCalls: number;
}
