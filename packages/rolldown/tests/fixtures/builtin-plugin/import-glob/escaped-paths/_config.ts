import { rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { defineTest } from 'rolldown-tests';
import { viteImportGlobPlugin } from 'rolldown/experimental';

const quotedModulePaths = ['eager', 'lazy', 'keys'].map((type) =>
  join(import.meta.dirname, 'dir', `a"b.${type}.js`),
);
const quotedModulePrefix = '\0quoted-module:';

function removeQuotedModules() {
  for (const path of quotedModulePaths) rmSync(path, { force: true });
}

export default defineTest({
  skip: process.platform === 'win32',
  config: {
    plugins: [
      viteImportGlobPlugin(),
      {
        name: 'quoted-module',
        resolveId(id) {
          if (id.startsWith('./dir/a"b.')) return quotedModulePrefix + id;
        },
        load(id) {
          if (id.startsWith(quotedModulePrefix)) return 'export default 42;\n';
        },
      },
    ],
  },
  beforeTest() {
    for (const path of quotedModulePaths) writeFileSync(path, 'export default 42;\n');
  },
  async afterTest() {
    try {
      await import('./assert.mjs');
    } finally {
      removeQuotedModules();
    }
  },
  catchError(error) {
    removeQuotedModules();
    throw error;
  },
});
