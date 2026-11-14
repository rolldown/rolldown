import fs from 'node:fs';
import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const seen: Record<string, string>[] = [];

// The computed spelling `import.meta['ROLLUP_FILE_URL_<id>']` must reach the hook
// just like the dot form — Rollup rewrites both.
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
          return `export const url = import.meta['ROLLUP_FILE_URL_${referenceId}'];`;
        },
      },
      {
        name: 'resolve-file-url',
        resolveFileUrl(args) {
          seen.push({ ...args });
          return `'resolved:' + ${JSON.stringify(args.relativePath)}`;
        },
      },
    ],
  },
  afterTest: async (output) => {
    const chunk = output.output.find((o) => o.type === 'chunk')!;
    expect(chunk.code).toContain('const url = "resolved:assets/asset');
    expect(chunk.code).not.toContain('import.meta.url');
    expect(chunk.code).not.toContain('new URL(');

    expect(seen).toHaveLength(1);
    expect(seen[0].fileName).toMatch(/^assets\/asset-\w+\.txt$/);
  },
});
