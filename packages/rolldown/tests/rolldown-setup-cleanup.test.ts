// @ts-nocheck These focused unit tests intentionally reach package source outside the test rootDir.
import { beforeEach, expect, test, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  bindingConstructionError: undefined as unknown,
  bindingConstructor: vi.fn(),
  callOptionsHook: vi.fn(async (option) => option),
  pluginPromiseThenCalls: 0,
  runtimeCapabilities: {
    asyncRuntimeBuild: true,
    backend: 'shared',
    blockOnJsThreadSafe: false,
    devSupported: true,
    flavor: 'MultiThread',
    target: 'native',
    threads: true,
    timers: true,
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
  CloseCoordinator: class {},
}));

vi.mock('../src/utils/create-bundler-option', () => ({
  createBundlerOptions: vi.fn(),
}));

import { build } from '../src/api/build';
import { rolldown } from '../src/api/rolldown';

beforeEach(() => {
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
    asyncRuntimeBuild: true,
    backend: 'shared',
    target: 'wasi-threads',
    wasi: true,
    watchSupported: false,
  });

  await expect(invoke()).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
    feature: 'parallelPlugins',
  });

  expect(mocks.pluginPromiseThenCalls).toBe(0);
  expect(mocks.callOptionsHook).not.toHaveBeenCalled();
  expect(mocks.bindingConstructor).not.toHaveBeenCalled();
});

test('rolldown rejects descriptors returned by the options hook before runtime setup', async () => {
  Object.assign(mocks.runtimeCapabilities, {
    asyncRuntimeBuild: true,
    backend: 'shared',
    target: 'wasi-threads',
    wasi: true,
    watchSupported: false,
  });
  mocks.callOptionsHook.mockResolvedValueOnce({
    plugins: [createParallelDescriptor()],
  });

  await expect(rolldown({ input: 'entry.js' })).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
    feature: 'parallelPlugins',
  });

  expect(mocks.bindingConstructor).not.toHaveBeenCalled();
});

test('rolldown propagates a native construction failure', async () => {
  const constructionError = new Error('bundle construction failed');
  mocks.bindingConstructionError = constructionError;

  await expect(rolldown({ input: 'entry.js' })).rejects.toBe(constructionError);
  expect(mocks.bindingConstructor).toHaveBeenCalledOnce();
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
