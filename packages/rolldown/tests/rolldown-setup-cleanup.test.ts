import { beforeEach, expect, test, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
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
    }
  },
  getRuntimeCapabilities: () => mocks.runtimeCapabilities,
}));

vi.mock('../src/plugin/plugin-driver', () => ({
  PluginDriver: {
    callOptionsHook: mocks.callOptionsHook,
  },
}));

vi.mock('../src/utils/create-bundler-option', () => ({
  createBundlerOptions: vi.fn(),
}));

import { rolldown } from '../src/api/rolldown';

beforeEach(() => {
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

test('rolldown rejects descriptors before plugin promises or setup', async () => {
  Object.assign(mocks.runtimeCapabilities, {
    target: 'wasi-threads',
    wasi: true,
    watchSupported: false,
  });

  await expect(
    rolldown({
      // @ts-expect-error a bare thenable is not the `Promise` a plugin option allows
      plugins: [createHangingPluginThenable(), createParallelDescriptor()],
    }),
  ).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
    feature: 'parallelPlugins',
  });

  expect(mocks.pluginPromiseThenCalls).toBe(0);
  expect(mocks.callOptionsHook).not.toHaveBeenCalled();
  expect(mocks.bindingConstructor).not.toHaveBeenCalled();
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
