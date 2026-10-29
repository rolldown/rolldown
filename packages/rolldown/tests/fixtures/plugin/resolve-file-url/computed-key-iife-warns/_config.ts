import fs from 'node:fs';
import path from 'node:path';
import { stripVTControlCharacters } from 'node:util';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const logs: { code?: string; message: string }[] = [];

// With no `resolveFileUrl` hook, the computed spelling `import.meta['ROLLUP_FILE_URL_<id>']`
// falls back to the default rewrite whose `import.meta.url` collapses to `{}.url` in `iife`,
// so it must warn just like the dot form does.
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
          return `export const url = import.meta['ROLLUP_FILE_URL_${referenceId}'];`;
        },
      },
    ],
  },
  afterTest: (output) => {
    const chunk = output.output.find((o) => o.type === 'chunk')!;
    expect(chunk.code).toContain('new URL(');
    expect(chunk.code).toContain('{}.url');

    const emptyImportMetaLogs = logs.filter((log) => log.code === 'EMPTY_IMPORT_META');
    expect(emptyImportMetaLogs).toHaveLength(1);
    const message = stripVTControlCharacters(emptyImportMetaLogs[0].message);
    expect(message).toContain('ROLLUP_FILE_URL_');
  },
});
