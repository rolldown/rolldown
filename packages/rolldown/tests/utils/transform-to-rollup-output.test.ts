import { expect, test, vi } from 'vitest';
import type { BindingOutputs } from '../../src/binding.cjs';
import type { MinimalPluginContext } from '../../src/plugin/minimal-plugin-context';
import type { OutputAsset, OutputChunk, SourceMap } from '../../src/types/rolldown-output';
import {
  type ChangedOutputs,
  collectChangedBundle,
  transformToOutputBundle,
  transformToRollupSourceMap,
} from '../../src/utils/transform-to-rollup-output';

function mutableBundle() {
  const chunk = {
    getFileName: () => 'entry.js',
    getName: () => 'entry',
    getExports: () => [],
    getIsEntry: () => true,
    getFacadeModuleId: () => 'entry',
    getIsDynamicEntry: () => false,
    getSourcemapFileName: () => 'entry.js.map',
    getPreliminaryFileName: () => 'entry.js',
    getCode: vi.fn(() => 'console.log("entry");'),
    getImports: vi.fn(() => []),
    getDynamicImports: vi.fn(() => []),
    getModuleIds: vi.fn(() => ['entry']),
    getModules: vi.fn(() => ({})),
    getMap: vi.fn(() =>
      JSON.stringify({
        version: 3,
        file: 'entry.js',
        mappings: 'AAAA',
        sources: ['entry.js'],
        sourcesContent: ['console.log("entry");'],
        names: [],
        ignoreList: [0],
        debugId: 'original',
      }),
    ),
  };
  const asset = {
    getFileName: () => 'bytes.bin',
    getName: () => undefined,
    getNames: () => [],
    getOriginalFileName: () => undefined,
    getOriginalFileNames: () => [],
    getSource: vi.fn(() => ({ inner: Buffer.from([1, 2, 3]) })),
  };
  const changed: ChangedOutputs = { updated: new Set(), deleted: new Set() };
  const bundle = transformToOutputBundle(
    { warn: vi.fn() } as unknown as MinimalPluginContext,
    { chunks: [chunk], assets: [asset] } as unknown as BindingOutputs,
    changed,
  );
  return { bundle, changed, chunk, asset };
}

test('reading mutable output properties does not transfer unchanged output', () => {
  const { bundle, changed, chunk, asset } = mutableBundle();
  expect(chunk.getMap).not.toHaveBeenCalled();
  expect(asset.getSource).not.toHaveBeenCalled();
  const outputChunk = bundle['entry.js'] as OutputChunk;
  const outputAsset = bundle['bytes.bin'] as OutputAsset;
  expect(outputChunk.map).toBe(outputChunk.map);
  expect(outputChunk.imports).toBe(outputChunk.imports);
  expect(outputChunk.dynamicImports).toEqual([]);
  expect(outputAsset.source).toBe(outputAsset.source);
  expect(outputAsset.names).toEqual([]);
  expect(outputAsset.originalFileNames).toEqual([]);
  expect(collectChangedBundle(changed, bundle).changes).toEqual({});
  expect(chunk.getMap).toHaveBeenCalledTimes(1);
  expect(asset.getSource).toHaveBeenCalledTimes(1);
  expect(chunk.getCode).not.toHaveBeenCalled();
  expect(chunk.getModules).not.toHaveBeenCalled();
  expect(chunk.getModuleIds).not.toHaveBeenCalled();
});

test.each(['imports', 'dynamicImports'] as const)('tracks in-place chunk %s edits', (key) => {
  const { bundle, changed } = mutableBundle();
  (bundle['entry.js'] as OutputChunk)[key].push('changed.js');
  expect(collectChangedBundle(changed, bundle).changes).toHaveProperty('entry.js');
});

test.each(['names', 'originalFileNames'] as const)('tracks in-place asset %s edits', (key) => {
  const { bundle, changed } = mutableBundle();
  (bundle['bytes.bin'] as OutputAsset)[key].push('changed.bin');
  expect(collectChangedBundle(changed, bundle).changes).toHaveProperty('bytes.bin');
});

test.each<[string, (map: SourceMap) => void]>([
  [
    'array element',
    (map) => {
      map.sources[0] = 'changed.js';
    },
  ],
  [
    'array method',
    (map) => {
      map.names.push('name');
    },
  ],
  [
    'array replacement',
    (map) => {
      map.sourcesContent = ['changed'];
    },
  ],
  [
    'scalar property',
    (map) => {
      map.mappings = '';
    },
  ],
  [
    'property addition',
    (map) => {
      Object.assign(map, { sourceRoot: 'source' });
    },
  ],
  [
    'property deletion',
    (map) => {
      delete map.debugId;
    },
  ],
  [
    'legacy ignore list',
    (map) => {
      map.x_google_ignoreList!.push(1);
    },
  ],
])('tracks a source map %s edit without reassigning the map', (_name, mutate) => {
  const { bundle, changed } = mutableBundle();
  mutate((bundle['entry.js'] as OutputChunk).map!);
  expect(collectChangedBundle(changed, bundle).changes).toHaveProperty('entry.js');
});

test.each<[string, (source: Uint8Array) => void]>([
  [
    'indexed',
    (source) => {
      source[0] = 9;
    },
  ],
  [
    'method',
    (source) => {
      source.set([9]);
    },
  ],
  [
    'subarray',
    (source) => {
      source.subarray(0, 1).fill(9);
    },
  ],
  [
    'buffer view',
    (source) => {
      new DataView(source.buffer, source.byteOffset).setUint8(0, 9);
    },
  ],
])('tracks a binary asset %s edit while preserving native typed arrays', (_name, mutate) => {
  const { bundle, changed } = mutableBundle();
  const source = (bundle['bytes.bin'] as OutputAsset).source as Uint8Array;
  expect(ArrayBuffer.isView(source)).toBe(true);
  expect(source).toBe((bundle['bytes.bin'] as OutputAsset).source);
  mutate(source);
  expect(collectChangedBundle(changed, bundle).changes).toHaveProperty('bytes.bin');
});

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
