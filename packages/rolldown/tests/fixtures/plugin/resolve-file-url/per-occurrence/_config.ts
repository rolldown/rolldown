import fs from 'node:fs';
import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

let calls = 0;

export default defineTest({
  config: {
    plugins: [
      {
        name: 'emit-asset',
        load(id) {
          if (!id.endsWith('main.js')) return;
          const referenceId = this.emitFile({
            type: 'asset',
            name: 'asset.txt',
            source: fs.readFileSync(path.join(import.meta.dirname, 'asset.txt')),
          });
          // The same reference id, twice.
          return [
            `export const a = import.meta.ROLLUP_FILE_URL_${referenceId};`,
            `export const b = import.meta.ROLLUP_FILE_URL_${referenceId};`,
          ].join('\n');
        },
      },
      {
        name: 'counts-calls',
        resolveFileUrl() {
          calls++;
          return `'call-${calls}'`;
        },
      },
    ],
  },
  afterTest: async (output) => {
    const chunk = output.output.find((o) => o.type === 'chunk')!;
    // Rollup calls the hook per AST occurrence, not per (module, referenceId).
    // Each call gets its own result, so both land in the output.
    expect(calls).toBe(2);
    expect(chunk.code).toContain('"call-1"');
    expect(chunk.code).toContain('"call-2"');
  },
});
