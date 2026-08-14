import { stripVTControlCharacters } from 'node:util';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const logs: { code?: string; message: string }[] = [];

// A plugin-provided replacement is finalized like user code. If it introduces
// `import.meta.url` in a non-ESM format where it cannot be polyfilled, it must
// trigger the same warning as a direct `import.meta.url` access.
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
            source: 'asset',
          });
          return `export const url = import.meta.ROLLUP_FILE_URL_${referenceId};`;
        },
      },
      {
        name: 'resolve-to-import-meta-url',
        resolveFileUrl() {
          return 'import.meta.url';
        },
      },
    ],
  },
  afterTest: (output) => {
    const chunk = output.output.find((item) => item.type === 'chunk')!;
    expect(chunk.code).toContain('{}.url');
    expect(chunk.code).not.toContain('import.meta');

    const emptyImportMetaLogs = logs.filter((log) => log.code === 'EMPTY_IMPORT_META');
    expect(emptyImportMetaLogs).toHaveLength(1);
    const message = stripVTControlCharacters(emptyImportMetaLogs[0].message);
    expect(message).toContain('`iife` output format');
    expect(message).toContain('import.meta.url');
    expect(message).toContain('import.meta.ROLLUP_FILE_URL_');
  },
});
