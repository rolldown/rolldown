import { expect, test, vi } from 'vitest';
import { transformToRollupSourceMap } from '../../src/utils/transform-to-rollup-output';

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
