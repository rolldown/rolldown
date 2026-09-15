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
          // A dead reference followed by a live one to the same emitted file.
          return [
            `const unused = import.meta.ROLLUP_FILE_URL_${referenceId};`,
            `export const url = import.meta.ROLLUP_FILE_URL_${referenceId};`,
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
    // The tree-shaken occurrence must not consume a hook call: the surviving
    // occurrence is the first call and receives its result, matching Rollup.
    expect(calls).toBe(1);
    expect(chunk.code).toContain('"call-1"');
    expect(chunk.code).not.toContain('call-2');
  },
});
