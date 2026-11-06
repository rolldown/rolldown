import { expect, test, vi } from 'vitest';
import type { BindingOutputChunk, BindingOutputs } from '../../src/binding.cjs';
import type { MinimalPluginContext } from '../../src/plugin/minimal-plugin-context';
import type { OutputChunk } from '../../src/types/rolldown-output';
import {
  type ChangedOutputs,
  collectChangedBundle,
  transformToOutputBundle,
  transformToRollupSourceMap,
} from '../../src/utils/transform-to-rollup-output';

test('SourceMap.toUrl encodes non-ASCII UTF-8 in multiple chunks without Buffer', () => {
  const map = transformToRollupSourceMap(
    JSON.stringify({
      version: 3,
      file: 'bundle.js',
      sources: ['こんにちは.ts'],
      sourcesContent: ['🌍'.repeat(0x4000)],
      names: [],
      mappings: '',
    }),
  );
  const json = map.toString();
  const expectedUrl = `data:application/json;charset=utf-8;base64,${Buffer.from(json, 'utf-8').toString('base64')}`;
  const bufferDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'Buffer');
  const btoaSpy = vi.spyOn(globalThis, 'btoa');

  try {
    Object.defineProperty(globalThis, 'Buffer', {
      configurable: true,
      value: undefined,
      writable: true,
    });

    expect(map.toUrl()).toBe(expectedUrl);
    expect(btoaSpy.mock.calls.length).toBeGreaterThan(1);
  } finally {
    if (bufferDescriptor === undefined) {
      Reflect.deleteProperty(globalThis, 'Buffer');
    } else {
      Object.defineProperty(globalThis, 'Buffer', bufferDescriptor);
    }
    btoaSpy.mockRestore();
  }
});

// Stands in for the native `BindingOutputChunk` and records which of its getters run, that is,
// which values are copied out of Rust.
function nativeChunk(fileName: string, imports: string[], calls: string[]) {
  const map = { version: 3, file: fileName, sources: ['a.ts'], names: ['a'], mappings: 'AAAAA' };
  const record =
    <T>(name: string, value: () => T) =>
    () => {
      calls.push(`${fileName}:${name}`);
      return value();
    };
  return {
    getFileName: record('getFileName', () => fileName),
    getName: record('getName', () => fileName.replace('.js', '')),
    getExports: record('getExports', () => []),
    getIsEntry: record('getIsEntry', () => false),
    getFacadeModuleId: record('getFacadeModuleId', () => null),
    getIsDynamicEntry: record('getIsDynamicEntry', () => false),
    getSourcemapFileName: record('getSourcemapFileName', () => `${fileName}.map`),
    getPreliminaryFileName: record('getPreliminaryFileName', () => fileName),
    getCode: record('getCode', () => `// ${fileName}\n`),
    getCodeIdentity: record('getCodeIdentity', () => 1),
    getModules: record('getModules', () => ({ keys: [], values: [] })),
    getImports: record('getImports', () => [...imports]),
    getDynamicImports: record('getDynamicImports', () => []),
    getModuleIds: record('getModuleIds', () => []),
    getMap: record('getMap', () => JSON.stringify(map)),
  } as unknown as BindingOutputChunk;
}

function outputBundle(calls: string[]) {
  const changed: ChangedOutputs = { updated: new Map(), deleted: new Set() };
  const outputs: BindingOutputs = {
    chunks: [
      nativeChunk('a.js', ['b.js', 'style.js'], calls),
      nativeChunk('b.js', [], calls),
      nativeChunk('style.js', [], calls),
    ],
    assets: [],
  };
  const bundle = transformToOutputBundle({} as MinimalPluginContext, outputs, changed);
  const chunk = (fileName: string) => bundle[fileName] as OutputChunk;
  return { bundle, changed, chunk };
}

const copiedOut = (calls: string[]) =>
  calls.filter((call) => /:get(Code|Map|ModuleIds|Modules)$/.test(call));

test('a chunk assigned the values it had is not sent back, and its code and map are not copied', () => {
  const calls: string[] = [];
  const { bundle, changed, chunk } = outputBundle(calls);
  for (const fileName of ['a.js', 'b.js', 'style.js']) {
    chunk(fileName).imports = chunk(fileName).imports.filter(() => true);
    chunk(fileName).isEntry = chunk(fileName).isEntry;
    chunk(fileName).fileName = chunk(fileName).fileName;
  }
  expect(collectChangedBundle(changed, bundle).changes).toStrictEqual({});
  expect(copiedOut(calls)).toStrictEqual([]);
});

test('only the properties a plugin changed are sent back', () => {
  const calls: string[] = [];
  const { bundle, changed, chunk } = outputBundle(calls);
  // What Vite's `vite:css-post` does with a pure CSS chunk.
  for (const fileName of ['a.js', 'b.js', 'style.js']) {
    chunk(fileName).imports = chunk(fileName).imports.filter((name) => name !== 'style.js');
  }
  chunk('a.js').code = '// a.js without style.js\n';
  delete bundle['style.js'];

  const { changes, deleted } = collectChangedBundle(changed, bundle);
  expect(changes).toStrictEqual({
    'a.js': {
      filename: 'a.js',
      isEntry: false,
      code: '// a.js without style.js\n',
      imports: ['b.js'],
    },
  });
  expect([...deleted]).toStrictEqual(['style.js']);
  expect(copiedOut(calls)).toStrictEqual([]);
});

test('a read map or array may be changed in place, so it is sent back with an assigned chunk', () => {
  const calls: string[] = [];
  const { bundle, changed, chunk } = outputBundle(calls);
  chunk('a.js').imports.push('extra.js');
  chunk('a.js').map!.names.push('extra');
  chunk('a.js').isEntry = true;
  chunk('b.js').map = null;
  chunk('style.js').fileName = 'renamed.js';

  const { changes } = collectChangedBundle(changed, bundle);
  expect(Object.keys(changes)).toStrictEqual(['a.js', 'b.js', 'style.js']);
  const a = changes['a.js'];
  expect(a).toMatchObject({
    filename: 'a.js',
    isEntry: true,
    imports: ['b.js', 'style.js', 'extra.js'],
  });
  expect(a).not.toHaveProperty('code');
  expect('map' in a && a.map?.inner).toMatchObject({ names: ['a', 'extra'], mappings: 'AAAAA' });
  expect(changes['b.js']).toStrictEqual({ filename: 'b.js', isEntry: false, map: null });
  expect(changes['style.js']).toStrictEqual({ filename: 'renamed.js', isEntry: false });
  // Only the maps the plugin read were copied out.
  expect(copiedOut(calls)).toStrictEqual(['a.js:getMap']);
});
