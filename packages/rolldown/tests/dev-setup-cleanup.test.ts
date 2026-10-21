import { beforeEach, expect, test, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  bindingConstructionError: undefined as unknown,
  bindingConstructions: 0,
  callOptionsHook: vi.fn(async (option) => option),
  createBundlerOptions: vi.fn(),
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
  BindingDevEngine: class {
    constructor() {
      mocks.bindingConstructions += 1;
      if (mocks.bindingConstructionError) throw mocks.bindingConstructionError;
    }
  },
  BindingRebuildStrategy: {
    Always: 'always',
    Never: 'never',
  },
  getRuntimeCapabilities: () => mocks.runtimeCapabilities,
}));

vi.mock('../src/plugin/plugin-driver', () => ({
  PluginDriver: {
    callOptionsHook: mocks.callOptionsHook,
  },
}));

vi.mock('../src/utils/create-bundler-option', () => ({
  createBundlerOptions: mocks.createBundlerOptions,
}));

import { DevEngine } from '../src/api/dev/dev-engine';

beforeEach(() => {
  mocks.bindingConstructionError = undefined;
  mocks.bindingConstructions = 0;
  mocks.callOptionsHook.mockClear();
  mocks.createBundlerOptions.mockReset();
  Object.assign(mocks.runtimeCapabilities, {
    devSupported: true,
    flavor: 'MultiThread',
    target: 'native',
    threads: true,
    wasi: false,
    watchSupported: true,
  });
});

test('dev rejects CurrentThread before callbacks or setup', async () => {
  Object.assign(mocks.runtimeCapabilities, {
    devSupported: false,
    flavor: 'CurrentThread',
    threads: false,
  });
  const onOutput = vi.fn();

  await expect(DevEngine.create({}, {}, { onOutput })).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
    feature: 'dev',
  });

  expect(onOutput).not.toHaveBeenCalled();
  expect(mocks.callOptionsHook).not.toHaveBeenCalled();
  expect(mocks.createBundlerOptions).not.toHaveBeenCalled();
  expect(mocks.bindingConstructions).toBe(0);
});
