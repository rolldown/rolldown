import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, test } from 'vitest';

import {
  assertAsyncRuntimeHostExports,
  ASYNC_RUNTIME_HOST_EXPORTS,
  EMNAPI_ASYNC_WORK_POOL_SIZE_DEFAULT,
  EMNAPI_ASYNC_WORK_POOL_SIZE_MAX,
  LOADED_BINDING_TARGET_EXPORT,
  normalizeEmnapiAsyncWorkPoolSize,
  patchWasiBrowserContextDestroyAwait,
  patchWasiBrowserWorkerTerminationAwait,
  patchWasiBindingContextLifecycle,
  patchWasiBindingLoader,
  patchWasiNodeAsyncWorkPoolSize,
  patchWasiNodeWorkerExecArgv,
  resolveWasiBindingTarget,
} from '../binding-loader-codegen';

const cjsAnchor = 'module.exports = __napiModule.exports\n';
const esmAnchor = 'export default __napiModule.exports\n';
const wasiNodeLoaderTemplate = `const __nodePath = { parse: () => ({ root: '/' }) }
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
const __workerOptions = {
  env: process.env,
}
const __fallbackWorkerOptions = {
  env: process.env,
}
`;
const generatedWasiNodeLoader = readFileSync(
  fileURLToPath(new URL('../src/rolldown-binding.wasi.cjs', import.meta.url)),
  'utf8',
);
const upstreamWasiBrowserLifecycle = `import {
  instantiateNapiModule as __emnapiInstantiateNapiModule,
} from '@napi-rs/wasm-runtime'
import { createContext as __emnapiCreateContext } from '@emnapi/runtime'

const __emnapiContext = __emnapiCreateContext()

let __napiInstance
let __wasiModule
let __napiModule

function __destroyEmnapiContext() {
  const __prepareWasmEnvCleanup =
    __napiInstance?.exports?.napi_prepare_wasm_env_cleanup
  if (typeof __prepareWasmEnvCleanup === 'function') {
    __prepareWasmEnvCleanup()
  }
  return __emnapiContext.destroy()
}

try {
  ;({
    instance: __napiInstance,
    module: __wasiModule,
    napiModule: __napiModule,
  } = await __emnapiInstantiateNapiModule(__wasmFile, {
    beforeInit({ instance }) {
      __napiInstance = instance
    },
  }))
} catch (__error) {
  const __cleanupErrors = []
  try {
    await __destroyEmnapiContext()
  } catch (__cleanupError) {
    __cleanupErrors.push(__cleanupError)
  }
  if (__cleanupErrors.length > 0) {
    throw __createInitializationCleanupError(__error, __cleanupErrors)
  }
  throw __error
}
export default __napiModule.exports
`;
const upstreamWasiNodeWorkerHelpers = `function __getWorkerExecArgv() {
  const __workerExecArgv = []
  for (let __index = 0; __index < process.execArgv.length; __index += 1) {
    const __arg = process.execArgv[__index]
    if (
      __arg === '--input-type' ||
      __arg === '--eval' ||
      __arg === '-e' ||
      __arg === '--print' ||
      __arg === '-p'
    ) {
      __index += 1
      continue
    }
    if (
      __arg.startsWith('--input-type=') ||
      __arg.startsWith('--eval=') ||
      __arg.startsWith('--print=')
    ) {
      continue
    }
    __workerExecArgv.push(__arg)
  }
  return __workerExecArgv
}

function __createWasiWorker(filename) {
  try {
    return new Worker(filename, {
      env: process.env,
      execArgv: __getWorkerExecArgv(),
    })
  } catch (error) {
    if (!error || error.code !== 'ERR_WORKER_INVALID_EXEC_ARGV') {
      throw error
    }
  }
  return new Worker(filename, {
    env: process.env,
    execArgv: [],
  })
}

const __rootDir = __nodePath.parse(process.cwd()).root
`;

describe('WASI binding target metadata', () => {
  test('resolves supported build targets without accepting unknown wasm targets', () => {
    expect(resolveWasiBindingTarget(undefined)).toBe('wasi-threads');
    expect(resolveWasiBindingTarget('aarch64-apple-darwin')).toBe('wasi-threads');
    expect(resolveWasiBindingTarget('wasm32-wasip1-threads')).toBe('wasi-threads');
    expect(resolveWasiBindingTarget('wasm32-wasip1')).toBe('wasi');
    expect(() => resolveWasiBindingTarget('wasm32-wasip2')).toThrow(
      'Unsupported WASI binding target',
    );
    expect(() => resolveWasiBindingTarget(null)).toThrow('Unsupported WASI binding target');
  });

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
  test.each([
    [undefined, EMNAPI_ASYNC_WORK_POOL_SIZE_DEFAULT],
    ['', EMNAPI_ASYNC_WORK_POOL_SIZE_DEFAULT],
    ['0', EMNAPI_ASYNC_WORK_POOL_SIZE_DEFAULT],
    ['0.5', EMNAPI_ASYNC_WORK_POOL_SIZE_DEFAULT],
    ['invalid', EMNAPI_ASYNC_WORK_POOL_SIZE_DEFAULT],
    ['Infinity', EMNAPI_ASYNC_WORK_POOL_SIZE_DEFAULT],
    ['1.9', 1],
    ['1e2', 100],
    ['0x10', 16],
    ['2048', EMNAPI_ASYNC_WORK_POOL_SIZE_MAX],
  ])('normalizes %j to %d', (value, expected) => {
    expect(normalizeEmnapiAsyncWorkPoolSize(value)).toBe(expected);
  });

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
	  workerEnv: __workerOptions.env,
	  fallbackWorkerEnv: __fallbackWorkerOptions.env,
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
      fallbackWorkerEnv: {
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
    [
      'CommonJS',
      `const {
  createContext: __emnapiCreateContext,
} = require('@napi-rs/wasm-runtime')
const __emnapiContext = __emnapiCreateContext()
const options = {
  beforeInit({ instance }) {
  },
}
module.exports = __napiModule.exports
`,
      "const { createContext: __emnapiCreateContext } = require('@emnapi/runtime')",
    ],
    [
      'ESM',
      `import {
  getDefaultContext as __emnapiGetDefaultContext,
} from '@napi-rs/wasm-runtime'
const __emnapiContext = __emnapiGetDefaultContext()
const options = {
  beforeInit({ instance }) {
  },
}
export default __napiModule.exports
`,
      "import { createContext as __emnapiCreateContext } from '@emnapi/runtime'",
    ],
  ])('normalizes the %s context import and lifecycle hooks', (_format, source, expectedImport) => {
    const patched = patchWasiBindingContextLifecycle(source);
    expect(patched).toContain(expectedImport);
    expect(patched).toContain('const __emnapiContext = __emnapiCreateContext()');
    expect(patched).toContain('function __wrapEmnapiContextDestroy(instance)');
    expect(patched).toContain('    __wrapEmnapiContextDestroy(instance)');
    expect(patchWasiBindingContextLifecycle(patched)).toBe(patched);
  });

  test('accepts and hardens the refreshed napi-rs browser lifecycle', () => {
    const patched = patchWasiBindingContextLifecycle(upstreamWasiBrowserLifecycle);

    expect(patched).toContain('let __emnapiWasmEnvCleanupPrepared = false');
    expect(patched).toContain('if (!__emnapiWasmEnvCleanupPrepared)');
    expect(patched).toContain('__napiInstance?.exports?.napi_prepare_wasm_env_cleanup');
    expect(patched).toContain('__emnapiWasmEnvCleanupPrepared = true');
    // Raw context.destroy() calls must settle pending napi work like
    // __destroyEmnapiContext() does, so the patch wraps destroy itself.
    expect(patched).toContain('const __emnapiContextDestroy = __emnapiContext.destroy');
    expect(patched).toContain('__emnapiContext.destroy = function() {');
    expect(patched).toContain('Reflect.apply(__emnapiContextDestroy, this, arguments)');
    expect(patchWasiBindingContextLifecycle(patched)).toBe(patched);
  });

  test('wraps context destroy on an already-guarded refreshed lifecycle', () => {
    const patched = patchWasiBindingContextLifecycle(upstreamWasiBrowserLifecycle);
    const withoutWrap = patched.replace(
      /if \(__emnapiContext !== undefined\) \{[\s\S]*?\n\}\n\n/,
      '',
    );

    expect(withoutWrap).not.toBe(patched);
    expect(patchWasiBindingContextLifecycle(withoutWrap)).toBe(patched);
  });

  test('rejects a partial refreshed lifecycle instead of falling back to legacy patching', () => {
    expect(() =>
      patchWasiBindingContextLifecycle(
        upstreamWasiBrowserLifecycle.replace('      __napiInstance = instance\n', ''),
      ),
    ).toThrow('WASI N-API instance capture');
  });

  test.each(['__emnapiContext.destroy()', '__destroyEmnapiContext()'])(
    'normalizes the generated browser cleanup await for %s',
    (expression) => {
      const source = `try {}\ncatch (__error) {\n  await ${expression}\n}\n`;
      const patched = patchWasiBrowserContextDestroyAwait(source);
      expect(patched).toContain(`await Promise.resolve(${expression})`);
      expect(patchWasiBrowserContextDestroyAwait(patched)).toBe(patched);
    },
  );

  test('normalizes synchronous threaded browser worker termination failures', () => {
    const source = `    try {
      __terminations.push(Promise.resolve(__worker.terminate()))
    } catch (__cleanupError) {
      __terminations.push({ error: __cleanupError })
    }
`;
    const patched = patchWasiBrowserWorkerTerminationAwait(source);

    expect(patched).toContain(
      '      __terminations.push(Promise.resolve({ error: __cleanupError }))',
    );
    expect(patchWasiBrowserWorkerTerminationAwait(patched)).toBe(patched);
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

  test('retries failed preparation without repeating successful preparation', () => {
    const cleanupEvents: string[] = [];
    let prepareAttempts = 0;
    let destroyAttempts = 0;
    const context = {
      destroy() {
        cleanupEvents.push('destroy');
        destroyAttempts += 1;
        if (destroyAttempts === 1) {
          throw new Error('destroy failed');
        }
      },
    };

    const execution = executeGeneratedWasiNodeLoader({
      createContext: () => context,
      prepareCleanup() {
        cleanupEvents.push('prepare');
        prepareAttempts += 1;
        if (prepareAttempts === 1) {
          throw new Error('prepare failed');
        }
      },
    });

    expect(() => execution.cleanup()).not.toThrow();
    expect(cleanupEvents).toEqual(['prepare']);
    expect(() => execution.cleanup()).not.toThrow();
    expect(cleanupEvents).toEqual(['prepare', 'prepare', 'destroy']);
    expect(() => execution.cleanup()).not.toThrow();
    expect(() => execution.cleanup()).not.toThrow();
    expect(cleanupEvents).toEqual(['prepare', 'prepare', 'destroy', 'destroy']);
  });

  test('preserves valid worker arguments while retrying rejected inherited arguments', () => {
    const workerExecArgvAttempts: string[][] = [];

    class Worker {
      onmessage?: (event: { data: unknown }) => void;

      constructor(_filename: string, options: { execArgv?: string[] }) {
        const execArgv = options.execArgv ?? [];
        workerExecArgvAttempts.push(execArgv);
        if (execArgv.includes('--title') || execArgv.includes('--stack-trace-limit=100')) {
          throw Object.assign(
            new Error(
              'Initiated Worker with invalid execArgv flags: --title, --stack-trace-limit=100',
            ),
            { code: 'ERR_WORKER_INVALID_EXEC_ARGV' },
          );
        }
      }

      unref(): void {}
    }

    executeGeneratedWasiNodeLoader({
      Worker,
      createContext: () => ({ destroy() {} }),
      createWorker: true,
      execArgv: [
        '--trace-warnings',
        '--input-type=module',
        '--eval',
        'evaluate()',
        '-p',
        'print()',
        '--title',
        'test-worker',
        '--require',
        './hook.cjs',
        '--stack-trace-limit=100',
        '--conditions=worker-test',
      ],
    });

    expect(workerExecArgvAttempts).toEqual([
      [
        '--trace-warnings',
        '--title',
        'test-worker',
        '--require',
        './hook.cjs',
        '--stack-trace-limit=100',
        '--conditions=worker-test',
      ],
      ['--trace-warnings', '--require', './hook.cjs', '--conditions=worker-test'],
    ]);
    expect(patchWasiNodeWorkerExecArgv(generatedWasiNodeLoader)).toBe(generatedWasiNodeLoader);
  });

  test('replaces the refreshed napi-rs all-or-nothing worker fallback', () => {
    const patched = patchWasiNodeWorkerExecArgv(upstreamWasiNodeWorkerHelpers);

    expect(patched).toContain('function __removeInvalidWasiWorkerExecArgv(execArgv, error)');
    expect(patched).toContain('let __workerExecArgv = __getWasiWorkerExecArgv()');
    expect(patched).not.toContain('execArgv: []');
    expect(patchWasiNodeWorkerExecArgv(patched)).toBe(patched);
  });
});

describe('async-runtime host export contract', () => {
  test.each([
    [
      'CommonJS',
      'commonjs' as const,
      ASYNC_RUNTIME_HOST_EXPORTS.map(
        (name) => `module.exports.${name} = nativeBinding.${name}\n`,
      ).join(''),
    ],
    [
      'ESM',
      'esm' as const,
      ASYNC_RUNTIME_HOST_EXPORTS.map(
        (name) => `export const ${name} = __napiModule.exports.${name}\n`,
      ).join(''),
    ],
  ])('accepts a complete generated %s loader', (_name, format, source) => {
    expect(() => assertAsyncRuntimeHostExports(source, format)).not.toThrow();
  });

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
