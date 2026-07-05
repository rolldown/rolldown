import { expect, test, vi } from 'vitest';

const binding = vi.hoisted(() => {
  let resolveTransform!: (value: { code: string; errors: []; warnings: [] }) => void;
  let resolveMinify!: (value: { code: string }) => void;

  return {
    startAsyncRuntime: vi.fn(),
    shutdownAsyncRuntime: vi.fn(),
    enhancedTransform: vi.fn(
      () =>
        new Promise((resolve) => {
          resolveTransform = resolve;
        }),
    ),
    minify: vi.fn(
      () =>
        new Promise((resolve) => {
          resolveMinify = resolve;
        }),
    ),
    resolveTransform: () =>
      resolveTransform({
        code: 'const value = 1;\n',
        errors: [],
        warnings: [],
      }),
    resolveMinify: () => resolveMinify({ code: 'const value=1;' }),
  };
});

vi.mock('../src/binding.cjs', () => ({
  collapseSourcemaps: vi.fn(),
  enhancedTransform: binding.enhancedTransform,
  enhancedTransformSync: vi.fn(),
  getRuntimeCapabilities: vi.fn(() => ({ target: 'wasi-threads' })),
  minify: binding.minify,
  minifySync: vi.fn(),
  shutdownAsyncRuntime: binding.shutdownAsyncRuntime,
  startAsyncRuntime: binding.startAsyncRuntime,
}));

// @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
import { minify } from '../src/utils/minify';
// @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
import { transform } from '../src/utils/transform';

test('standalone async utilities retain independent runtime leases until settlement', async () => {
  const transformPromise = transform('input.ts', 'const value: number = 1;');
  expect(binding.startAsyncRuntime).not.toHaveBeenCalled();

  const minifyPromise = minify('input.js', 'const value = 1;');
  expect(binding.startAsyncRuntime).toHaveBeenCalledOnce();
  expect(binding.shutdownAsyncRuntime).not.toHaveBeenCalled();

  binding.resolveTransform();
  await expect(transformPromise).resolves.toMatchObject({ code: 'const value = 1;\n' });
  expect(binding.shutdownAsyncRuntime).toHaveBeenCalledOnce();

  binding.resolveMinify();
  await expect(minifyPromise).resolves.toMatchObject({ code: 'const value=1;' });
  expect(binding.shutdownAsyncRuntime).toHaveBeenCalledTimes(2);
});
