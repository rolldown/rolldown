import fs from 'node:fs';
import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const logs: { code?: string; message: string }[] = [];

// a `resolveFileUrl` hook that fully replaces the reference with format-correct code
// (no `import.meta.url`) must not trigger the `EMPTY_IMPORT_META` warning,
// even in `iife`/`umd` output where the default rewrite would.
export default defineTest({
  config: {
    output: { format: 'iife', name: 'iifeName' },
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

    // The reference was fully resolved, so no `EMPTY_IMPORT_META` warning should fire.
    const emptyImportMetaLogs = logs.filter((log) => log.code === 'EMPTY_IMPORT_META');
    expect(emptyImportMetaLogs).toHaveLength(0);
  },
});
