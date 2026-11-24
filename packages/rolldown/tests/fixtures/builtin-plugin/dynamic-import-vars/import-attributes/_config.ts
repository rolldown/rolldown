import path from 'node:path';
import { stripVTControlCharacters } from 'node:util';
import type { RolldownLog } from 'rolldown';
import { viteDynamicImportVarsPlugin, viteImportGlobPlugin } from 'rolldown/experimental';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const deprecatedAssertionWarnings: RolldownLog[] = [];
const invalidAttributeWarnings: RolldownLog[] = [];

export default defineTest({
  config: {
    input: 'main.ts',
    experimental: { importAttributes: true },
    onLog(level, log) {
      if (level === 'warn' && log.code === 'DEPRECATED_IMPORT_ASSERTION') {
        deprecatedAssertionWarnings.push(log);
      }
      if (level === 'warn' && log.code === 'INVALID_IMPORT_ATTRIBUTE') {
        invalidAttributeWarnings.push(log);
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
          if (source === './files/module.js?malformed') {
            expect(attributes).toEqual({ type: 'text' });
            return path.resolve(import.meta.dirname, 'files/module.js');
          }
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
    expect(deprecatedAssertionWarnings).toHaveLength(1);
    const [deprecatedWarning] = deprecatedAssertionWarnings;
    expect(deprecatedWarning).toStrictEqual({
      code: 'DEPRECATED_IMPORT_ASSERTION',
      plugin: 'builtin:vite-dynamic-import-vars',
      id: expect.stringMatching(/[/\\]main\.ts$/),
      pos: 575,
      loc: { file: deprecatedWarning.id, line: 18, column: 43 },
      message: expect.any(String),
    });
    expect(stripVTControlCharacters(deprecatedWarning.message)).toMatchInlineSnapshot(`
      "[DEPRECATED_IMPORT_ASSERTION] The legacy \`assert\` keyword in "main.ts" is deprecated and is treated as \`with\`. Use \`with\` instead.
          ╭─[ main.ts:18:43 ]
          │
       18 │   return import(\`@/\${name}.js\`, { /* 😀 */ assert: { type: 'text' } });
          │                                            ───┬──\x20\x20
          │                                               ╰──── replace \`assert\` with \`with\`
      ────╯
      "
    `);
    expect(invalidAttributeWarnings).toHaveLength(1);
    const [warning] = invalidAttributeWarnings;
    expect(warning).toStrictEqual({
      code: 'INVALID_IMPORT_ATTRIBUTE',
      plugin: 'builtin:vite-dynamic-import-vars',
      id: expect.stringMatching(/[/\\]main\.ts$/),
      pos: 870,
      loc: { file: warning.id, line: 26, column: 66 },
      message: expect.any(String),
    });
    expect(stripVTControlCharacters(warning.message)).toMatchInlineSnapshot(`
      "[INVALID_IMPORT_ATTRIBUTE] Rolldown could not statically analyze an import attribute of a dynamic import in "main.ts". Import attributes need to have string keys and values, so the attribute is ignored.
          ╭─[ main.ts:26:67 ]
          │
       26 │   return import(\`@/\${name}.js?malformed\`, { with: { type: 'text', malformed: 123 } });
          │                                                                   ───────┬──────\x20\x20
          │                                                                          ╰──────── not a string key and value
      ────╯
      "
    `);
  },
});
