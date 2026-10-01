import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { describe, expect, test } from 'vitest';

import { insertWasiPoolWorkerPreload } from '../binding-loader-codegen';

const generatedWasiNodeLoader = readFileSync(
  fileURLToPath(new URL('../src/rolldown-binding.wasi.cjs', import.meta.url)),
  'utf8',
);

describe('generated WASI loader lifecycle', () => {
  test('uses a fresh context per evaluation and prepares each context once', () => {
    const contexts: Array<{ destroy(): void }> = [];
    const cleanupEvents: string[] = [];
    const cleanups: Array<() => void> = [];

    for (const id of [1, 2]) {
      const execution = executeGeneratedWasiNodeLoader({
        createContext() {
          const context = {
            destroy() {
              cleanupEvents.push(`destroy:${id}`);
            },
          };
          contexts.push(context);
          return context;
        },
        prepareCleanup() {
          cleanupEvents.push(`prepare:${id}`);
        },
      });
      cleanups.push(() => execution.cleanup());
    }

    expect(contexts[0]).not.toBe(contexts[1]);
    cleanups[0]();
    cleanups[0]();
    cleanups[1]();
    cleanups[1]();
    expect(cleanupEvents).toEqual(['prepare:1', 'destroy:1', 'prepare:2', 'destroy:2']);
  });

  test('raw context destroy prepares and tears down exactly once with the real emnapi runtime', () => {
    // Uses the actual pinned @emnapi/runtime Context (resolved from the
    // rolldown package, exactly what generated loaders load) instead of a
    // mock: Context.destroy() drains its cleanup queue destructively, so a
    // second delegation from the loader's exit-time helper must be a no-op.
    const emnapiRequire = createRequire(
      fileURLToPath(new URL('../src/rolldown-binding.wasi.cjs', import.meta.url)),
    );
    const { createContext } = emnapiRequire('@emnapi/runtime') as {
      createContext: () => {
        addCleanupHook(envObject: unknown, fn: (arg: number) => void, arg: number): void;
        destroy(): void;
      };
    };
    const cleanupEvents: string[] = [];
    let context: ReturnType<typeof createContext> | undefined;
    const execution = executeGeneratedWasiNodeLoader({
      createContext() {
        context = createContext();
        context.addCleanupHook(undefined, () => cleanupEvents.push('teardown'), 0);
        return context;
      },
      prepareCleanup() {
        cleanupEvents.push('prepare');
      },
    });

    // An embedder may destroy the emnapi context directly, bypassing the
    // loader's __destroyEmnapiContext helper. The generated destroy wrapper
    // must run the wasm-side cleanup preparation before the teardown.
    context!.destroy();
    expect(cleanupEvents).toEqual(['prepare', 'teardown']);

    // The loader's exit-time cleanup shares the preparation latch and delegates
    // into emnapi's already-drained queue: prepare and teardown each ran once.
    expect(() => execution.cleanup()).not.toThrow();
    expect(cleanupEvents).toEqual(['prepare', 'teardown']);
  });
});

// See internal-docs/async-runtime/implementation.md (section 13, "Pool worker preload").
describe('generated WASI loader pool worker preload', () => {
  test('the committed loader carries it once, and inserting it again changes nothing', () => {
    expect(generatedWasiNodeLoader.split('function __preloadWasiPoolWorkers(').length - 1).toBe(1);
    expect(insertWasiPoolWorkerPreload(generatedWasiNodeLoader)).toBe(generatedWasiNodeLoader);

    const start = generatedWasiNodeLoader.indexOf("// Inserted by rolldown's build");
    const call = '__preloadWasiPoolWorkers(__napiModule.exports)\n} catch {}\n';
    const end = generatedWasiNodeLoader.indexOf(call, start) + call.length;
    const unpatched = generatedWasiNodeLoader.slice(0, start) + generatedWasiNodeLoader.slice(end);
    expect(unpatched).not.toContain('__preloadWasiPoolWorkers');
    expect(insertWasiPoolWorkerPreload(unpatched)).toBe(generatedWasiNodeLoader);
  });

  test('a loader that no longer matches its anchors fails the build', () => {
    expect(() =>
      insertWasiPoolWorkerPreload(
        generatedWasiNodeLoader.replace('reuseWorker: true,', 'reuseWorker: false,'),
      ),
    ).toThrow(/WASI pool worker preload/);
    expect(() =>
      insertWasiPoolWorkerPreload(
        generatedWasiNodeLoader.replace(
          '}\nmodule.exports = __napiModule.exports\n',
          '}\n\nmodule.exports = __napiModule.exports\n',
        ),
      ),
    ).toThrow(/WASI pool worker preload placement/);
  });

  test('puts one loading worker per MultiThread worker into the pool', () => {
    const manager = createThreadManagerStub();
    executeGeneratedWasiNodeLoader({
      createContext: () => ({ destroy() {} }),
      threadManager: manager,
      runtimeConfig: () => ({ flavor: 'MultiThread', workerThreads: 3 }),
    });
    expect(manager.allocated).toHaveLength(3);
    expect(manager.loaded).toEqual(manager.allocated);
    expect(manager.unusedWorkers).toEqual(manager.allocated);
  });

  test('preloads nothing under CurrentThread', () => {
    const manager = createThreadManagerStub();
    executeGeneratedWasiNodeLoader({
      createContext: () => ({ destroy() {} }),
      threadManager: manager,
      runtimeConfig: () => ({ flavor: 'CurrentThread', workerThreads: 1 }),
    });
    expect(manager.allocated).toHaveLength(0);
  });

  test('drops a worker that failed to load from the pool', async () => {
    const manager = createThreadManagerStub((worker) =>
      worker.id === 1 ? Promise.reject(new Error('load failed')) : Promise.resolve(worker),
    );
    executeGeneratedWasiNodeLoader({
      createContext: () => ({ destroy() {} }),
      threadManager: manager,
      runtimeConfig: () => ({ flavor: 'MultiThread', workerThreads: 3 }),
    });
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(manager.unusedWorkers.map((worker) => worker.id)).toEqual([0, 2]);
  });

  test('stops at a synchronous failure and terminates that worker', () => {
    const manager = createThreadManagerStub((worker) => {
      if (worker.id === 1) throw new Error('load threw');
      return Promise.resolve(worker);
    });
    executeGeneratedWasiNodeLoader({
      createContext: () => ({ destroy() {} }),
      threadManager: manager,
      runtimeConfig: () => ({ flavor: 'MultiThread', workerThreads: 3 }),
    });
    expect(manager.allocated.map((worker) => worker.id)).toEqual([0, 1]);
    expect(manager.unusedWorkers.map((worker) => worker.id)).toEqual([0]);
    expect(manager.terminated.map((worker) => worker.id)).toEqual([1]);
  });

  test('never fails the load', () => {
    const manager = createThreadManagerStub();
    const execution = executeGeneratedWasiNodeLoader({
      createContext: () => ({ destroy() {} }),
      threadManager: manager,
      runtimeConfig: () => {
        throw new Error('no config');
      },
    });
    expect(manager.allocated).toHaveLength(0);
    expect(execution.exports.getAsyncRuntimeConfig).toBeTypeOf('function');
  });
});

interface PoolWorkerStub {
  id: number;
}

function createThreadManagerStub(
  load: (worker: PoolWorkerStub) => Promise<unknown> = (worker) => Promise.resolve(worker),
) {
  const manager = {
    unusedWorkers: [] as PoolWorkerStub[],
    allocated: [] as PoolWorkerStub[],
    loaded: [] as PoolWorkerStub[],
    terminated: [] as PoolWorkerStub[],
    allocateUnusedWorker() {
      const worker = { id: manager.allocated.length };
      manager.allocated.push(worker);
      manager.unusedWorkers.push(worker);
      return worker;
    },
    loadWasmModuleToWorker(worker: PoolWorkerStub) {
      manager.loaded.push(worker);
      return load(worker);
    },
    terminateWorker(worker: PoolWorkerStub) {
      manager.terminated.push(worker);
    },
  };
  return manager;
}

interface GeneratedWasiNodeLoaderOptions {
  createContext: () => {
    destroy(): void;
    feature?: Record<string, unknown>;
    suppressDestroy?: () => void;
  };
  prepareCleanup?: () => void;
  // Handed to the loader's plugins as `PThread`, the way emnapi does.
  threadManager?: object;
  runtimeConfig?: () => { flavor: string; workerThreads: number };
}

class WorkerStub {
  unref(): void {}
}

// The generated loader bootstraps the CurrentThread task and timer hosts at
// load, so the stub napi module must expose the seven host exports the
// bootstrap reads (contract version 4, live registrations).
function createHostIntegrationExports(): Record<string, unknown> {
  const active = new Set<string>();
  let nextLow = 1;
  return {
    getCurrentThreadTaskHostContractVersion: () => 4,
    isCurrentThreadHostRegistrationActive: (high: number, low: number) =>
      active.has(`${high}:${low}`),
    reserveCurrentThreadHostRegistration: () => ({ high: 0, low: nextLow++ }),
    registerCurrentThreadTaskHost: (high: number, low: number) => {
      active.add(`${high}:${low}`);
    },
    registerTimerHost: (high: number, low: number) => {
      active.add(`${high}:${low}`);
    },
    unregisterCurrentThreadTaskHost: (high: number, low: number) => {
      active.delete(`${high}:${low}`);
    },
    unregisterTimerHost: (high: number, low: number) => {
      active.delete(`${high}:${low}`);
    },
  };
}

function executeGeneratedWasiNodeLoader({
  createContext,
  prepareCleanup = () => {},
  threadManager,
  runtimeConfig,
}: GeneratedWasiNodeLoaderOptions): { cleanup(): void; exports: Record<string, unknown> } {
  const module: { exports: Record<string, unknown> } = { exports: {} };
  const listeners = {
    beforeExit: [] as Array<() => void>,
    exit: [] as Array<() => void>,
    newListener: [] as Array<(event: string, listener: () => void) => void>,
  };
  const require = Object.assign(
    (specifier: string) => {
      switch (specifier) {
        case 'node:fs':
          return {
            existsSync: (path: string) => path.endsWith('.wasm'),
            readFileSync: () => new Uint8Array(),
          };
        case 'node:path':
          return {
            join: (...parts: string[]) => parts.join('/'),
            parse: () => ({ root: '/' }),
          };
        case 'node:wasi':
          return { WASI: class {} };
        case 'node:worker_threads':
          return { Worker: WorkerStub };
        case '@napi-rs/wasm-runtime':
          return {
            createOnMessage: () => () => {},
            instantiateNapiModuleSync(
              _wasm: Uint8Array,
              options: {
                beforeInit(input: { instance: { exports: Record<string, () => void> } }): void;
                plugins?: unknown[];
              },
            ) {
              if (threadManager) {
                for (const plugin of options.plugins ?? []) {
                  if (typeof plugin === 'function') plugin({ PThread: threadManager });
                }
              }
              const instance = {
                exports: {
                  napi_prepare_wasm_env_cleanup: prepareCleanup,
                },
              };
              options.beforeInit({ instance });
              return {
                instance,
                module: {},
                napiModule: {
                  exports: {
                    ...createHostIntegrationExports(),
                    ...(runtimeConfig ? { getAsyncRuntimeConfig: runtimeConfig } : {}),
                  },
                },
              };
            },
          };
        case '@napi-rs/async-runtime':
          // The real host protocol package: the generated loader installs the
          // CurrentThread hosts against the stub exports above.
          return createRequire(
            fileURLToPath(new URL('../src/rolldown-binding.wasi.cjs', import.meta.url)),
          )('@napi-rs/async-runtime');
        case '@emnapi/runtime':
          return {
            createContext() {
              const context = createContext();
              context.feature ??= {};
              context.suppressDestroy ??= () => {};
              return context;
            },
          };
        default:
          throw new Error(`Unexpected require: ${specifier}`);
      }
    },
    { resolve: (specifier: string) => specifier },
  );

  // oxlint-disable-next-line typescript/no-implied-eval -- execute the generated loader with isolated runtime stubs
  new Function('require', 'module', 'process', '__dirname', 'WebAssembly', generatedWasiNodeLoader)(
    require,
    module,
    {
      cwd: () => '/',
      env: {},
      getMaxListeners: () => 10,
      prependListener(event: keyof typeof listeners, listener: never) {
        listeners[event].unshift(listener);
      },
      once(event: 'beforeExit' | 'exit', listener: () => void) {
        for (const notify of listeners.newListener) {
          notify(event, listener);
        }
        listeners[event].push(listener);
      },
      rawListeners(event: keyof typeof listeners) {
        return [...listeners[event]];
      },
      removeListener(event: keyof typeof listeners, listener: never) {
        const index = listeners[event].lastIndexOf(listener);
        if (index >= 0) listeners[event].splice(index, 1);
      },
      setMaxListeners() {},
    },
    '/fixture',
    { Memory: class {} },
  );
  return {
    exports: module.exports,
    cleanup() {
      const listener = listeners.exit.at(-1) ?? listeners.beforeExit.at(-1);
      if (!listener) {
        throw new Error('Generated WASI loader did not retain a context cleanup listener');
      }
      listener();
    },
  };
}
