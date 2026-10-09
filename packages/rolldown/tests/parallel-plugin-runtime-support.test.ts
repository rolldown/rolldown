// @ts-nocheck This focused unit test mocks the generated binding surface.
import { pathToFileURL } from 'node:url';

import { beforeEach, expect, test, vi } from 'vitest';

const binding = vi.hoisted(() => ({
  registryConstructions: 0,
  wasi: false,
}));

// The real module behind the two overrides: the binding conversion reads other exports
// at module scope.
vi.mock('../src/binding.cjs', async (importOriginal) => ({
  ...(await importOriginal()),
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

import { defineParallelPlugin } from '../src/plugin/parallel-plugin';
import { bindingifyInputOptions } from '../src/utils/bindingify-input-options';
import { initializeParallelPlugins } from '../src/utils/initialize-parallel-plugins';

beforeEach(() => {
  binding.registryConstructions = 0;
  binding.wasi = false;
});

test('defines parallel plugins for native bindings', () => {
  const createPlugin = defineParallelPlugin('/project/plugin.mjs');

  expect(createPlugin({ answer: 42 })).toMatchObject({
    _parallel: {
      fileUrl: pathToFileURL('/project/plugin.mjs').href,
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

// The browser build skips worker setup, so a descriptor an `options` hook adds after the
// `rolldown()` scan reaches the binding conversion, which must not drop it silently.
test('rejects a parallel descriptor at the binding conversion on WASI', () => {
  binding.wasi = true;

  expect(() =>
    bindingifyInputOptions(
      [{ _parallel: { fileUrl: 'file:///project/added-by-options-hook.mjs', options: {} } }],
      {},
      {},
      undefined,
      [],
      () => {},
      'info',
      false,
      undefined,
    ),
  ).toThrow(
    expect.objectContaining({
      code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
      feature: 'parallelPlugins',
    }),
  );
});
