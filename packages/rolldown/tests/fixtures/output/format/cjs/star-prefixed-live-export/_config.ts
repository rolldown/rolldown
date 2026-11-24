import { createRequire } from 'node:module';
import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    output: { format: 'cjs', entryFileNames: 'main.cjs' },
    plugins: [
      {
        name: 'check-module-exports',
        renderStart() {
          const info = this.getModuleInfo(path.join(import.meta.dirname, 'main.js'));
          expect(info?.exports.toSorted()).toEqual(['**count', 'increment']);
        },
      },
    ],
  },
  afterTest() {
    const require = createRequire(import.meta.url);
    const entry = require('./dist/main.cjs');
    expect(entry['*count']).toBe(0);
    entry.increment();
    expect(entry['*count']).toBe(1);
  },
});
