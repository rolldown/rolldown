import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { describe, expect, test } from 'vitest';

import { assertWasiThreadCrashLatch, assertWasiThreadPoolPreload } from '../binding-loader-codegen';

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

// The cli emits the preload; rolldown only checks the seam.
// See internal-docs/async-runtime/implementation.md (section 13, "Pool worker preload").
describe('generated WASI loader pool worker preload seam', () => {
  test('the committed threaded loader carries it', () => {
    expect(() => assertWasiThreadPoolPreload(generatedWasiNodeLoader)).not.toThrow();
  });

  test('a loader without one of its pieces fails the build', () => {
    for (const [search, replacement] of [
      // The count source.
      ['exports?.napi_wasm_runtime_pool_workers', 'exports?.napi_wasm_runtime_pool_size'],
      // The until-exit tracking of a Worker the reconcile terminates.
      ['function __untrackWasiWorkerOnExit(worker) {', 'function __untrackWasiWorker(worker) {'],
      // The configure wrap, and the export tail that reads it back.
      ['  __wrapWasiConfigureAsyncRuntime(__napiModule.exports)\n', ''],
      ['module.exports.configureAsyncRuntime = __napiModule.exports.configureAsyncRuntime\n', ''],
      // The preload call itself.
      ['try {\n  __reconcileWasiThreadPool()\n} catch {}\n', ''],
    ]) {
      expect(generatedWasiNodeLoader).toContain(search);
      expect(() =>
        assertWasiThreadPoolPreload(generatedWasiNodeLoader.replace(search, replacement)),
      ).toThrow(/WASI thread pool preload/);
    }
  });

  test('a preload call outside the load-to-export-tail window fails the build', () => {
    const call = 'try {\n  __reconcileWasiThreadPool()\n} catch {}\n';
    const tail = 'module.exports = __napiModule.exports\n';
    const moved = generatedWasiNodeLoader.replace(call, '').replace(tail, `${tail}${call}`);
    expect(() => assertWasiThreadPoolPreload(moved)).toThrow(
      /preload call between the load try\/catch and the CommonJS export tail/,
    );
  });
});

// After a crash disposal no emnapi deferred call may enter wasm (napi-rs
// e6e50eb4); the cli emits the gate, rolldown only checks the seam.
// See internal-docs/async-runtime/implementation.md (section 7, the crash latch).
describe('generated WASI loader crash-disposal re-entry gate', () => {
  const wasiWorker = readFileSync(
    fileURLToPath(new URL('../src/wasi-worker.mjs', import.meta.url)),
    'utf8',
  );

  test('the committed threaded loader carries it', () => {
    expect(() => assertWasiThreadCrashLatch(generatedWasiNodeLoader, wasiWorker)).not.toThrow();
  });

  test('a loader without one of its pieces fails the build', () => {
    for (const [search, replacement] of [
      // The deferred-call hook, its use by the context, and the gate's close.
      ['function __wasiSetImmediate(callback) {', 'function __wasiDeferCall(callback) {'],
      ['features: { setImmediate: __wasiSetImmediate }', 'features: {}'],
      ['  __wasiReentryClosed = true\n', ''],
    ]) {
      expect(generatedWasiNodeLoader).toContain(search);
      expect(() =>
        assertWasiThreadCrashLatch(
          generatedWasiNodeLoader.replace(search, replacement),
          wasiWorker,
        ),
      ).toThrow(/WASI thread crash latch \(loader\)/);
    }
  });
});

interface GeneratedWasiNodeLoaderOptions {
  createContext: () => {
    destroy(): void;
    feature?: Record<string, unknown>;
    suppressDestroy?: () => void;
  };
  prepareCleanup?: () => void;
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
}: GeneratedWasiNodeLoaderOptions): { cleanup(): void } {
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
              },
            ) {
              const instance = {
                exports: {
                  napi_prepare_wasm_env_cleanup: prepareCleanup,
                },
              };
              options.beforeInit({ instance });
              return {
                instance,
                module: {},
                napiModule: { exports: createHostIntegrationExports() },
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
    cleanup() {
      const listener = listeners.exit.at(-1) ?? listeners.beforeExit.at(-1);
      if (!listener) {
        throw new Error('Generated WASI loader did not retain a context cleanup listener');
      }
      listener();
    },
  };
}
