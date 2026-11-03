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
          // The only reference lives in a statement tree-shaking removes.
          return [
            `const unused = import.meta.ROLLUP_FILE_URL_${referenceId};`,
            `export const kept = 'kept';`,
          ].join('\n');
        },
      },
      {
        name: 'throws',
        resolveFileUrl() {
          calls++;
          throw new Error('resolveFileUrl must not be called for tree-shaken references');
        },
      },
    ],
  },
  afterTest: async (output) => {
    // Rollup derives hook calls from what tree-shaking retains: a dead-only
    // reference means zero calls, so a throwing hook cannot fail the build.
    expect(calls).toBe(0);
    const chunk = output.output.find((o) => o.type === 'chunk')!;
    expect(chunk.code).toContain('kept');
    expect(chunk.code).not.toContain('ROLLUP_FILE_URL');
    expect(chunk.code).not.toContain('new URL(');
  },
});
