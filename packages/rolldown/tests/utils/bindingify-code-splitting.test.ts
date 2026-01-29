import { rolldown } from 'rolldown';
import { expect, test, vi, beforeEach, afterEach } from 'vitest';

let consoleSpy: ReturnType<typeof vi.spyOn>;

beforeEach(() => {
  consoleSpy = vi.spyOn(console, 'log').mockImplementation(() => {});
});

afterEach(() => {
  consoleSpy.mockRestore();
});

test('codeSplitting: false inlines dynamic imports', async () => {
  const bundle = await rolldown({
    input: './fixtures/tree-shake/inline-dynamic-imports/main.js',
    cwd: import.meta.dirname + '/..',
  });
  const result = await bundle.generate({
    codeSplitting: false,
  });
  // Should produce a single chunk when code splitting is disabled
  expect(result.output.length).toBe(1);
});

test('codeSplitting: false with manualChunks throws error', async () => {
  const bundle = await rolldown({
    input: './fixtures/tree-shake/inline-dynamic-imports/main.js',
    cwd: import.meta.dirname + '/..',
  });
  await expect(
    bundle.generate({
      codeSplitting: false,
      manualChunks: () => 'vendor',
    }),
  ).rejects.toThrow(
    'Invalid configuration: "output.manualChunks" cannot be used when "output.codeSplitting" is set to false.',
  );
});

test('codeSplitting: false with advancedChunks warns and ignores', async () => {
  const bundle = await rolldown({
    input: './fixtures/tree-shake/inline-dynamic-imports/main.js',
    cwd: import.meta.dirname + '/..',
  });
  await bundle.generate({
    codeSplitting: false,
    advancedChunks: { minSize: 1000 },
  });
  expect(consoleSpy).toHaveBeenCalledWith(
    '`advancedChunks` option is ignored because `codeSplitting` is set to `false`.',
  );
});

test('codeSplitting: false with inlineDynamicImports warns and ignores', async () => {
  const bundle = await rolldown({
    input: './fixtures/tree-shake/inline-dynamic-imports/main.js',
    cwd: import.meta.dirname + '/..',
  });
  await bundle.generate({
    codeSplitting: false,
    inlineDynamicImports: true,
  });
  expect(consoleSpy).toHaveBeenCalledWith(
    '`inlineDynamicImports` option is ignored because `codeSplitting: false` is set.',
  );
});

test('codeSplitting: true with inlineDynamicImports warns and ignores', async () => {
  const bundle = await rolldown({
    input: './fixtures/tree-shake/inline-dynamic-imports/main.js',
    cwd: import.meta.dirname + '/..',
  });
  await bundle.generate({
    codeSplitting: true,
    inlineDynamicImports: true,
  });
  expect(consoleSpy).toHaveBeenCalledWith(
    '`inlineDynamicImports` option is ignored because `codeSplitting: true` is set.',
  );
});

test('codeSplitting: undefined with inlineDynamicImports shows deprecation warning', async () => {
  const bundle = await rolldown({
    input: './fixtures/tree-shake/inline-dynamic-imports/main.js',
    cwd: import.meta.dirname + '/..',
  });
  await bundle.generate({
    inlineDynamicImports: true,
  });
  expect(consoleSpy).toHaveBeenCalledWith(
    '`inlineDynamicImports` option is deprecated, please use `codeSplitting: false` instead.',
  );
});

test('codeSplitting: object with inlineDynamicImports warns and ignores', async () => {
  const bundle = await rolldown({
    input: './fixtures/tree-shake/inline-dynamic-imports/main.js',
    cwd: import.meta.dirname + '/..',
  });
  await bundle.generate({
    codeSplitting: { minSize: 1000 },
    inlineDynamicImports: true,
  });
  expect(consoleSpy).toHaveBeenCalledWith(
    '`inlineDynamicImports` option is ignored because the `codeSplitting` option is specified.',
  );
});

test('codeSplitting: object with advancedChunks warns and ignores', async () => {
  const bundle = await rolldown({
    input: './fixtures/tree-shake/inline-dynamic-imports/main.js',
    cwd: import.meta.dirname + '/..',
  });
  await bundle.generate({
    codeSplitting: { minSize: 1000 },
    advancedChunks: { minSize: 2000 },
  });
  expect(consoleSpy).toHaveBeenCalledWith(
    '`advancedChunks` option is ignored because the `codeSplitting` option is specified.',
  );
});

test('advancedChunks without codeSplitting shows deprecation warning', async () => {
  const bundle = await rolldown({
    input: './fixtures/tree-shake/inline-dynamic-imports/main.js',
    cwd: import.meta.dirname + '/..',
  });
  await bundle.generate({
    advancedChunks: { minSize: 1000 },
  });
  expect(consoleSpy).toHaveBeenCalledWith(
    '`advancedChunks` option is deprecated, please use `codeSplitting` instead.',
  );
});

test('manualChunks without codeSplitting works', async () => {
  const bundle = await rolldown({
    input: './fixtures/tree-shake/inline-dynamic-imports/main.js',
    cwd: import.meta.dirname + '/..',
  });
  const result = await bundle.generate({
    manualChunks: (id) => {
      if (id.includes('dynamic')) {
        return 'dynamic';
      }
    },
  });
  // Should produce multiple chunks with manualChunks
  expect(result.output.length).toBeGreaterThan(1);
});
