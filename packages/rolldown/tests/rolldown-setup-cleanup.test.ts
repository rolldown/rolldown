// @ts-nocheck These focused unit tests intentionally reach package source outside the test rootDir.
import { beforeEach, expect, test, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  acquireRuntimeLease: vi.fn(),
  bindingConstructionError: undefined as unknown,
  bindingConstructor: vi.fn(),
  callOptionsHook: vi.fn(async (option) => option),
  pluginPromiseThenCalls: 0,
  runtimeCapabilities: {
    devSupported: true,
    flavor: 'MultiThread',
    target: 'native',
    threads: true,
    wasi: false,
    watchSupported: true,
  },
}));

vi.mock('../src/binding.cjs', () => ({
  BindingBundler: class {
    constructor() {
      mocks.bindingConstructor();
      if (mocks.bindingConstructionError) throw mocks.bindingConstructionError;
    }
  },
  getRuntimeCapabilities: () => mocks.runtimeCapabilities,
}));

vi.mock('../src/plugin/plugin-driver', () => ({
  PluginDriver: {
    callOptionsHook: mocks.callOptionsHook,
  },
}));

vi.mock('../src/runtime-lifecycle', () => ({
  acquireRuntimeLease: mocks.acquireRuntimeLease,
  CloseCoordinator: class {},
}));

vi.mock('../src/utils/create-bundler-option', () => ({
  createBundlerOptions: vi.fn(),
}));

import { build } from '../src/api/build';
import { rolldown } from '../src/api/rolldown';

beforeEach(() => {
  mocks.acquireRuntimeLease.mockReset();
  mocks.bindingConstructionError = undefined;
  mocks.bindingConstructor.mockReset();
  mocks.callOptionsHook.mockClear();
  mocks.pluginPromiseThenCalls = 0;
  Object.assign(mocks.runtimeCapabilities, {
    devSupported: true,
    flavor: 'MultiThread',
    target: 'native',
    threads: true,
    wasi: false,
    watchSupported: true,
  });
});

test.each([
  [
    'rolldown',
    () =>
      rolldown({
        plugins: [createHangingPluginThenable(), createParallelDescriptor()],
      }),
  ],
  [
    'build output',
    () =>
      build({
        output: {
          plugins: [createParallelDescriptor()],
        },
        plugins: [createHangingPluginThenable()],
        write: false,
      }),
  ],
])('%s rejects descriptors before plugin promises or setup', async (_name, invoke) => {
  Object.assign(mocks.runtimeCapabilities, {
    target: 'wasi-threads',
    wasi: true,
  });

  await expect(invoke()).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
    feature: 'parallelPlugins',
  });

  expect(mocks.pluginPromiseThenCalls).toBe(0);
  expect(mocks.callOptionsHook).not.toHaveBeenCalled();
  expect(mocks.acquireRuntimeLease).not.toHaveBeenCalled();
  expect(mocks.bindingConstructor).not.toHaveBeenCalled();
});

test('rolldown does not enter native construction when runtime acquisition fails', async () => {
  const acquisitionError = new Error('runtime acquisition failed');
  mocks.acquireRuntimeLease.mockRejectedValue(acquisitionError);

  await expect(rolldown({ input: 'entry.js' })).rejects.toBe(acquisitionError);
  expect(mocks.bindingConstructor).not.toHaveBeenCalled();
});

test('rolldown rejects descriptors returned by the options hook before runtime setup', async () => {
  Object.assign(mocks.runtimeCapabilities, {
    target: 'wasi-threads',
    wasi: true,
  });
  mocks.callOptionsHook.mockResolvedValueOnce({
    plugins: [createParallelDescriptor()],
  });

  await expect(rolldown({ input: 'entry.js' })).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
    feature: 'parallelPlugins',
  });

  expect(mocks.acquireRuntimeLease).not.toHaveBeenCalled();
  expect(mocks.bindingConstructor).not.toHaveBeenCalled();
});

test('rolldown releases the transferred lease when native construction fails', async () => {
  const constructionError = new Error('bundle construction failed');
  const release = vi.fn();
  mocks.acquireRuntimeLease.mockResolvedValue({ release });
  mocks.bindingConstructionError = constructionError;

  await expect(rolldown({ input: 'entry.js' })).rejects.toBe(constructionError);
  expect(mocks.bindingConstructor).toHaveBeenCalledOnce();
  expect(release).toHaveBeenCalledOnce();
});

function createHangingPluginThenable() {
  return {
    // oxlint-disable-next-line unicorn/no-thenable -- verifies preflight before promise assimilation
    then() {
      mocks.pluginPromiseThenCalls += 1;
      return new Promise(() => {});
    },
  };
}

function createParallelDescriptor() {
  return {
    _parallel: {
      fileUrl: 'file:///project/old-package-plugin.mjs',
      options: {},
    },
  };
}

test('rolldown preserves construction and release failures', async () => {
  const constructionError = new Error('bundle construction failed');
  const releaseError = new Error('runtime release failed');
  const release = vi.fn(() => {
    throw releaseError;
  });
  mocks.acquireRuntimeLease.mockResolvedValue({ release });
  mocks.bindingConstructionError = constructionError;

  const error = await rolldown({ input: 'entry.js' }).catch((error: unknown) => error);

  expect(error).toBeInstanceOf(AggregateError);
  expect(error.errors).toEqual([constructionError, releaseError]);
  expect(error.cause).toBe(constructionError);
  expect(release).toHaveBeenCalledOnce();
});
