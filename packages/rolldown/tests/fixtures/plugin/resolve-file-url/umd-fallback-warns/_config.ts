import fs from 'node:fs';
import path from 'node:path';
import { stripVTControlCharacters } from 'node:util';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const seen: Record<string, string>[] = [];
const logs: { code?: string; message: string }[] = [];

// when a `resolveFileUrl` hook is present but declines (returns `null`), the default
// `new URL(..., import.meta.url).href` rewrite stands, which becomes `{}.url` in `umd`.
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
        name: 'declines',
        resolveFileUrl(args) {
          seen.push({ ...args });
          return null;
        },
      },
    ],
  },
  afterTest: (output) => {
    const chunk = output.output.find((o) => o.type === 'chunk')!;
    // The default rewrite stands and its `import.meta.url` collapses to `{}.url`.
    expect(chunk.code).toContain('new URL(');
    expect(chunk.code).toContain('{}.url');

    expect(seen).toHaveLength(1);
    const args = seen[0];
    expect(args.format).toBe('umd');
    expect(args.chunkId).toBe('main.js');
    expect(args.moduleId.replace(/\\/g, '/')).toContain(
      'resolve-file-url/umd-fallback-warns/main.js',
    );
    expect(args.referenceId).toMatch(/^[$_a-zA-Z][$\w]*$/);
    expect(args.fileName).toMatch(/^assets\/asset-\w+\.txt$/);
    expect(args.relativePath).toBe(args.fileName);

    const emptyImportMetaLogs = logs.filter((log) => log.code === 'EMPTY_IMPORT_META');
    expect(emptyImportMetaLogs).toHaveLength(1);
    const message = stripVTControlCharacters(emptyImportMetaLogs[0].message);
    expect(message).toContain('`umd` output format');
    // The warning must make clear the offending `import.meta.url` was generated from
    // expanding an `import.meta.ROLLUP_FILE_URL_*` reference
    expect(message).toContain('ROLLUP_FILE_URL_');
    expect(message).toContain('import.meta.url');
  },
});
