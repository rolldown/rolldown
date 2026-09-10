// @ts-nocheck This focused unit test intentionally reaches package source outside the test rootDir.
import { beforeEach, expect, test, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  bindingifyInputOptions: vi.fn(),
  initializeParallelPlugins: vi.fn(),
  runtimeCapabilities: {
    asyncRuntimeBuild: false,
    backend: 'tokio',
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
  getRuntimeCapabilities: () => mocks.runtimeCapabilities,
  ParallelJsPluginRegistry: class {},
}));

vi.mock('../src/utils/bindingify-input-options', () => ({
  bindingifyInputOptions: mocks.bindingifyInputOptions,
}));

vi.mock('../src/utils/initialize-parallel-plugins', async (importOriginal) => ({
  ...(await importOriginal()),
  initializeParallelPlugins: mocks.initializeParallelPlugins,
}));

import { createBundlerOptions } from '../src/utils/create-bundler-option';
import { getRetryableCleanup } from '../src/utils/retryable-cleanup';

beforeEach(() => {
  mocks.bindingifyInputOptions.mockReset();
  mocks.initializeParallelPlugins.mockReset();
  Object.assign(mocks.runtimeCapabilities, {
    devSupported: true,
    flavor: 'MultiThread',
    target: 'native',
    threads: true,
    wasi: false,
    watchSupported: true,
  });
});

test('bundler-option setup retries the retained worker after termination first rejects', async () => {
  const setupError = new Error('binding option conversion failed');
  const cleanupError = new Error('worker termination failed');
  const stopWorkers = vi
    .fn<() => Promise<void>>()
    .mockRejectedValueOnce(cleanupError)
    .mockResolvedValue(undefined);
  mocks.initializeParallelPlugins.mockResolvedValue({
    registry: {},
    stopWorkers,
  });
  mocks.bindingifyInputOptions.mockImplementation(() => {
    throw setupError;
  });

  const error = await createBundlerOptions({}, {}, false).catch((error: unknown) => error);

  expect(error).toBeInstanceOf(AggregateError);
  expect((error as AggregateError).errors).toEqual([setupError, cleanupError]);
  expect(stopWorkers).toHaveBeenCalledTimes(2);
  expect(getRetryableCleanup(error)).toBeUndefined();
});

test('post-worker option access failures terminate the initialized worker pool', async () => {
  const setupError = new Error('experimental option getter failed');
  const stopWorkers = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
  mocks.initializeParallelPlugins.mockResolvedValue({
    registry: {},
    stopWorkers,
  });
  const inputOptions = Object.defineProperty({}, 'experimental', {
    get() {
      throw setupError;
    },
  });

  await expect(createBundlerOptions(inputOptions, {}, false)).rejects.toBe(setupError);

  expect(stopWorkers).toHaveBeenCalledOnce();
  expect(mocks.bindingifyInputOptions).not.toHaveBeenCalled();
});

test('outputOptions rejects an injected descriptor before assimilating a preceding thenable', async () => {
  Object.assign(mocks.runtimeCapabilities, {
    asyncRuntimeBuild: true,
    backend: 'shared',
    target: 'wasi-threads',
    wasi: true,
    watchSupported: false,
  });
  let pluginPromiseThenCalls = 0;
  const optionPromise = createBundlerOptions(
    {
      plugins: [
        {
          name: 'inject-parallel-output',
          outputOptions(options) {
            return {
              ...options,
              plugins: [
                {
                  // oxlint-disable-next-line unicorn/no-thenable -- verifies preflight before promise assimilation
                  then() {
                    pluginPromiseThenCalls += 1;
                    return new Promise(() => {});
                  },
                },
                {
                  _parallel: {
                    fileUrl: 'file:///project/old-package-plugin.mjs',
                    options: {},
                  },
                },
              ],
            };
          },
        },
      ],
    },
    {},
    false,
  );

  await expect(
    withTimeout(optionPromise, 'outputOptions parallel descriptor preflight'),
  ).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
    feature: 'parallelPlugins',
  });
  expect(pluginPromiseThenCalls).toBe(0);
  expect(mocks.initializeParallelPlugins).not.toHaveBeenCalled();
  expect(mocks.bindingifyInputOptions).not.toHaveBeenCalled();
});

test.each([
  [
    'same-identity cycle',
    () => {
      let options: object;
      options = new Proxy(
        {},
        {
          getPrototypeOf() {
            return options;
          },
        },
      );
      return options;
    },
    /Prototype cycle detected while inspecting callback options/,
  ],
  [
    'fresh-proxy chain',
    () => {
      const createOptions = (): object =>
        new Proxy(
          {},
          {
            getPrototypeOf() {
              return createOptions();
            },
          },
        );
      return createOptions();
    },
    /Prototype chain exceeded 256 objects while inspecting callback options/,
  ],
])('rejects a non-terminating input option prototype %s', async (_, createOptions, expected) => {
  await expect(createBundlerOptions(createOptions(), {}, false)).rejects.toThrow(expected);
  expect(mocks.initializeParallelPlugins).not.toHaveBeenCalled();
  expect(mocks.bindingifyInputOptions).not.toHaveBeenCalled();
});

// The two cases above throw inside `hasDefinedProperty`, which walks the chain
// before `readPropertyOnce` is ever reached, so they do not pin the walk that
// `readPropertyOnce` performs and deliberately discards. That call is not dead
// code: it is what makes an unbounded chain be REFUSED BEFORE the property is
// read, so no user code on the chain runs. Dropping it still yields the same
// TypeError - the snapshot's `enumerable` walk throws right after - but only
// after `Reflect.get` has already walked into and executed the accessor.
//
// This case slips past `hasDefinedProperty`: the first plugin owns
// `outputOptions`, so `outputOptionPlugins.some(...)` short-circuits and the
// over-deep second plugin is first traversed by `readPropertyOnce` itself.
test('refuses an over-deep plugin prototype chain before reading the hook off it', async () => {
  let accessorCalls = 0;
  let overDeepPlugin = Object.create(null, {
    outputOptions: {
      configurable: true,
      get() {
        accessorCalls += 1;
        return (options) => options;
      },
    },
  });
  for (let depth = 0; depth < 300; depth++) {
    overDeepPlugin = Object.create(overDeepPlugin);
  }

  await expect(
    createBundlerOptions(
      {
        // Owned so `hasUserLogCallback` short-circuits before walking any plugin.
        onLog() {},
        plugins: [
          { name: 'owns-output-options', outputOptions: (options) => options },
          overDeepPlugin,
        ],
      },
      {},
      false,
    ),
  ).rejects.toThrow(/Prototype chain exceeded 256 objects while inspecting callback options/);
  expect(accessorCalls).toBe(0);
  expect(mocks.initializeParallelPlugins).not.toHaveBeenCalled();
  expect(mocks.bindingifyInputOptions).not.toHaveBeenCalled();
});

function withTimeout<T>(promise: Promise<T>, operation: string): Promise<T> {
  const timeoutMs = 2_000;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      reject(new Error(`${operation} timed out after ${timeoutMs}ms`));
    }, timeoutMs);
  });
  return Promise.race([promise, timeout]).finally(() => {
    if (timer) clearTimeout(timer);
  });
}
