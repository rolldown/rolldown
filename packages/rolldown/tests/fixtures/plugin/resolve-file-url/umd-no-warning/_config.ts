import fs from 'node:fs';
import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const seen: Record<string, string>[] = [];
const logs: { code?: string; message: string }[] = [];

// a `resolveFileUrl` hook that fully replaces the reference with format-correct code
// (no `import.meta.url`) must not trigger the `EMPTY_IMPORT_META` warning in `umd` output.
export default defineTest({
  config: {
    output: { format: 'umd', name: 'umdName' },
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
          return `new URL(${JSON.stringify(args.relativePath)}, self.location.href).href`;
        },
      },
    ],
  },
  afterTest: (output) => {
    const chunk = output.output.find((o) => o.type === 'chunk')!;
    expect(chunk.code).toContain('self.location.href');
    expect(chunk.code).not.toContain('import.meta');
    expect(chunk.code).not.toContain('{}.url');

    expect(seen).toHaveLength(1);
    expect(seen[0].format).toBe('umd');

    // The reference was fully resolved, so no `EMPTY_IMPORT_META` warning should fire.
    expect(logs.filter((log) => log.code === 'EMPTY_IMPORT_META')).toHaveLength(0);
  },
});
