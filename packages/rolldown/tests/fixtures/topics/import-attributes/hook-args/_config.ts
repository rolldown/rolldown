import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { parseModuleId, stringifyModuleId } from 'rolldown/utils';
import { expect } from 'vitest';

const main = path.join(import.meta.dirname, 'main.js');
const data = path.join(import.meta.dirname, 'data.json');
const dep = path.join(import.meta.dirname, 'dep.js');
const depId = stringifyModuleId(dep, { kind: 'x' });

const resolveCalls: Record<string, unknown> = {};
const loadCalls: Record<string, unknown> = {};

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
        load(id, { rawId, attributes }) {
          loadCalls[id] = { rawId, attributes };
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

    // The ids built in Rust are the ones `stringifyModuleId` builds.
    const dataId = '/data.json?attributes=%7B%22type%22%3A%22json%22%7D';
    expect(stringifyModuleId(data, { type: 'json' })).toBe(path.join(import.meta.dirname, dataId));
    expect(loadCalls[path.join(import.meta.dirname, dataId)]).toEqual({
      rawId: data,
      attributes: { type: 'json' },
    });
    expect(loadCalls[depId]).toEqual({ rawId: dep, attributes: { kind: 'x' } });
    expect(loadCalls[main]).toEqual({ rawId: main, attributes: {} });
    expect(parseModuleId(depId)).toEqual({ rawId: dep, attributes: { kind: 'x' } });
  },
});
