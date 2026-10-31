import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { stringifyModuleId } from 'rolldown/utils';
import { expect } from 'vitest';

const main = path.join(import.meta.dirname, 'main.js');
const dep = path.join(import.meta.dirname, 'dep.js');
let seen: unknown;

export default defineTest({
  config: {
    input: main,
    experimental: { importAttributes: true },
    plugins: [
      {
        name: 'test-plugin',
        resolveDynamicImport(source, importer, options) {
          if (source !== './dep.js') return;
          seen = { importer, ...options };
          // The returned attributes and `meta` belong to the module they resolve to.
          return { id: dep, attributes: { kind: 'y' }, meta: { tagged: true } };
        },
        buildEnd() {
          const info = this.getModuleInfo(stringifyModuleId(dep, { kind: 'y' }));
          expect(info?.meta).toEqual({ tagged: true });
        },
      },
    ],
  },
  afterTest() {
    expect(seen).toEqual({
      importer: main,
      attributes: { kind: 'x' },
      importerRawId: main,
      importerAttributes: {},
    });
  },
});
