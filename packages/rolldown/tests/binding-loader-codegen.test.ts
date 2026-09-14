import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { describe, expect, test } from 'vitest';

import {
  EMNAPI_ASYNC_WORK_POOL_SIZE_MAX,
  LOADED_BINDING_TARGET_EXPORT,
  patchWasiBindingLoader,
  patchWasiNodeAsyncWorkPoolSize,
} from '../binding-loader-codegen';

const cjsAnchor = 'module.exports = __napiModule.exports\n';
const esmAnchor = 'export default __napiModule.exports\n';
const wasiNodeLoaderTemplate = `const __nodePath = { parse: () => ({ root: '/' }) }
const __cwd = process.cwd()
const __rootDir = __nodePath.parse(__cwd).root
const __hostRoot =
  process.platform === 'android' ? __cwd : __rootDir
const __emnapiOptions = {
    asyncWorkPoolSize: (function () {
      const threadsSizeFromEnv = Number(process.env.NAPI_RS_ASYNC_WORK_POOL_SIZE ?? process.env.UV_THREADPOOL_SIZE)
      // NaN > 0 is false
      if (threadsSizeFromEnv > 0) {
        return threadsSizeFromEnv
      } else {
        return 4
      }
    })(),
}
`;
const generatedWasiNodeLoader = readFileSync(
  fileURLToPath(new URL('../src/rolldown-binding.wasi.cjs', import.meta.url)),
  'utf8',
);
describe('WASI binding target metadata', () => {
  test.each([
    ['CommonJS', cjsAnchor, `module.exports.${LOADED_BINDING_TARGET_EXPORT}`],
    ['ESM', esmAnchor, `export const ${LOADED_BINDING_TARGET_EXPORT}`],
  ])('replaces %s metadata across repeated and reversed builds', (_name, anchor, exportName) => {
    const threaded = patchWasiBindingLoader(anchor, 'wasi-threads');
    expect(threaded).toContain(`${exportName} = 'wasi-threads'`);

    const threadless = patchWasiBindingLoader(threaded, 'wasi');
    expect(threadless).toContain(`${exportName} = 'wasi'`);
    expect(threadless).not.toContain(`${exportName} = 'wasi-threads'`);

    const reversed = patchWasiBindingLoader(threadless, 'wasi-threads');
    expect(reversed).toContain(`${exportName} = 'wasi-threads'`);
    expect(reversed).not.toContain(`${exportName} = 'wasi'`);
    expect(patchWasiBindingLoader(reversed, 'wasi-threads')).toBe(reversed);
  });
});

describe('WASI async work pool normalization', () => {
  test('the generated Node loader gives emnapi the capped value', () => {
    const patched = patchWasiNodeAsyncWorkPoolSize(wasiNodeLoaderTemplate);
    const process = {
      cwd: () => '/',
      env: {
        NAPI_RS_ASYNC_WORK_POOL_SIZE: '2048',
        UV_THREADPOOL_SIZE: '2',
        UNRELATED: 'preserved',
      },
    };
    // oxlint-disable-next-line typescript/no-implied-eval -- evaluate the generated loader snippet in an isolated scope
    const result = Function(
      'process',
      `${patched}
return { pool: __emnapiOptions.asyncWorkPoolSize }`,
    )(process);

    expect(result).toEqual({ pool: EMNAPI_ASYNC_WORK_POOL_SIZE_MAX });
    expect(process.env.NAPI_RS_ASYNC_WORK_POOL_SIZE).toBe('2048');
    expect(patchWasiNodeAsyncWorkPoolSize(patched)).toBe(patched);
  });

  test('the generated Node loader falls back to the UV pool size', () => {
    const patched = patchWasiNodeAsyncWorkPoolSize(wasiNodeLoaderTemplate);
    // oxlint-disable-next-line typescript/no-implied-eval -- evaluate the generated loader snippet in an isolated scope
    const result = Function(
      'process',
      `${patched}
return { pool: __emnapiOptions.asyncWorkPoolSize }`,
    )({
      cwd: () => '/',
      env: { UV_THREADPOOL_SIZE: '6' },
    });

    expect(result.pool).toBe(6);
  });
});

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
    expect(generatedWasiNodeLoader).toContain('let __emnapiWasmEnvCleanupPrepared = false');
    expect(generatedWasiNodeLoader).toContain('function __destroyEmnapiContext()');
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
