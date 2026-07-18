import fs from 'node:fs';
import path from 'node:path';
import { stripVTControlCharacters } from 'node:util';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const seen: Record<string, string>[] = [];
const logs: { code?: string; message: string }[] = [];

// Without a `resolveFileUrl` replacement, the default `new URL(..., import.meta.url).href`
// rewrite stands. Unlike `cjs` on the `node` platform, `cjs` on the `neutral` platform
// cannot polyfill `import.meta.url`, so it becomes `{}.url` and emits one warning.
export default defineTest({
  config: {
    platform: 'neutral',
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
        name: 'declines',
        resolveFileUrl(args) {
          seen.push({ ...args });
          return null;
        },
      },
    ],
  },
  afterTest: (output) => {
    const chunk = output.output.find((item) => item.type === 'chunk')!;
    expect(chunk.code).toContain('new URL(');
    expect(chunk.code).toContain('{}.url');
    expect(chunk.code).not.toContain('import.meta');

    expect(seen).toHaveLength(1);
    const args = seen[0];
    expect(args.format).toBe('cjs');
    expect(args.chunkId).toBe('main.js');
    expect(args.moduleId.replace(/\\/g, '/')).toContain(
      'resolve-file-url/cjs-neutral-fallback-warns/main.js',
    );
    expect(args.referenceId).toMatch(/^[$_a-zA-Z][$\w]*$/);
    expect(args.fileName).toMatch(/^assets\/asset-\w+\.txt$/);
    expect(args.relativePath).toBe(args.fileName);

    const emptyImportMetaLogs = logs.filter((log) => log.code === 'EMPTY_IMPORT_META');
    expect(emptyImportMetaLogs).toHaveLength(1);
    const message = stripVTControlCharacters(emptyImportMetaLogs[0].message);
    expect(message).toContain('`cjs` output format');
    expect(message).toContain('ROLLUP_FILE_URL_');
    expect(message).toContain('import.meta.url');
  },
});
