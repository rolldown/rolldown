import path from 'node:path';
import { viteDynamicImportVarsPlugin, viteImportGlobPlugin } from 'rolldown/experimental';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

let deprecatedAssertionWarnings = 0;

export default defineTest({
  config: {
    input: 'main.ts',
    experimental: { importAttributes: true },
    onLog(level, log) {
      if (level === 'warn' && log.code === 'DEPRECATED_IMPORT_ASSERTION') {
        deprecatedAssertionWarnings++;
      }
    },
    plugins: [
      viteDynamicImportVarsPlugin({
        async resolver(id) {
          return id.replace('@', path.resolve(import.meta.dirname, 'files'));
        },
      }),
      viteImportGlobPlugin(),
      {
        name: 'check-dynamic-import-attributes',
        resolveId(source, _importer, { attributes }) {
          if (source === './files/module.js?raw' || 'custom-key' in attributes) {
            expect(source).toBe('./files/module.js?raw');
            expect(attributes).toEqual({ type: 'text', 'custom-key': 'a"b\\c\n' });
            return path.resolve(import.meta.dirname, 'files/module.js');
          }
        },
      },
    ],
  },
  async afterTest() {
    await import('./assert.mjs');
    expect(deprecatedAssertionWarnings).toBe(1);
  },
});
