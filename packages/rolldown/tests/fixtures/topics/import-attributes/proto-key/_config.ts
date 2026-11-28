import path from 'node:path';
import { stringifyModuleId } from 'rolldown/utils';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const dep = path.join(import.meta.dirname, 'files/dep.js');
const tags = ['static', 'dynamic', 'dynamic-computed'];
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
      {
        name: 'check-proto-attributes',
        resolveId(source, _importer, { attributes }) {
          if (source !== './files/dep.js') return;
          resolved.add(attributes.tag);
        },
        buildEnd(error) {
          if (error) return;
          expect([...this.getModuleIds()].filter((id) => id.startsWith(dep)).sort()).toStrictEqual(
            tags.map((tag) => stringifyModuleId(dep, { tag })).sort(),
          );
        },
      },
    ],
  },
  afterTest() {
    expect([...resolved].sort()).toStrictEqual([...tags].sort());
    expect(warnings).toStrictEqual([expect.stringContaining("['__proto__']")]);
  },
});
