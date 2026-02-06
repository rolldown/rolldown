import { minify, minifySync } from 'rolldown/utils';
import { expect, test } from 'vitest';

// Generated from `transformSync('original.ts', 'const a: number = 1;', { sourcemap: true })`
const transformedCode = 'const a = 1;\n';
const transformedMap = {
  version: 3 as const,
  sources: ['original.ts'],
  sourcesContent: ['const a: number = 1;'],
  names: [],
  mappings: 'AAAA,MAAM,IAAY',
};

test('minify', async () => {
  const result = await minify('foo.js', 'const a = 1;');
  expect(result.code).toBe('const a=1;');
  expect(result.map).toBeUndefined();
});

test('minify with sourcemap', async () => {
  const result = await minify('foo.js', 'const a = 1;', { sourcemap: true });
  expect(result.code).toBe('const a=1;');
  expect(result.map).toBeDefined();
  expect(result.map?.sources).toEqual(['foo.js']);
});

test('minify with inputMap', async () => {
  const result = await minify('original.js', transformedCode, {
    sourcemap: true,
    inputMap: transformedMap,
  });

  expect(result.code).toBe('const a=1;');
  expect(result.map).toBeDefined();
  expect(result.map?.sources).toEqual(['original.ts']);
  expect(result.map?.sourcesContent).toEqual(['const a: number = 1;']);
});

test('minifySync', () => {
  const result = minifySync('foo.js', 'const a = 1;');
  expect(result.code).toBe('const a=1;');
  expect(result.map).toBeUndefined();
});

test('minifySync with sourcemap', () => {
  const result = minifySync('foo.js', 'const a = 1;', { sourcemap: true });
  expect(result.code).toBe('const a=1;');
  expect(result.map).toBeDefined();
  expect(result.map?.sources).toEqual(['foo.js']);
});

test('minifySync with inputMap', () => {
  const result = minifySync('original.js', transformedCode, {
    sourcemap: true,
    inputMap: transformedMap,
  });

  expect(result.code).toBe('const a=1;');
  expect(result.map).toBeDefined();
  expect(result.map?.sources).toEqual(['original.ts']);
  expect(result.map?.sourcesContent).toEqual(['const a: number = 1;']);
});
