import fs from 'node:fs';
import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const seen: Record<string, string>[] = [];
const logs: { code?: string; message: string }[] = [];

export default defineTest({
  config: {
    output: { format: 'cjs' },
    onLog(_level, log) {
      logs.push(log);
    },
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
          return `export const url = import.meta.ROLLUP_FILE_URL_${referenceId};`;
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
  afterTest: (output) => {
    const chunk = output.output.find((o) => o.type === 'chunk')!;
    expect(chunk.code).toContain('"resolved:assets/asset');
    expect(chunk.code).not.toContain('import.meta');
    expect(chunk.code).not.toContain('new URL(');

    expect(seen).toHaveLength(1);
    const args = seen[0];
    expect(args.format).toBe('cjs');
    expect(args.chunkId).toBe('main.js');
    expect(args.moduleId.replace(/\\/g, '/')).toContain('resolve-file-url/cjs-format/main.js');
    expect(args.referenceId).toMatch(/^[$_a-zA-Z][$\w]*$/);
    expect(args.fileName).toMatch(/^assets\/asset-\w+\.txt$/);
    expect(args.relativePath).toBe(args.fileName);

    // `cjs` polyfills `import.meta.url`, so even without a replacement no
    // `EMPTY_IMPORT_META` warning may fire — let alone with one.
    expect(logs.filter((log) => log.code === 'EMPTY_IMPORT_META')).toHaveLength(0);
  },
});
