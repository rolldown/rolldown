// @ts-nocheck This focused unit test mocks the generated binding surface.
import { beforeEach, expect, test, vi } from 'vitest';

const binding = vi.hoisted(() => ({
  registryConstructions: 0,
  wasi: false,
}));

vi.mock('../src/binding.cjs', () => ({
  getRuntimeCapabilities: () => ({
    devSupported: true,
    flavor: 'MultiThread',
    target: binding.wasi ? 'wasi-threads' : 'native',
    threads: true,
    wasi: binding.wasi,
    watchSupported: !binding.wasi,
  }),
  ParallelJsPluginRegistry: class {
    constructor() {
      binding.registryConstructions += 1;
    }
  },
}));

// @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
import {
  assertParallelPluginOptionsSupported,
  defineParallelPlugin,
} from '../src/plugin/parallel-plugin';
// @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
import { initializeParallelPlugins } from '../src/utils/initialize-parallel-plugins';

beforeEach(() => {
  binding.registryConstructions = 0;
  binding.wasi = false;
});

test('defines parallel plugins for native bindings', () => {
  const createPlugin = defineParallelPlugin('/project/plugin.mjs');

  expect(createPlugin({ answer: 42 })).toMatchObject({
    _parallel: {
      fileUrl: 'file:///project/plugin.mjs',
      options: { answer: 42 },
    },
  });
});

test('rejects parallel plugins before worker setup on WASI bindings', () => {
  binding.wasi = true;

  expect(() => defineParallelPlugin('/project/plugin.mjs')).toThrow(
    expect.objectContaining({
      code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
      feature: 'parallelPlugins',
    }),
  );
});

test('rejects fabricated parallel descriptors before registry or worker creation on WASI', async () => {
  binding.wasi = true;

  await expect(
    initializeParallelPlugins([
      {
        _parallel: {
          fileUrl: 'file:///project/old-package-plugin.mjs',
          options: {},
        },
      },
    ]),
  ).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
    feature: 'parallelPlugins',
  });
  expect(binding.registryConstructions).toBe(0);
});

test('preflight handles circular plugin arrays without losing nested descriptors', () => {
  binding.wasi = true;
  const plugins: unknown[] = [];
  plugins.push(plugins, {
    _parallel: {
      fileUrl: 'file:///project/old-package-plugin.mjs',
      options: {},
    },
  });

  expect(() => assertParallelPluginOptionsSupported(plugins)).toThrow(
    expect.objectContaining({
      code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
      feature: 'parallelPlugins',
    }),
  );
});
