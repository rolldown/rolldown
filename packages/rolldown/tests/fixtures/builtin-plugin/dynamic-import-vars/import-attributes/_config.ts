import path from 'node:path';
import { stripVTControlCharacters } from 'node:util';
import type { RolldownLog } from 'rolldown';
import { viteDynamicImportVarsPlugin, viteImportGlobPlugin } from 'rolldown/experimental';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const warnings: RolldownLog[] = [];

export default defineTest({
  config: {
    input: 'main.ts',
    experimental: { importAttributes: true },
    onLog(level, log) {
      if (level === 'warn' && log.code === 'DEPRECATED_IMPORT_ASSERTION') {
        warnings.push(log);
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
    expect(warnings).toHaveLength(1);
    const [warning] = warnings;
    expect(warning).toStrictEqual({
      code: 'DEPRECATED_IMPORT_ASSERTION',
      plugin: 'builtin:vite-dynamic-import-vars',
      id: expect.stringMatching(/[/\\]main\.ts$/),
      pos: 575,
      loc: { file: warning.id, line: 18, column: 43 },
      message: expect.any(String),
    });
    expect(stripVTControlCharacters(warning.message)).toMatchInlineSnapshot(`
      "[DEPRECATED_IMPORT_ASSERTION] The legacy \`assert\` keyword in "main.ts" is deprecated and is treated as \`with\`. Use \`with\` instead.
          ╭─[ main.ts:18:43 ]
          │
       18 │   return import(\`@/\${name}.js\`, { /* 😀 */ assert: { type: 'text' } });
          │                                            ───┬──\x20\x20
          │                                               ╰──── replace \`assert\` with \`with\`
      ────╯
      "
    `);
  },
});
