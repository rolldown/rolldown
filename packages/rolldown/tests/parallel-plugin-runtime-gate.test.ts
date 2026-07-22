// @ts-nocheck This focused unit test mocks the generated binding surface.
import { expect, test, vi } from 'vitest';

const binding = vi.hoisted(() => {
  const nativeSharedCapabilities: Record<string, unknown> = {
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
  };
  // The Node threaded-WASI artifact: reports `parallelPlugins: false` through
  // the support matrix while `import.meta.browserBuild` stays false.
  const threadedWasiCapabilities: Record<string, unknown> = {
    asyncRuntimeBuild: true,
    backend: 'shared',
    blockOnJsThreadSafe: false,
    devSupported: false,
    flavor: 'CurrentThread',
    target: 'wasi-threads',
    threads: false,
    timers: true,
    wasi: true,
    watchSupported: false,
  };
  return {
    capabilities: threadedWasiCapabilities as Record<string, unknown>,
    nativeSharedCapabilities,
    registryConstructions: [] as number[],
    threadedWasiCapabilities,
    workerSpawns: [] as unknown[],
  };
});

vi.mock('../src/binding.cjs', () => ({
  getRuntimeCapabilities: () => binding.capabilities,
  ParallelJsPluginRegistry: class {
    id = 1;
    constructor(workerCount: number) {
      binding.registryConstructions.push(workerCount);
    }
  },
}));

// Worker spawning is the side effect the runtime gate must precede; a real
// spawn would load the dist worker entry and the real binding.
vi.mock('node:worker_threads', () => ({
  Worker: class {
    constructor(...args: unknown[]) {
      binding.workerSpawns.push(args);
    }
    once(event: string, listener: (message: unknown) => void) {
      if (event === 'message') {
        queueMicrotask(() => listener({ type: 'success' }));
      }
      return this;
    }
    unref() {}
    terminate() {
      return Promise.resolve();
    }
  },
}));

// @ts-ignore These focused unit tests intentionally reach package source outside the test rootDir.
import { defineParallelPlugin } from '../src/plugin/parallel-plugin';
// @ts-ignore These focused unit tests intentionally reach package source outside the test rootDir.
import { getRuntimeSupport } from '../src/runtime-support';
// @ts-ignore These focused unit tests intentionally reach package source outside the test rootDir.
import { initializeParallelPlugins } from '../src/utils/initialize-parallel-plugins';

test('the support matrix reports parallel plugins unsupported on threaded WASI', () => {
  expect(getRuntimeSupport().parallelPlugins).toBe(false);
});

test('defineParallelPlugin fails on an unsupported artifact instead of returning a factory', () => {
  expect(() => defineParallelPlugin('file:///parallel-plugin.mjs')).toThrowError(
    expect.objectContaining({
      code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
      feature: 'parallelPlugins',
    }),
  );
});

test('materialized parallel markers fail before any registry or worker side effect', async () => {
  const materializedMarker = {
    name: 'materialized-parallel',
    _parallel: { fileUrl: 'file:///parallel-plugin.mjs', options: undefined },
  };

  await expect(initializeParallelPlugins([materializedMarker])).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
    feature: 'parallelPlugins',
  });
  expect(binding.registryConstructions).toEqual([]);
  expect(binding.workerSpawns).toEqual([]);
});

test('the gate does not affect supported native artifacts', async () => {
  binding.capabilities = binding.nativeSharedCapabilities;
  try {
    const factory = defineParallelPlugin('/parallel-plugin.mjs');
    const marker = factory({ mode: 'fast' });
    expect(marker._parallel.options).toEqual({ mode: 'fast' });

    const result = await initializeParallelPlugins([marker]);
    expect(result?.registry.id).toBe(1);
    expect(binding.registryConstructions).toHaveLength(1);
    expect(binding.workerSpawns.length).toBeGreaterThan(0);
    await result?.stopWorkers();
  } finally {
    binding.capabilities = binding.threadedWasiCapabilities;
  }
});
