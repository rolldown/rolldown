import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { stringifyModuleId } from 'rolldown/utils';
import { expect } from 'vitest';

const main = path.join(import.meta.dirname, 'main.js');
const data = path.join(import.meta.dirname, 'data.json');
const dep = path.join(import.meta.dirname, 'dep.js');
const depId = stringifyModuleId(dep, { kind: 'x' });

const resolveCalls: Record<string, unknown> = {};

export default defineTest({
  config: {
    input: main,
    experimental: { importAttributes: true },
    plugins: [
      {
        name: 'test-plugin',
        resolveId(source, importer, { attributes, importerRawId, importerAttributes }) {
          resolveCalls[source] = { importer, attributes, importerRawId, importerAttributes };
        },
        buildEnd() {
          expect([...this.getModuleIds()].sort()).toEqual(
            [
              main,
              stringifyModuleId(data, { type: 'json' }),
              depId,
              path.join(import.meta.dirname, 'leaf.js'),
            ].sort(),
          );
        },
      },
    ],
  },
  afterTest() {
    expect(resolveCalls['./data.json']).toEqual({
      importer: main,
      attributes: { type: 'json' },
      importerRawId: main,
      importerAttributes: {},
    });
    // The importer of a module imported with attributes carries them in its id.
    expect(resolveCalls['./leaf.js']).toEqual({
      importer: depId,
      attributes: {},
      importerRawId: dep,
      importerAttributes: { kind: 'x' },
    });
  },
});
