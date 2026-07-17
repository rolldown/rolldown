import fs from 'node:fs';
import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const seen: Record<string, string>[] = [];

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
          return `export const url = import.meta.ROLLDOWN_FILE_URL_${referenceId};`;
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
    // The returned code becomes real AST, so `"resolved:" + "<path>"` is
    // constant-folded into a single literal.
    expect(chunk.code).toContain('const url = "resolved:assets/asset');
    expect(chunk.code).not.toContain('import.meta.url');
    expect(chunk.code).not.toContain('new URL(');

    expect(seen).toHaveLength(1);
    const args = seen[0];
    expect(args.format).toBe('es');
    expect(args.moduleId.replace(/\\/g, '/')).toContain('resolve-file-url/rolldown-alias/main.js');
    expect(args.referenceId).toMatch(/^[$_a-zA-Z][$\w]*$/);
    expect(args.fileName).toMatch(/^assets\/asset-\w+\.txt$/);
    expect(args.relativePath).toBe(args.fileName);
    expect(args.chunkId).toBe('main.js');
  },
});
