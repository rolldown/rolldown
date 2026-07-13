import { stripVTControlCharacters } from 'node:util';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const logs: { code?: string; message: string }[] = [];

export default defineTest({
  config: {
    output: [
      { format: 'esm', entryFileNames: 'main.mjs' },
      { format: 'cjs', entryFileNames: 'main.cjs' },
      { format: 'iife', entryFileNames: 'main.iife.js', name: 'iifeName' },
      { format: 'umd', entryFileNames: 'main.umd.js', name: 'umdName' },
    ],
    onLog(_level, log) {
      logs.push(log);
    },
    plugins: [
      {
        name: 'emit-asset',
        resolveId(source) {
          if (source === 'virtual:asset') {
            return '\0virtual:asset';
          }
        },
        load(id) {
          if (id === '\0virtual:asset') {
            const referenceId = this.emitFile({
              type: 'asset',
              name: 'asset.txt',
              source: 'hello',
            });
            return `export default import.meta.ROLLUP_FILE_URL_${referenceId};`;
          }
        },
      },
    ],
  },
  afterTest: (outputs) => {
    const [esm, cjs, iife, umd] = outputs.map((output) => output.output[0].code);

    expect(esm).toContain('import.meta.url');
    // For `cjs` on the `node` platform, the generated `import.meta.url` is polyfilled too.
    expect(cjs).toContain('pathToFileURL');
    expect(cjs).not.toContain('import.meta');
    // `iife` and `umd` have no way to polyfill `import.meta.url`, so it becomes `{}.url`.
    expect(iife).toContain('{}.url');
    expect(umd).toContain('{}.url');

    // Only the formats whose output actually ends up with an empty `import.meta` are warned about.
    const emptyImportMetaLogs = logs.filter((log) => log.code === 'EMPTY_IMPORT_META');
    expect(emptyImportMetaLogs).toHaveLength(2);
    expect(stripVTControlCharacters(emptyImportMetaLogs[0].message)).toContain(
      '`import.meta` may not be a valid syntax with the `iife` output format.',
    );
    expect(stripVTControlCharacters(emptyImportMetaLogs[1].message)).toContain(
      '`import.meta` may not be a valid syntax with the `umd` output format.',
    );
  },
});
