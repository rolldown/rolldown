import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const setup = path.join(import.meta.dirname, 'setup.cjs');
const after = path.join(import.meta.dirname, 'after.js');
const tail = path.join(import.meta.dirname, 'tail.cjs');

export default defineTest({
  config: {
    input: './entry.js',
    plugins: [
      {
        name: 'runtime-setup',
        transform(code, id) {
          if (id === '\0rolldown/runtime.js') {
            return `import ${JSON.stringify(setup)};\nimport ${JSON.stringify(after)};\nimport {} from ${JSON.stringify(tail)};\n${code}`;
          }
        },
      },
    ],
  },
  async afterTest(output) {
    const chunks = output.output.filter((item) => item.type === 'chunk');
    expect(chunks).toHaveLength(1);
    globalThis.__rolldown10906Order = [];
    globalThis.__rolldown10906LoadTail = false;
    await import('./dist/entry.js' as string);
    expect(globalThis.__rolldown10906Order).toEqual(['setup', 'after', 'tail', 'entry']);
  },
});

declare global {
  var __rolldown10906Order: string[];
  var __rolldown10906LoadTail: boolean;
}
