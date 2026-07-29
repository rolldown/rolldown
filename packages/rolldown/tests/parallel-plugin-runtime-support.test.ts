// @ts-nocheck This focused unit test mocks the generated binding surface.
import { pathToFileURL } from 'node:url';

import { beforeEach, expect, test, vi } from 'vitest';

const binding = vi.hoisted(() => ({
  registryConstructions: 0,
  wasi: false,
}));

vi.mock('../src/binding.cjs', () => ({
  getRuntimeCapabilities: () => ({
    asyncRuntimeBuild: false,
    backend: 'tokio',
    blockOnJsThreadSafe: false,
    devSupported: true,
    flavor: 'MultiThread',
    target: binding.wasi ? 'wasi-threads' : 'native',
    threads: true,
    timers: true,
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
// @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
import { normalizePluginOption } from '../src/utils/normalize-plugin-option';

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

test('preflight skips an earlier array getter before rejecting a materialized descriptor', () => {
  binding.wasi = true;
  let getterCalls = 0;
  const plugins = Object.defineProperties([], {
    0: {
      get() {
        getterCalls += 1;
        throw new Error('preflight executed an array getter');
      },
    },
    1: {
      value: {
        _parallel: {
          fileUrl: 'file:///project/old-package-plugin.mjs',
          options: {},
        },
      },
    },
    length: { value: 2 },
  });

  expect(() => assertParallelPluginOptionsSupported(plugins)).toThrow(
    expect.objectContaining({
      code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
      feature: 'parallelPlugins',
    }),
  );
  expect(getterCalls).toBe(0);
});

test('preflight does not use indexed proxy gets while inspecting materialized entries', () => {
  binding.wasi = true;
  let getCalls = 0;
  const plugins = new Proxy(
    [
      undefined,
      {
        _parallel: {
          fileUrl: 'file:///project/old-package-plugin.mjs',
          options: {},
        },
      },
    ],
    {
      get() {
        getCalls += 1;
        throw new Error('preflight executed an indexed proxy get');
      },
      getOwnPropertyDescriptor(target, key) {
        if (key === '0') {
          throw new Error('preflight could not inspect an earlier proxy descriptor');
        }
        return Reflect.getOwnPropertyDescriptor(target, key);
      },
    },
  );

  expect(() => assertParallelPluginOptionsSupported(plugins)).toThrow(
    expect.objectContaining({
      code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
      feature: 'parallelPlugins',
    }),
  );
  expect(getCalls).toBe(0);
});

test('preflight contains an earlier revoked proxy before rejecting a later descriptor', () => {
  binding.wasi = true;
  const { proxy, revoke } = Proxy.revocable({}, {});
  revoke();

  expect(() =>
    assertParallelPluginOptionsSupported([
      proxy,
      {
        _parallel: {
          fileUrl: 'file:///project/old-package-plugin.mjs',
          options: {},
        },
      },
    ]),
  ).toThrow(
    expect.objectContaining({
      code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
      feature: 'parallelPlugins',
    }),
  );
});

test('accessor-produced descriptors are checked after normal plugin materialization', async () => {
  binding.wasi = true;
  let getterCalls = 0;
  const descriptor = {
    _parallel: {
      fileUrl: 'file:///project/old-package-plugin.mjs',
      options: {},
    },
  };
  const plugins = Object.defineProperties([], {
    0: {
      get() {
        getterCalls += 1;
        return descriptor;
      },
    },
    length: { value: 1 },
  });

  expect(() => assertParallelPluginOptionsSupported(plugins)).not.toThrow();
  expect(getterCalls).toBe(0);

  const normalized = await normalizePluginOption(plugins);
  expect(getterCalls).toBe(1);
  await expect(initializeParallelPlugins(normalized)).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
    feature: 'parallelPlugins',
  });
  expect(binding.registryConstructions).toBe(0);
});
