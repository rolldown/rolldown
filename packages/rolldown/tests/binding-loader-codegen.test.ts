import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { describe, expect, test } from 'vitest';

import {
  assertAsyncRuntimeHostExports,
  EMNAPI_ASYNC_WORK_POOL_SIZE_MAX,
  LOADED_BINDING_TARGET_EXPORT,
  patchWasiBrowserContextDestroyAwait,
  patchWasiBrowserWorkerTerminationAwait,
  patchWasiBindingContextLifecycle,
  patchWasiBindingLoader,
  patchWasiNodeAsyncWorkPoolSize,
  patchWasiNodeWorkerExecArgv,
} from '../binding-loader-codegen';

const cjsAnchor = 'module.exports = __napiModule.exports\n';
const esmAnchor = 'export default __napiModule.exports\n';
const wasiNodeLoaderTemplate = `const __nodePath = { parse: () => ({ root: '/' }) }
function __createWasiWorker(filename) {
  return {
    env: process.env,
  }
}
const __rootDir = __nodePath.parse(process.cwd()).root
const __wasiOptions = {
  env: process.env,
}
const __emnapiOptions = {
    asyncWorkPoolSize: (function() {
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
const generatedWasiBrowserLoader = readFileSync(
  fileURLToPath(new URL('../src/rolldown-binding.wasi-browser.js', import.meta.url)),
  'utf8',
);
// Reversing the lifecycle patch's raw-destroy settlement wrapper reconstructs
// the pristine template output the patcher receives from a fresh napi build.
const contextDestroyWrapPattern =
  /function __wrapEmnapiContextDestroyForSettlement\(context\) \{[\s\S]*?\n\}\n\n/;
const wrappedContextCreation =
  '__emnapiContext = __wrapEmnapiContextDestroyForSettlement(__emnapiCreateContext({ autoDestroy: false }))';
const plainContextCreation = '__emnapiContext = __emnapiCreateContext({ autoDestroy: false })';
function unwrapGeneratedLifecycle(source: string): string {
  const unwrapped = source
    .replace(contextDestroyWrapPattern, '')
    .replace(wrappedContextCreation, plainContextCreation);
  expect(unwrapped).not.toBe(source);
  return unwrapped;
}

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

  test('rejects duplicate target exports instead of preserving the stale winner', () => {
    const duplicate = `${cjsAnchor}module.exports.${LOADED_BINDING_TARGET_EXPORT} = 'wasi'\nmodule.exports.${LOADED_BINDING_TARGET_EXPORT} = 'wasi-threads'\n`;
    expect(() => patchWasiBindingLoader(duplicate, 'wasi')).toThrow(
      'expected at most one binding target export',
    );
  });

  test.each([
    [
      'CommonJS',
      `${cjsAnchor}module.exports.${LOADED_BINDING_TARGET_EXPORT} = "unknown";\n`,
      `module.exports.${LOADED_BINDING_TARGET_EXPORT}`,
    ],
    [
      'ESM',
      `${esmAnchor}export const ${LOADED_BINDING_TARGET_EXPORT} = "unknown";\n`,
      `export const ${LOADED_BINDING_TARGET_EXPORT}`,
    ],
  ])(
    'replaces an unexpected existing %s target without adding a duplicate',
    (_name, source, exportName) => {
      const patched = patchWasiBindingLoader(source, 'wasi');
      expect(patched).toContain(`${exportName} = 'wasi'`);
      expect(patched.match(new RegExp(exportName.replaceAll('.', '\\.'), 'g'))).toHaveLength(1);
      expect(patched).not.toContain('unknown');
    },
  );
});

describe('WASI async work pool normalization', () => {
  test('the generated Node loader gives emnapi and the WASI guest the same capped value', () => {
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
return {
	  pool: __emnapiOptions.asyncWorkPoolSize,
	  wasiEnv: __wasiOptions.env,
	  workerEnv: __createWasiWorker('wasi-worker.mjs').env,
	}`,
    )(process);

    expect(result).toEqual({
      pool: EMNAPI_ASYNC_WORK_POOL_SIZE_MAX,
      wasiEnv: {
        NAPI_RS_ASYNC_WORK_POOL_SIZE: String(EMNAPI_ASYNC_WORK_POOL_SIZE_MAX),
        UV_THREADPOOL_SIZE: '2',
        UNRELATED: 'preserved',
      },
      workerEnv: {
        NAPI_RS_ASYNC_WORK_POOL_SIZE: String(EMNAPI_ASYNC_WORK_POOL_SIZE_MAX),
        UV_THREADPOOL_SIZE: '2',
        UNRELATED: 'preserved',
      },
    });
    expect(process.env.NAPI_RS_ASYNC_WORK_POOL_SIZE).toBe('2048');
    expect(patchWasiNodeAsyncWorkPoolSize(patched)).toBe(patched);
  });

  test('the generated Node loader normalizes the UV fallback into the authoritative NAPI key', () => {
    const patched = patchWasiNodeAsyncWorkPoolSize(wasiNodeLoaderTemplate);
    // oxlint-disable-next-line typescript/no-implied-eval -- evaluate the generated loader snippet in an isolated scope
    const result = Function(
      'process',
      `${patched}
return {
  pool: __emnapiOptions.asyncWorkPoolSize,
  wasiEnv: __wasiOptions.env,
}`,
    )({
      cwd: () => '/',
      env: { UV_THREADPOOL_SIZE: '6' },
    });

    expect(result.pool).toBe(6);
    expect(result.wasiEnv).toEqual({
      NAPI_RS_ASYNC_WORK_POOL_SIZE: '6',
      UV_THREADPOOL_SIZE: '6',
    });
  });
});

describe('generated WASI loader lifecycle', () => {
  test.each([
    ['CommonJS', () => generatedWasiNodeLoader],
    ['ESM browser', () => generatedWasiBrowserLoader],
  ])('injects the raw-destroy settlement wrapper into the %s loader', (_format, read) => {
    const patched = read();
    const unwrapped = unwrapGeneratedLifecycle(patched);

    // Patching the pristine template output reproduces the committed loader
    // byte for byte, and re-patching is the identity.
    expect(patchWasiBindingContextLifecycle(unwrapped)).toBe(patched);
    expect(patchWasiBindingContextLifecycle(patched)).toBe(patched);

    expect(patched).toContain('function __wrapEmnapiContextDestroyForSettlement(context) {');
    expect(patched).toContain(wrappedContextCreation);
    expect(patched).toContain('const __contextDestroy = context.destroy');
    expect(patched).toContain('Reflect.apply(__contextDestroy, this, arguments)');
  });

  test.each([
    ['the settlement drain', 'function __drainWasmEnvCleanup() {', 'WASI disposal chain helper'],
    [
      'the initialization rollback',
      'function __rollbackWasiInitialization() {',
      'WASI disposal chain helper',
    ],
    [
      'the dispose publication',
      '__publishWasiDispose(__napiModule.exports)',
      'WASI dispose symbol publication',
    ],
    [
      'the settlement barrier',
      '  __prepareWasmEnvCleanup()\n  const result = __emnapiContext.destroy()\n',
      'WASI context destroy settlement barrier',
    ],
  ])('rejects a generated loader missing %s', (_label, marker, diagnostic) => {
    const unwrapped = unwrapGeneratedLifecycle(generatedWasiNodeLoader);
    const mutated = unwrapped.replace(marker, marker.replace('__', '__x_'));
    expect(mutated).not.toBe(unwrapped);
    expect(() => patchWasiBindingContextLifecycle(mutated)).toThrow(diagnostic);
  });

  test('verifies the thenable-aware browser context destroy instead of rewriting it', () => {
    expect(patchWasiBrowserContextDestroyAwait(generatedWasiBrowserLoader)).toBe(
      generatedWasiBrowserLoader,
    );
    expect(() =>
      patchWasiBrowserContextDestroyAwait(
        generatedWasiBrowserLoader.replace(
          '  const destroyResult = __destroyEmnapiContext()',
          '  const destroyResult = await __emnapiContext.destroy()',
        ),
      ),
    ).toThrow('WASI browser thenable-aware context destroy');
  });

  test('verifies the thenable-aware browser worker termination instead of rewriting it', () => {
    expect(patchWasiBrowserWorkerTerminationAwait(generatedWasiBrowserLoader)).toBe(
      generatedWasiBrowserLoader,
    );
    expect(() =>
      patchWasiBrowserWorkerTerminationAwait(
        generatedWasiBrowserLoader.replace(
          'function __terminateWasiWorkers() {',
          'function __x() {',
        ),
      ),
    ).toThrow('WASI browser worker termination');
  });

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
    expect(patchWasiBindingContextLifecycle(generatedWasiNodeLoader)).toBe(generatedWasiNodeLoader);
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

  test('verifies the retrying worker factory instead of rewriting it', () => {
    // @napi-rs/cli ships the argument-preserving retry factory itself now; the
    // patcher pins the helper set and the construction call.
    expect(patchWasiNodeWorkerExecArgv(generatedWasiNodeLoader)).toBe(generatedWasiNodeLoader);
    expect(() =>
      patchWasiNodeWorkerExecArgv(
        generatedWasiNodeLoader.replace(
          'function __removeInvalidWasiWorkerExecArgv(execArgv, error) {',
          'function __removed(execArgv, error) {',
        ),
      ),
    ).toThrow('WASI worker execArgv helper');
    expect(() =>
      patchWasiNodeWorkerExecArgv(
        generatedWasiNodeLoader.replace(
          'function __getWasiWorkerExecArgv() {',
          `function __getWasiWorkerExecArgv() {}

function __getWasiWorkerExecArgv() {`,
        ),
      ),
    ).toThrow('WASI worker execArgv helper');
  });
});

describe('async-runtime host export contract', () => {
  test('reports every missing named export', () => {
    expect(() =>
      assertAsyncRuntimeHostExports(
        'module.exports.registerTimerHost = nativeBinding.registerTimerHost\n',
        'commonjs',
      ),
    ).toThrow(
      'getCurrentThreadTaskHostContractVersion, isCurrentThreadHostRegistrationActive, registerCurrentThreadTaskHost, reserveCurrentThreadHostRegistration, unregisterCurrentThreadTaskHost, unregisterTimerHost',
    );
  });
});

interface GeneratedWasiNodeLoaderOptions {
  Worker?: new (
    filename: string,
    options: { env: Record<string, string>; execArgv?: string[] },
  ) => {
    onmessage?: (event: { data: unknown }) => void;
    unref(): void;
  };
  createContext: () => {
    destroy(): void;
    feature?: Record<string, unknown>;
    suppressDestroy?: () => void;
  };
  createWorker?: boolean;
  execArgv?: string[];
  prepareCleanup?: () => void;
}

function executeGeneratedWasiNodeLoader({
  Worker = class {
    unref(): void {}
  },
  createContext,
  createWorker = false,
  execArgv = [],
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
          return { Worker };
        case '@napi-rs/wasm-runtime':
          return {
            createOnMessage: () => () => {},
            instantiateNapiModuleSync(
              _wasm: Uint8Array,
              options: {
                beforeInit(input: { instance: { exports: Record<string, () => void> } }): void;
                onCreateWorker(): object;
              },
            ) {
              if (createWorker) {
                options.onCreateWorker();
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
                napiModule: { exports: {} },
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
      execArgv,
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
