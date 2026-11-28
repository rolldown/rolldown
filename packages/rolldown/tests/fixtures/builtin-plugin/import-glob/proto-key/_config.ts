import { viteImportGlobPlugin } from 'rolldown/experimental';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const tags = ['lazy', 'lazy-computed', 'eager', 'eager-computed'];
const resolved = new Set<string>();
const warnings: string[] = [];

export default defineTest({
  config: {
    experimental: { importAttributes: true },
    onLog(level, log) {
      if (level === 'warn' && log.code === 'INVALID_IMPORT_ATTRIBUTE') {
        warnings.push(log.message);
      }
    },
    plugins: [
      viteImportGlobPlugin(),
      {
        name: 'check-proto-attributes',
        resolveId(source, _importer, { attributes }) {
          if (source !== './files/dep.js') return;
          resolved.add(attributes.tag);
        },
      },
    ],
  },
  afterTest() {
    expect([...resolved].sort()).toStrictEqual([...tags].sort());
    expect(warnings).toStrictEqual([]);
  },
});
