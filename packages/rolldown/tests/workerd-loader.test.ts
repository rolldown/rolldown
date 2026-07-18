import { spawnSync } from 'node:child_process';
import { Buffer as NodeBuffer } from 'node:buffer';
import { existsSync } from 'node:fs';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { runInNewContext } from 'node:vm';
import { createBuildCommand } from '@napi-rs/cli';
// @ts-ignore This focused build-codegen test intentionally reaches package tooling outside the test rootDir.
import { isAsyncRuntimeDeclarationBuild } from '../generate-workerd-loader';
// @ts-ignore This focused build-codegen test intentionally reaches package tooling outside the test rootDir.
import { injectCurrentThreadHostBootstrap } from '../generate-workerd-loader';
// @ts-ignore This focused build-codegen test intentionally reaches package tooling outside the test rootDir.
import { preserveGeneratedBindingSources } from '../generate-workerd-loader';
// @ts-ignore This focused build-codegen test intentionally reaches package tooling outside the test rootDir.
import { preserveInactiveWasiDeclaration } from '../generate-workerd-loader';
// @ts-ignore This focused build-codegen test intentionally reaches package tooling outside the test rootDir.
import { rewriteThreadlessMemoryDescriptor } from '../generate-workerd-loader';
// @ts-ignore This focused unit test intentionally reaches generated package source outside the test rootDir.
import type { DeferredRolldownInstance } from '../src/rolldown-binding.wasip1-deferred.js';
// @ts-ignore This focused integration test intentionally reaches the package source outside the test rootDir.
import * as workerd from '../src/workerd';
// @ts-ignore This focused unit test intentionally reaches the package source outside the test rootDir.
import { registerWorkerdCurrentThreadTaskHost } from '../src/workerd-task-host';
// @ts-ignore This focused unit test intentionally reaches the package source outside the test rootDir.
import { registerWorkerdTimerHost } from '../src/workerd-timer-host';
import { describe, expect, test, vi } from 'vitest';

const { createInstance, getWorkerdRuntimeStats, instantiate, WORKERD_WASM_MEMORY } = workerd;

const wasmPath = new URL('../src/rolldown-binding.wasm32-wasip1.wasm', import.meta.url);
const wasiTest = test.runIf(existsSync(wasmPath));
const deferredLoaderPath = new URL('../src/rolldown-binding.wasip1-deferred.js', import.meta.url);
const cjsLoaderPath = new URL('../src/rolldown-binding.wasip1.cjs', import.meta.url);
const browserLoaderPath = new URL('../src/rolldown-binding.wasip1-browser.js', import.meta.url);
const threadedBrowserLoaderPath = new URL(
  '../src/rolldown-binding.wasi-browser.js',
  import.meta.url,
);
const currentThreadBootstrapStart = '/* ROLLDOWN_CURRENT_THREAD_HOST_BOOTSTRAP_START */';
const currentThreadBootstrapEnd = '/* ROLLDOWN_CURRENT_THREAD_HOST_BOOTSTRAP_END */';
const browserInitializationGuardStart = '/* ROLLDOWN_BROWSER_INITIALIZATION_GUARD_START */';
const browserInitializationGuardEnd = '/* ROLLDOWN_BROWSER_INITIALIZATION_GUARD_END */';
const nodeInitializationCleanupStart = '/* ROLLDOWN_NODE_INITIALIZATION_CLEANUP_START */';
const nodeInitializationCleanupEnd = '/* ROLLDOWN_NODE_INITIALIZATION_CLEANUP_END */';
const privateManagedHostExports = [
  'getCurrentThreadTaskHostContractVersion',
  'isCurrentThreadHostRegistrationActive',
  'registerCurrentThreadTaskHost',
  'registerTimerHost',
  'reserveCurrentThreadHostRegistration',
  'unregisterCurrentThreadTaskHost',
  'unregisterTimerHost',
] as const;
const removedTaskHostExports = [
  'cancelCurrentThreadRuntimeTaskDispatch',
  'driveCurrentThreadRuntimeTasks',
] as const;

let nextMockHostRegistration = 1;

function installMockHostRegistrationControls(binding: Record<PropertyKey, unknown>): void {
  const reserve = Reflect.get(binding, 'reserveCurrentThreadHostRegistration');
  const isActive = Reflect.get(binding, 'isCurrentThreadHostRegistrationActive');
  if (typeof reserve === 'function' && typeof isActive === 'function') {
    return;
  }
  const reserved = new Set<number>();
  const live = new Set<number>();
  Reflect.set(binding, 'getCurrentThreadTaskHostContractVersion', () => 4);
  Reflect.set(binding, 'reserveCurrentThreadHostRegistration', () => {
    const low = nextMockHostRegistration++;
    reserved.add(low);
    return { high: 0, low };
  });
  Reflect.set(binding, 'isCurrentThreadHostRegistrationActive', (_high: number, low: number) =>
    live.has(low),
  );
  const install = (registerName: string, unregisterName: string) => {
    const register = Reflect.get(binding, registerName);
    if (typeof register !== 'function') return;
    const unregister = Reflect.get(binding, unregisterName);
    Reflect.set(binding, registerName, function (this: unknown, ...args: unknown[]) {
      const high = args[0];
      const low = args[1];
      if (high !== 0 || typeof low !== 'number' || !reserved.delete(low)) {
        throw new TypeError('Mock host registration was not reserved');
      }
      Reflect.apply(register, this, args.slice(2));
      live.add(low);
    });
    Reflect.set(binding, unregisterName, (_high: number, low: number) => {
      reserved.delete(low);
      live.delete(low);
      if (typeof unregister === 'function') {
        Reflect.apply(unregister, binding, [_high, low]);
      }
    });
  };
  install('registerCurrentThreadTaskHost', 'unregisterCurrentThreadTaskHost');
  install('registerTimerHost', 'unregisterTimerHost');
}

function createWorkerdTimerHostBinding(
  registerTimerHost: (schedule: unknown, cancel: unknown) => void,
  options: {
    registration?: { high: number; low: number };
    unregisterTimerHost?: (high: number, low: number) => void;
  } = {},
) {
  const registration = options.registration ?? { high: 0, low: nextMockHostRegistration++ };
  let active = false;
  return {
    getCurrentThreadTaskHostContractVersion: () => 4,
    isCurrentThreadHostRegistrationActive: (high: number, low: number) =>
      active && high === registration.high && low === registration.low,
    registerTimerHost(high: number, low: number, schedule: unknown, cancel: unknown) {
      if (high !== registration.high || low !== registration.low) {
        throw new TypeError('Unexpected timer host registration');
      }
      registerTimerHost(schedule, cancel);
      active = true;
    },
    reserveCurrentThreadHostRegistration: () => registration,
    unregisterTimerHost(high: number, low: number) {
      options.unregisterTimerHost?.(high, low);
      active = false;
    },
  };
}

async function readCurrentThreadHostBootstrap(loaderPath: URL): Promise<string> {
  const source = await readFile(loaderPath, 'utf8');
  const start = source.indexOf(currentThreadBootstrapStart);
  const end = source.indexOf(currentThreadBootstrapEnd, start);
  expect(start).toBeGreaterThanOrEqual(0);
  expect(end).toBeGreaterThan(start);
  expect(source.indexOf(currentThreadBootstrapStart, start + 1)).toBe(-1);
  expect(source.indexOf(currentThreadBootstrapEnd, end + 1)).toBe(-1);
  return source.slice(start + currentThreadBootstrapStart.length, end);
}

async function readGeneratedNodeLifecycle(): Promise<string> {
  const source = await readFile(cjsLoaderPath, 'utf8');
  const start = source.indexOf('function __destroyEmnapiContext()');
  const end = source.indexOf('if (__contextInitializationFailed)', start);
  expect(start).toBeGreaterThanOrEqual(0);
  expect(end).toBeGreaterThan(start);
  return source.slice(start, end);
}

function countOccurrences(source: string, search: string): number {
  return source.split(search).length - 1;
}

function runCurrentThreadHostBootstrap(
  source: string,
  binding: object,
  globals: {
    setTimeout?: unknown;
    clearTimeout?: unknown;
  } = {},
): void {
  const getGlobal = (name: keyof typeof globals, fallback: unknown) =>
    Object.prototype.hasOwnProperty.call(globals, name) ? globals[name] : fallback;
  runInNewContext(
    `let __browserTaskHostRegistration
let __browserTimerHostRegistration
let __nodeTaskHostRegistration
let __nodeTimerHostRegistration
${source}`,
    {
      __napiModule: { exports: binding },
      setTimeout: getGlobal('setTimeout', globalThis.setTimeout),
      clearTimeout: getGlobal('clearTimeout', globalThis.clearTimeout),
    },
  );
}

async function loadDeferredLoaderWithDependencies(dependencies: object) {
  const source = await readFile(deferredLoaderPath, 'utf8');
  const dependencyKey = `__rolldownWorkerdLoaderTest${Date.now()}${Math.random()}`;
  const testDependencies = { Buffer: NodeBuffer, ...dependencies } as Record<PropertyKey, unknown>;
  const createContext = Reflect.get(testDependencies, 'createContext');
  if (typeof createContext === 'function') {
    Reflect.set(testDependencies, 'createContext', (...args: unknown[]) => {
      const context = Reflect.apply(createContext, dependencies, args);
      if (
        context &&
        (typeof context === 'object' || typeof context === 'function') &&
        !Reflect.has(context, 'feature')
      ) {
        Reflect.set(context, 'feature', {});
      }
      return context;
    });
  }
  const instantiateNapiModule = Reflect.get(testDependencies, 'instantiateNapiModule');
  if (typeof instantiateNapiModule === 'function') {
    Reflect.set(testDependencies, 'instantiateNapiModule', async (...args: unknown[]) => {
      const result = await Reflect.apply(instantiateNapiModule, dependencies, args);
      const binding = result?.napiModule?.exports;
      if (binding && (typeof binding === 'object' || typeof binding === 'function')) {
        installMockHostRegistrationControls(binding);
      }
      return result;
    });
  }
  Object.defineProperty(globalThis, dependencyKey, {
    configurable: true,
    value: testDependencies,
  });
  const transformed = source
    .replace(
      /import \{[\s\S]*?\} from '@napi-rs\/wasm-runtime'\nimport \{ createContext as __emnapiCreateContext \} from '@emnapi\/runtime'\n/,
      `const {
  getDefaultContext: __emnapiGetDefaultContext,
  instantiateNapiModule: __emnapiInstantiateNapiModule,
  WASI: __WASI,
  createContext: __emnapiCreateContext,
  Buffer,
} = globalThis[${JSON.stringify(dependencyKey)}]\n`,
    )
    .replace("import { Buffer } from 'buffer'\n", '');
  try {
    return await import(
      `data:text/javascript;base64,${Buffer.from(transformed).toString('base64')}#${dependencyKey}`
    );
  } finally {
    Reflect.deleteProperty(globalThis, dependencyKey);
  }
}

async function loadBrowserLoaderWithDependencies(dependencies: object) {
  const source = await readFile(browserLoaderPath, 'utf8');
  const runtimeImports =
    /import \{[\s\S]*?\} from '@napi-rs\/wasm-runtime'\nimport \{ createContext as __emnapiCreateContext \} from '@emnapi\/runtime'\nimport \{ memfs, Buffer \} from '@napi-rs\/wasm-runtime\/fs'\n/;
  if (!runtimeImports.test(source)) {
    throw new Error('Unable to inject generated browser loader test dependencies');
  }
  const dependencyKey = `__rolldownBrowserLoaderTest${Date.now()}${Math.random()}`;
  const testDependencies = { Buffer: NodeBuffer, ...dependencies } as Record<PropertyKey, unknown>;
  const createContext = Reflect.get(testDependencies, 'createContext');
  if (typeof createContext === 'function') {
    Reflect.set(testDependencies, 'createContext', (...args: unknown[]) => {
      const context = Reflect.apply(createContext, dependencies, args);
      if (
        context &&
        (typeof context === 'object' || typeof context === 'function') &&
        !Reflect.has(context, 'feature')
      ) {
        Reflect.set(context, 'feature', {});
      }
      return context;
    });
  }
  const instantiateNapiModule = Reflect.get(testDependencies, 'instantiateNapiModule');
  if (typeof instantiateNapiModule === 'function') {
    Reflect.set(testDependencies, 'instantiateNapiModule', async (...args: unknown[]) => {
      const result = await Reflect.apply(instantiateNapiModule, dependencies, args);
      const binding = result?.napiModule?.exports;
      if (binding && (typeof binding === 'object' || typeof binding === 'function')) {
        installMockHostRegistrationControls(binding);
      }
      return result;
    });
  }
  Object.defineProperty(globalThis, dependencyKey, {
    configurable: true,
    value: testDependencies,
  });
  const transformed = source
    .replace(
      runtimeImports,
      `const {
  Buffer,
  createContext: __emnapiCreateContext,
  fetch: __browserFetch,
  instantiateNapiModule: __emnapiInstantiateNapiModule,
  memfs,
  WASI: __WASI,
} = globalThis[${JSON.stringify(dependencyKey)}]\n`,
    )
    .replace(
      /const __wasmUrl = new URL\([^\n]+\)\.href\nconst __wasmResponse = await globalThis\.fetch\(__wasmUrl\)/,
      `const __wasmUrl = 'https://example.invalid/rolldown-binding.wasm32-wasip1.wasm'
const __wasmResponse = await __browserFetch(__wasmUrl)`,
    );
  try {
    return await import(
      `data:text/javascript;base64,${Buffer.from(transformed).toString('base64')}#${dependencyKey}`
    );
  } finally {
    Reflect.deleteProperty(globalThis, dependencyKey);
  }
}

async function getDeferredInitializationFailure(primaryError: unknown): Promise<unknown> {
  const cleanupErrors = [new Error('cleanup failed once'), new Error('cleanup failed twice')];
  const context = {
    suppressDestroy() {},
    destroy() {
      throw cleanupErrors.shift();
    },
  };
  const loader = await loadDeferredLoaderWithDependencies({
    createContext: () => context,
    getDefaultContext: () => context,
    instantiateNapiModule: () => Promise.reject(primaryError),
    WASI: class {},
  });
  const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));

  return await loader
    .createInstance(module, {
      initialMemoryPages: 1,
      maximumMemoryPages: 1,
    })
    .then(
      () => {
        throw new Error('Expected deferred workerd initialization to fail');
      },
      (error: unknown) => error,
    );
}

function expectCleanupFailure(
  failure: unknown,
  primaryError: unknown,
  cleanupMessage: string,
): void {
  expect(Object.is(failure, primaryError)).toBe(false);
  expect(failure).toBeInstanceOf(AggregateError);
  const aggregate = failure as AggregateError & { cause?: unknown };
  expect(aggregate.cause).toBe(primaryError);
  expect(aggregate.errors).toHaveLength(2);
  expect(aggregate.errors[0]).toBe(primaryError);
  expect(aggregate.errors[1]).toMatchObject({
    message: cleanupMessage,
    errors: [expect.any(Error), expect.any(Error)],
  });
}

describe.sequential('managed workerd loader', () => {
  test('keeps the deferred Buffer import bundleable for workerd', async () => {
    const source = await readFile(deferredLoaderPath, 'utf8');

    expect(source).toContain(
      "// oxlint-disable-next-line unicorn/prefer-node-protocol -- workerd builds alias this bare specifier to the npm polyfill\nimport { Buffer } from 'buffer'\n",
    );
    expect(source).not.toContain("from 'node:buffer'");
  });

  test.each([
    {
      name: 'threadless target',
      options: { target: 'wasm32-wasip1' },
      active: 'threadless',
    },
    {
      name: 'native async-runtime build',
      options: { noDefaultFeatures: true, features: ['async-runtime'] },
      active: 'threadless',
    },
    {
      name: 'threaded target',
      options: { target: 'wasm32-wasip1-threads' },
      active: 'threaded',
    },
    {
      name: 'native default build',
      options: {},
      active: 'threaded',
    },
  ] as const)('preserves the inactive declaration during a $name', async ({ options, active }) => {
    const directory = await mkdtemp(join(tmpdir(), 'rolldown-wasi-declarations-'));
    const paths = {
      threaded: join(directory, 'threaded.d.cts'),
      threadless: join(directory, 'threadless.d.cts'),
    };
    try {
      await Promise.all([
        writeFile(paths.threaded, 'threaded-original'),
        writeFile(paths.threadless, 'threadless-original'),
      ]);
      const restore = preserveInactiveWasiDeclaration(options, paths);
      const inactive = active === 'threadless' ? 'threaded' : 'threadless';
      await Promise.all([
        writeFile(paths[active], `${active}-generated`),
        writeFile(paths[inactive], `${inactive}-overwritten`),
      ]);

      restore();

      await expect(readFile(paths[active], 'utf8')).resolves.toBe(`${active}-generated`);
      await expect(readFile(paths[inactive], 'utf8')).resolves.toBe(`${inactive}-original`);
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });

  test.each([
    {
      name: 'direct declaration',
      prefix: 'const ',
    },
    {
      name: 'deferred lifecycle assignment',
      prefix: '',
    },
  ])('rewrites the $name memory descriptor', ({ prefix }) => {
    const source = `${prefix}__wasmMemory = new WebAssembly.Memory({
  initial: 16384,
  maximum: 65536,
})
`;

    expect(
      rewriteThreadlessMemoryDescriptor(source, 'rolldown-binding.wasip1.cjs', {
        initialMemory: 1024,
        maximumMemory: 65536,
      }),
    ).toBe(`${prefix}__wasmMemory = new WebAssembly.Memory({
  initial: 1024,
  maximum: 65536,
})
`);
  });

  test('recognizes async-runtime in comma-combined napi CLI features', () => {
    const options = createBuildCommand([
      '--no-default-features',
      '--features',
      'async-runtime,runtime-waker-teardown-test',
    ]).getOptions();

    expect(options.features).toEqual(['async-runtime,runtime-waker-teardown-test']);
    expect(isAsyncRuntimeDeclarationBuild(options)).toBe(true);
  });

  test('restores all generated binding sources after a profile build fails', async () => {
    const directory = await mkdtemp(join(tmpdir(), 'rolldown-generated-binding-sources-'));
    const buildError = new Error('profile build failed');
    const paths = {
      binding: join(directory, 'binding.cjs'),
      browser: join(directory, 'browser.js'),
      declaration: join(directory, 'binding.d.cts'),
      loader: join(directory, 'rolldown-binding.wasip1-browser.js'),
      created: join(directory, 'wasi-worker-browser.mjs'),
      unrelated: join(directory, 'unrelated.ts'),
    };
    try {
      const originalBinding = Buffer.from([0x2f, 0x2f, 0x20, 0x64, 0x69, 0x72, 0x74, 0x79, 0xff]);
      await Promise.all([
        writeFile(paths.binding, originalBinding),
        writeFile(paths.browser, 'dirty browser entry'),
        writeFile(paths.declaration, 'dirty declaration'),
        writeFile(paths.loader, 'dirty loader'),
        writeFile(paths.unrelated, 'unrelated original'),
      ]);

      await expect(
        preserveGeneratedBindingSources(async () => {
          await Promise.all([
            writeFile(paths.binding, 'test-profile binding'),
            rm(paths.browser),
            rm(paths.declaration),
            writeFile(paths.loader, 'test-profile loader'),
            writeFile(paths.created, 'new generated worker'),
            writeFile(paths.unrelated, 'unrelated build output'),
          ]);
          throw buildError;
        }, directory),
      ).rejects.toBe(buildError);

      await expect(readFile(paths.binding)).resolves.toEqual(originalBinding);
      await expect(readFile(paths.browser, 'utf8')).resolves.toBe('dirty browser entry');
      await expect(readFile(paths.declaration, 'utf8')).resolves.toBe('dirty declaration');
      await expect(readFile(paths.loader, 'utf8')).resolves.toBe('dirty loader');
      await expect(readFile(paths.created, 'utf8')).rejects.toMatchObject({ code: 'ENOENT' });
      await expect(readFile(paths.unrelated, 'utf8')).resolves.toBe('unrelated build output');
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });

  test('keeps test-only runtime probes out of generated public sources', async () => {
    const [bindingSource, declarationSource] = await Promise.all([
      readFile(new URL('../src/binding.cjs', import.meta.url), 'utf8'),
      readFile(new URL('../src/binding.d.cts', import.meta.url), 'utf8'),
    ]);

    expect(bindingSource).not.toContain('__rolldownTest');
    expect(declarationSource).not.toContain('__rolldownTest');
  });

  test('hardens the generated browser initialization lifecycle', () => {
    const source = `const __wasmMemory = new WebAssembly.Memory({
  initial: 16384,
  maximum: 65536,
})

const __emnapiContext = __emnapiCreateContext()

function __createInitializationCleanupError(__error, __cleanupError) {
  return new AggregateError([__error, __cleanupError])
}

let __napiInstance
let __wasiModule
let __napiModule

try {
  __emnapiContext.feature.Buffer = Buffer

  ;({
    instance: __napiInstance,
    module: __wasiModule,
    napiModule: __napiModule,
  } = await __emnapiInstantiateNapiModule(__wasmFile, {}))
} catch (__error) {
  try {
    await __emnapiContext.destroy()
  } catch (__cleanupError) {
    throw __createInitializationCleanupError(__error, __cleanupError)
  }
  throw __error
}
export default __napiModule.exports
`;

    const hardened = injectCurrentThreadHostBootstrap(
      source,
      'rolldown-binding.wasip1-browser.js',
      'export default __napiModule.exports',
      true,
      {
        initialMemory: 1024,
        maximumMemory: 65536,
      },
    );

    expect(hardened).toContain(browserInitializationGuardStart);
    expect(hardened).toContain(browserInitializationGuardEnd);
    expect(hardened).toContain(currentThreadBootstrapStart);
    expect(hardened).toContain(currentThreadBootstrapEnd);
    expect(hardened).toContain('let __browserTaskHostRegistration');
    expect(hardened).toContain('initial: 1024');
    expect(hardened).toContain('__emnapiContext.feature.Buffer = Buffer');
    expect(hardened).toContain('Threadless browser initialization cleanup failed');
    expect(hardened).toContain('const __cleanupSync = (__operation, __message)');
    expect(hardened).toContain('const __cleanup = async');
    expect(hardened).toContain('await __cleanup(');
    expect(hardened).toContain('getCurrentThreadTaskHostContractVersion');
    expect(hardened).toContain('__taskHostContractVersion !== 4');
    expect(hardened).toContain(
      'Reflect.apply(\n      __reserveCurrentThreadHostRegistration,\n      __rolldownBinding,\n      [],\n    )',
    );
    expect(hardened).toContain(
      'Reflect.apply(__registerCurrentThreadTaskHost, __rolldownBinding, [\n    __taskHostRegistration.high,\n    __taskHostRegistration.low,\n  ])',
    );
    for (const removedExport of removedTaskHostExports) {
      expect(hardened).not.toContain(removedExport);
    }
    expect(
      injectCurrentThreadHostBootstrap(
        hardened,
        'rolldown-binding.wasip1-browser.js',
        'export default __napiModule.exports',
        true,
        {
          initialMemory: 1024,
          maximumMemory: 65536,
        },
      ),
    ).toBe(hardened);
  });

  test('hardens the generated napi-rs Node initialization lifecycle', async () => {
    const lifecycle = await readGeneratedNodeLifecycle();
    const source = `${lifecycle}
let __wasmMemory
let __napiModule

try {
  __registerEmnapiContextBeforeExit()

  __wasmMemory = new WebAssembly.Memory({
    initial: 16384,
    maximum: 65536,
  })

  ;({ napiModule: __napiModule } = __emnapiInstantiateNapiModuleSync())
  __handoffEmnapiContextCleanupToExit()
} catch (__error) {
  let __cleanupResult
  let __cleanupFailed = false
  try {
    __cleanupResult = __destroyEmnapiContext()
  } catch (__cleanupError) {
    __cleanupFailed = true
    __preserveCleanupError(__error, __cleanupError)
    try {
      __retainEmnapiContextCleanupListener()
    } catch (__listenerError) {
      __preserveCleanupError(__error, __listenerError)
    }
  }
  if (__cleanupResult) {
    void __cleanupResult.then(
      () => {
        try {
          __removeEmnapiContextCleanupListeners()
        } catch (__cleanupError) {
          __preserveCleanupError(__error, __cleanupError)
        }
      },
      (__cleanupError) => {
        __preserveCleanupError(__error, __cleanupError)
        try {
          __retainEmnapiContextCleanupListener()
        } catch (__listenerError) {
          __preserveCleanupError(__error, __listenerError)
        }
      },
    )
  } else if (!__cleanupFailed) {
    try {
      __removeEmnapiContextCleanupListeners()
    } catch (__cleanupError) {
      __preserveCleanupError(__error, __cleanupError)
    }
  }
  throw __error
}
module.exports = __napiModule.exports
`;

    const hardened = injectCurrentThreadHostBootstrap(
      source,
      'rolldown-binding.wasip1.cjs',
      '} catch (__error) {\n  let __cleanupResult\n  let __cleanupFailed = false',
      false,
      {
        initialMemory: 1024,
        maximumMemory: 65536,
      },
    );

    expect(hardened).toContain(nodeInitializationCleanupStart);
    expect(hardened).toContain(nodeInitializationCleanupEnd);
    expect(hardened).toContain(currentThreadBootstrapStart);
    expect(hardened).toContain(currentThreadBootstrapEnd);
    expect(hardened).toContain('let __nodeTaskHostRegistration');
    expect(hardened).toContain('let __nodeTimerHostRegistration');
    expect(hardened).toContain('__nodeTaskHostRegistration = __taskHostRegistration');
    expect(hardened).toContain('__nodeTimerHostRegistration = __timerHostRegistration');
    expect(hardened).toContain('initial: 1024');
    expect(hardened).toContain('Threadless Node timer-host cleanup failed');
    expect(hardened).toContain('Threadless Node task-host cleanup failed');
    expect(hardened).toContain('Threadless Node initialization cleanup failed');
    expect(hardened).toContain('for (let __attempt = 0; __attempt < 2; __attempt += 1)');
    expect(hardened).toContain('void __cleanupResult.then(');
    expect(hardened).toContain('__removeEmnapiContextCleanupListeners()');
    expect(hardened).toContain('__registerEmnapiContextBeforeExit()');
    expect(hardened).toContain('__retainEmnapiContextCleanupListener()');
    expect(hardened).toContain('__preserveCleanupError(__error, __cleanupError)');
    expect(
      injectCurrentThreadHostBootstrap(
        hardened,
        'rolldown-binding.wasip1.cjs',
        '} catch (__error) {\n  let __cleanupResult\n  let __cleanupFailed = false',
        false,
        {
          initialMemory: 1024,
          maximumMemory: 65536,
        },
      ),
    ).toBe(hardened);
  });

  test.each([
    {
      name: 'removes the context destroy helper',
      mutate: (source: string) =>
        source.replace(
          'function __destroyEmnapiContext() {',
          'function __destroyEmnapiContextMissing() {',
        ),
    },
    {
      name: 'weakens state-preserving listener removal',
      mutate: (source: string) =>
        source.replace(
          `    process.removeListener('beforeExit', __destroyEmnapiContextBeforeExit)
    __emnapiContextRegisteredForBeforeExit = false`,
          `    __emnapiContextRegisteredForBeforeExit = false
    process.removeListener('beforeExit', __destroyEmnapiContextBeforeExit)`,
        ),
    },
    {
      name: 'duplicates a lifecycle helper',
      mutate: (source: string) =>
        source.replace(
          'function __retainEmnapiContextCleanupListener() {',
          `function __retainEmnapiContextCleanupListener() {}

function __retainEmnapiContextCleanupListener() {`,
        ),
    },
  ])('rejects a marked Node loader that $name', async ({ mutate }) => {
    const source = await readFile(cjsLoaderPath, 'utf8');
    const mutated = mutate(source);
    expect(mutated).not.toBe(source);

    expect(() =>
      injectCurrentThreadHostBootstrap(
        mutated,
        'rolldown-binding.wasip1.cjs',
        '} catch (__error) {\n  let __cleanupResult\n  let __cleanupFailed = false',
        false,
        {
          initialMemory: 1024,
          maximumMemory: 65536,
        },
      ),
    ).toThrow(/Unexpected generated Node lifecycle contract/);
  });

  test('keeps the installed napi-rs CLI lifecycle contract in every executable bundle', async () => {
    const require = createRequire(import.meta.url);
    const packageDir = dirname(require.resolve('@napi-rs/cli/package.json'));
    const helperSignatures = [
      'function __removeEmnapiContextBeforeExitListener() {',
      'function __removeEmnapiContextAtExitListener() {',
      'function __removeEmnapiContextCleanupListeners() {',
      'function __retainEmnapiContextCleanupListener() {',
      'function __handoffEmnapiContextCleanupToExit() {',
      'function __preserveCleanupError(__error, __cleanupError) {',
    ];

    for (const name of ['cli.js', 'index.cjs', 'index.js']) {
      const source = await readFile(join(packageDir, 'dist', name), 'utf8');
      for (const signature of helperSignatures) {
        expect(countOccurrences(source, signature), `${name}: ${signature}`).toBe(1);
      }
      expect(source).toContain(
        `process.removeListener('beforeExit', __destroyEmnapiContextBeforeExit)
    __emnapiContextRegisteredForBeforeExit = false`,
      );
      expect(source).toContain(
        `process.removeListener('exit', __destroyEmnapiContextAtExit)
    __emnapiContextRegisteredForExit = false`,
      );
      expect(source).toContain(`      }
      return __error.cause === __cleanupError`);
      expect(source.match(/^  __handoffEmnapiContextCleanupToExit\(\)$/gm)).toHaveLength(1);
    }
  });

  test('keeps generated threaded browser context cleanup await lint-safe', async () => {
    const source = await readFile(threadedBrowserLoaderPath, 'utf8');

    expect(source).toContain('await Promise.resolve(__destroyEmnapiContext())');
    expect(source).not.toContain('await __destroyEmnapiContext()');
  });

  test('exposes createInstance and instantiate through the same managed host path', () => {
    expect(createInstance).toBe(instantiate);
  });

  test('does not create a managed context before the module promise settles', async () => {
    const createContext = vi.fn();
    const loader = await loadDeferredLoaderWithDependencies({
      createContext,
      getDefaultContext: vi.fn(),
      instantiateNapiModule: vi.fn(),
      WASI: class {},
    });
    const moduleError = new Error('module resolution failed');
    let rejectModule!: (error: unknown) => void;
    const module = new Promise<WebAssembly.Module>((_resolve, reject) => {
      rejectModule = reject;
    });

    const initialization = loader.createInstance(module);
    await Promise.resolve();
    expect(createContext).not.toHaveBeenCalled();

    rejectModule(moduleError);
    await expect(initialization).rejects.toBe(moduleError);
    expect(createContext).not.toHaveBeenCalled();
  });

  test('injects the imported Buffer constructor into managed emnapi contexts', async () => {
    const context = {
      feature: {} as { Buffer?: typeof NodeBuffer },
      suppressDestroy() {},
      destroy() {},
    };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async () => ({
        napiModule: {
          exports: {
            registerCurrentThreadTaskHost() {},
            registerTimerHost() {},
          },
        },
      }),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));

    const instance = await loader.createInstance(module, {
      initialMemoryPages: 1,
      maximumMemoryPages: 1,
    });
    try {
      expect(context.feature.Buffer).toBe(NodeBuffer);
    } finally {
      instance.dispose();
    }
  });

  test('destroys the browser context when top-level instantiation fails', async () => {
    const initializationError = new Error('browser instantiation failed');
    const destroy = vi.fn();
    const createContext = vi.fn(() => ({ destroy }));
    const instantiateNapiModule = vi.fn(() => Promise.reject(initializationError));

    await expect(
      loadBrowserLoaderWithDependencies({
        createContext,
        fetch: async () => ({
          ok: true,
          arrayBuffer: async () => new ArrayBuffer(0),
        }),
        instantiateNapiModule,
        memfs: () => ({ fs: {}, vol: {} }),
        WASI: class {},
      }),
    ).rejects.toBe(initializationError);

    expect(createContext).toHaveBeenCalledOnce();
    expect(instantiateNapiModule).toHaveBeenCalledOnce();
    expect(destroy).toHaveBeenCalledOnce();
  });

  test('retries transient browser host and context cleanup failures', async () => {
    const initializationError = new Error('browser timer host registration failed');
    const registration = { high: 0x1234_5678, low: 0x9abc_def0 };
    const prepareWasmEnvCleanup = vi.fn();
    const unregisterCurrentThreadTaskHost = vi
      .fn()
      .mockImplementationOnce(() => {
        throw new Error('transient task host cleanup failure');
      })
      .mockImplementationOnce(() => {});
    const destroy = vi
      .fn()
      .mockRejectedValueOnce(new Error('transient context cleanup failure'))
      .mockResolvedValueOnce(undefined);

    await expect(
      loadBrowserLoaderWithDependencies({
        createContext: () => ({ destroy }),
        fetch: async () => ({
          ok: true,
          arrayBuffer: async () => new ArrayBuffer(0),
        }),
        instantiateNapiModule: async () => ({
          instance: {
            exports: {
              napi_prepare_wasm_env_cleanup: prepareWasmEnvCleanup,
            },
          },
          module: {},
          napiModule: {
            exports: {
              getCurrentThreadTaskHostContractVersion: () => 4,
              isCurrentThreadHostRegistrationActive: (high: number, low: number) =>
                high === registration.high && low === registration.low,
              reserveCurrentThreadHostRegistration: () => registration,
              registerCurrentThreadTaskHost: () => {},
              registerTimerHost() {
                throw initializationError;
              },
              unregisterCurrentThreadTaskHost,
              unregisterTimerHost: vi.fn(),
            },
          },
        }),
        memfs: () => ({ fs: {}, vol: {} }),
        WASI: class {},
      }),
    ).rejects.toBe(initializationError);

    expect(unregisterCurrentThreadTaskHost).toHaveBeenCalledTimes(2);
    expect(unregisterCurrentThreadTaskHost).toHaveBeenNthCalledWith(
      1,
      registration.high,
      registration.low,
    );
    expect(unregisterCurrentThreadTaskHost).toHaveBeenNthCalledWith(
      2,
      registration.high,
      registration.low,
    );
    expect(prepareWasmEnvCleanup).toHaveBeenCalledOnce();
    expect(destroy).toHaveBeenCalledTimes(2);
  });

  test('rolls back the exact browser task host before context destruction', async () => {
    const registrationError = new Error('browser timer host registration failed');
    const unregisterErrors = [
      new Error('browser task host cleanup failed once'),
      new Error('browser task host cleanup failed twice'),
    ];
    const destroyErrors = [
      new Error('browser context cleanup failed once'),
      new Error('browser context cleanup failed twice'),
    ];
    const registration = { high: 0x1234_5678, low: 0x9abc_def0 };
    const timerRegistration = { high: 0x1234_5678, low: 0x9abc_def1 };
    const reservations = [registration, timerRegistration];
    const cleanupOrder: string[] = [];
    const unregisterTimerHost = vi.fn((high: number, low: number) => {
      cleanupOrder.push(`unregister timer ${high}:${low}`);
    });
    const rawBinding = {
      getCurrentThreadTaskHostContractVersion: () => 4,
      isCurrentThreadHostRegistrationActive: () => true,
      reserveCurrentThreadHostRegistration: () => reservations.shift(),
      registerCurrentThreadTaskHost() {
        cleanupOrder.push('register task');
      },
      registerTimerHost() {
        cleanupOrder.push('register timer');
        throw registrationError;
      },
      unregisterCurrentThreadTaskHost(high: number, low: number) {
        cleanupOrder.push(`unregister task ${high}:${low}`);
        throw unregisterErrors[
          cleanupOrder.filter((step) => step.startsWith('unregister task')).length - 1
        ];
      },
      unregisterTimerHost,
    };
    const context = {
      destroy() {
        cleanupOrder.push('destroy context');
        throw destroyErrors[cleanupOrder.filter((step) => step === 'destroy context').length - 1];
      },
    };

    const failure = await loadBrowserLoaderWithDependencies({
      createContext: () => context,
      fetch: async () => ({
        ok: true,
        arrayBuffer: async () => new ArrayBuffer(0),
      }),
      instantiateNapiModule: async () => ({
        instance: {},
        module: {},
        napiModule: { exports: rawBinding },
      }),
      memfs: () => ({ fs: {}, vol: {} }),
      WASI: class {},
    }).then(
      () => {
        throw new Error('Expected browser host registration to fail');
      },
      (error: unknown) => error,
    );

    expect(cleanupOrder).toEqual([
      'register task',
      'register timer',
      `unregister timer ${timerRegistration.high}:${timerRegistration.low}`,
      `unregister task ${registration.high}:${registration.low}`,
      `unregister task ${registration.high}:${registration.low}`,
      'destroy context',
      'destroy context',
    ]);
    // The reserved timer token is rolled back exactly once even though its
    // registration threw: v4 reserves the capability before side effects, so
    // cleanup can always target the exact token.
    expect(unregisterTimerHost).toHaveBeenCalledTimes(1);
    expect(unregisterTimerHost).toHaveBeenCalledWith(timerRegistration.high, timerRegistration.low);
    expect(failure).toMatchObject({
      cause: registrationError,
      errors: [
        registrationError,
        expect.objectContaining({
          message: 'Threadless browser initialization cleanup failed',
          errors: [
            expect.objectContaining({
              message: 'Threadless browser task-host cleanup failed',
              errors: unregisterErrors,
            }),
            expect.objectContaining({
              message: 'Threadless browser context cleanup failed',
              errors: destroyErrors,
            }),
          ],
        }),
      ],
    });
  });

  test('unregisters the exact managed task host when timer registration fails', async () => {
    const registrationError = new Error('timer host registration failed');
    const registration = { high: 0x1234_5678, low: 0x9abc_def0 };
    const timerRegistration = { high: 0x1234_5678, low: 0x9abc_def1 };
    const reservations = [registration, timerRegistration];
    const live = new Set<number>();
    const cleanupOrder: string[] = [];
    const rawBinding = {
      getCurrentThreadTaskHostContractVersion: () => 4,
      isCurrentThreadHostRegistrationActive: vi.fn((_high: number, low: number) => live.has(low)),
      reserveCurrentThreadHostRegistration: vi.fn(() => reservations.shift()),
      registerCurrentThreadTaskHost: vi.fn((_high: number, low: number) => {
        cleanupOrder.push('register task');
        live.add(low);
      }),
      unregisterCurrentThreadTaskHost: vi.fn((high: number, low: number) => {
        cleanupOrder.push(`unregister task ${high}:${low}`);
        live.delete(low);
      }),
      registerTimerHost: vi.fn(() => {
        cleanupOrder.push('register timer');
        throw registrationError;
      }),
      unregisterTimerHost: vi.fn((high: number, low: number) => {
        cleanupOrder.push(`unregister timer ${high}:${low}`);
        live.delete(low);
      }),
    };
    const context = {
      suppressDestroy() {},
      destroy() {
        cleanupOrder.push('destroy context');
      },
    };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async () => ({ napiModule: { exports: rawBinding } }),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));

    await expect(
      loader.createInstance(module, {
        initialMemoryPages: 1,
        maximumMemoryPages: 1,
      }),
    ).rejects.toBe(registrationError);
    expect(rawBinding.registerCurrentThreadTaskHost).toHaveBeenCalledWith(
      registration.high,
      registration.low,
    );
    expect(rawBinding.unregisterCurrentThreadTaskHost).toHaveBeenCalledWith(
      registration.high,
      registration.low,
    );
    // The reserved timer token is rolled back even though its registration
    // threw: v4 reserves the capability before side effects, so cleanup can
    // always target the exact token.
    expect(rawBinding.unregisterTimerHost).toHaveBeenCalledWith(
      timerRegistration.high,
      timerRegistration.low,
    );
    expect(cleanupOrder).toEqual([
      'register task',
      'register timer',
      `unregister timer ${timerRegistration.high}:${timerRegistration.low}`,
      `unregister task ${registration.high}:${registration.low}`,
      'destroy context',
    ]);
  });

  test('rejects an inactive managed task-host registration before timer registration', async () => {
    const registration = { high: 0x1234_5678, low: 0x9abc_def0 };
    const context = {
      suppressDestroy() {},
      destroy: vi.fn(),
    };
    const rawBinding = {
      getCurrentThreadTaskHostContractVersion: () => 4,
      // The binding accepts the registration but never reports it live.
      isCurrentThreadHostRegistrationActive: vi.fn(() => false),
      reserveCurrentThreadHostRegistration: vi.fn(() => registration),
      registerCurrentThreadTaskHost: vi.fn(),
      unregisterCurrentThreadTaskHost: vi.fn(),
      registerTimerHost: vi.fn(),
      unregisterTimerHost: vi.fn(),
    };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async () => ({ napiModule: { exports: rawBinding } }),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));

    await expect(
      loader.createInstance(module, {
        initialMemoryPages: 1,
        maximumMemoryPages: 1,
      }),
    ).rejects.toThrow(/inactive task host registration/);
    expect(rawBinding.registerCurrentThreadTaskHost).toHaveBeenCalledWith(
      registration.high,
      registration.low,
    );
    expect(rawBinding.registerTimerHost).not.toHaveBeenCalled();
    // The reserved token is rolled back exactly: the registration performed
    // side effects even though the liveness revalidation failed.
    expect(rawBinding.unregisterCurrentThreadTaskHost).toHaveBeenCalledWith(
      registration.high,
      registration.low,
    );
    expect(rawBinding.unregisterTimerHost).not.toHaveBeenCalled();
    expect(context.destroy).toHaveBeenCalledOnce();
  });

  test('retains context cleanup diagnostics for primitive host registration failures', async () => {
    const primaryError: unknown = 'primitive host registration failure';
    const cleanupErrors = [new Error('cleanup failed once'), new Error('cleanup failed twice')];
    let cleanupAttempt = 0;
    const context = {
      suppressDestroy() {},
      destroy() {
        throw cleanupErrors[cleanupAttempt++];
      },
    };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async () => ({
        napiModule: {
          exports: {
            registerCurrentThreadTaskHost() {
              throw primaryError;
            },
            registerTimerHost() {},
          },
        },
      }),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
    const failure = await loader
      .createInstance(module, {
        initialMemoryPages: 1,
        maximumMemoryPages: 1,
      })
      .then(
        () => {
          throw new Error('Expected managed host registration to fail');
        },
        (error: unknown) => error,
      );

    expect(failure).toMatchObject({
      cause: primaryError,
      errors: [
        primaryError,
        expect.objectContaining({
          message: 'Managed workerd initialization cleanup failed',
          errors: [
            expect.objectContaining({
              message: 'Managed workerd context cleanup failed',
              errors: cleanupErrors,
            }),
          ],
        }),
      ],
    });
  });

  test('destroys a context whose setup fails before instantiation', async () => {
    const setupError = new Error('suppressDestroy failed');
    const destroy = vi.fn();
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => ({
        suppressDestroy() {
          throw setupError;
        },
        destroy,
      }),
      getDefaultContext: () => {
        throw new Error('default context should not be used');
      },
      instantiateNapiModule: vi.fn(),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));

    await expect(
      loader.createInstance(module, {
        initialMemoryPages: 1,
        maximumMemoryPages: 1,
      }),
    ).rejects.toBe(setupError);
    expect(destroy).toHaveBeenCalledOnce();
  });

  test('retains context setup cleanup failures', async () => {
    const setupError = new Error('suppressDestroy failed');
    const cleanupErrors = [new Error('destroy failed once'), new Error('destroy failed twice')];
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => ({
        suppressDestroy() {
          throw setupError;
        },
        destroy() {
          throw cleanupErrors.shift();
        },
      }),
      getDefaultContext: () => {
        throw new Error('default context should not be used');
      },
      instantiateNapiModule: vi.fn(),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));

    const failure = await loader
      .createInstance(module, {
        initialMemoryPages: 1,
        maximumMemoryPages: 1,
      })
      .then(
        () => {
          throw new Error('Expected context setup to fail');
        },
        (error: unknown) => error,
      );
    expect(failure).toBeInstanceOf(AggregateError);
    expect(failure).toMatchObject({
      cause: setupError,
      errors: [
        setupError,
        expect.objectContaining({
          message: 'Managed workerd context setup cleanup failed',
          errors: [
            expect.objectContaining({
              message: 'Managed workerd context setup cleanup failed',
              errors: [expect.any(Error), expect.any(Error)],
            }),
          ],
        }),
      ],
    });
  });

  test('retries transient beforeExit listener cleanup during failed context setup', async () => {
    const setupError = new Error('suppressDestroy failed');
    const beforeExitListener = () => {};
    let beforeExitListeners: Array<() => void> = [];
    let newListeners: Array<(event: string, listener: () => void) => void> = [];
    let beforeExitRemoveCalls = 0;
    const destroy = vi.fn();
    vi.stubGlobal('process', {
      getMaxListeners: () => 10,
      setMaxListeners() {},
      rawListeners(event: string) {
        return event === 'newListener' ? [...newListeners] : [...beforeExitListeners];
      },
      prependListener(_event: string, listener: (event: string, listener: () => void) => void) {
        newListeners.unshift(listener);
      },
      removeListener(event: string, listener: () => void) {
        if (event === 'newListener') {
          newListeners = newListeners.filter((candidate) => candidate !== listener);
          return;
        }
        beforeExitRemoveCalls += 1;
        if (beforeExitRemoveCalls === 1) {
          throw new Error('transient listener cleanup failure');
        }
        beforeExitListeners = beforeExitListeners.filter((candidate) => candidate !== listener);
      },
    });
    try {
      const loader = await loadDeferredLoaderWithDependencies({
        createContext: () => {
          for (const listener of newListeners.slice()) {
            listener('beforeExit', beforeExitListener);
          }
          beforeExitListeners.push(beforeExitListener);
          return {
            suppressDestroy() {
              throw setupError;
            },
            destroy,
          };
        },
        getDefaultContext: () => {
          throw new Error('default context should not be used');
        },
        instantiateNapiModule: vi.fn(),
        WASI: class {},
      });
      const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));

      await expect(
        loader.createInstance(module, {
          initialMemoryPages: 1,
          maximumMemoryPages: 1,
        }),
      ).rejects.toBe(setupError);
      expect(beforeExitRemoveCalls).toBe(2);
      expect(newListeners).toEqual([]);
      expect(beforeExitListeners).toEqual([]);
      expect(destroy).toHaveBeenCalledOnce();
    } finally {
      vi.unstubAllGlobals();
    }
  });

  test('removes only the added occurrence of a duplicate beforeExit listener', async () => {
    const setupError = new Error('suppressDestroy failed');
    const beforeExitListener = () => {};
    const beforeExitListeners = [beforeExitListener];
    const newListeners: Array<(event: string, listener: () => void) => void> = [];
    const removeListener = vi.fn((event: string, listener: () => void) => {
      const listeners = event === 'newListener' ? newListeners : beforeExitListeners;
      const index = listeners.lastIndexOf(listener);
      if (index >= 0) listeners.splice(index, 1);
    });
    const destroy = vi.fn();
    vi.stubGlobal('process', {
      getMaxListeners: () => 10,
      setMaxListeners() {},
      rawListeners(event: string) {
        return event === 'newListener' ? [...newListeners] : [...beforeExitListeners];
      },
      prependListener(_event: string, listener: (event: string, listener: () => void) => void) {
        newListeners.unshift(listener);
      },
      removeListener,
    });
    try {
      const loader = await loadDeferredLoaderWithDependencies({
        createContext: () => {
          for (const listener of newListeners.slice()) {
            listener('beforeExit', beforeExitListener);
          }
          beforeExitListeners.push(beforeExitListener);
          return {
            suppressDestroy() {
              throw setupError;
            },
            destroy,
          };
        },
        getDefaultContext: () => {
          throw new Error('default context should not be used');
        },
        instantiateNapiModule: vi.fn(),
        WASI: class {},
      });
      const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));

      await expect(
        loader.createInstance(module, {
          initialMemoryPages: 1,
          maximumMemoryPages: 1,
        }),
      ).rejects.toBe(setupError);
      expect(removeListener).toHaveBeenCalledWith('beforeExit', beforeExitListener);
      expect(newListeners).toEqual([]);
      expect(beforeExitListeners).toEqual([beforeExitListener]);
      expect(destroy).toHaveBeenCalledOnce();
    } finally {
      vi.unstubAllGlobals();
    }
  });

  test('preserves beforeExit listeners installed by existing newListener hooks', async () => {
    const setupError = new Error('suppressDestroy failed');
    const autoDestroyListener = () => {};
    const foreignBeforeExitListener = () => {};
    const beforeExitListeners: Array<() => void> = [];
    const newListeners: Array<(event: string, listener: () => void) => void> = [];
    const emitNewListener = (event: string, listener: () => void) => {
      for (const candidate of newListeners.slice()) candidate(event, listener);
    };
    const foreignNewListenerHook = (event: string, listener: () => void) => {
      if (event === 'beforeExit' && listener === autoDestroyListener) {
        emitNewListener('beforeExit', foreignBeforeExitListener);
        beforeExitListeners.push(foreignBeforeExitListener);
      }
    };
    newListeners.push(foreignNewListenerHook);
    const destroy = vi.fn();
    vi.stubGlobal('process', {
      getMaxListeners: () => 10,
      setMaxListeners() {},
      rawListeners(event: string) {
        return event === 'newListener' ? [...newListeners] : [...beforeExitListeners];
      },
      prependListener(_event: string, listener: (event: string, listener: () => void) => void) {
        newListeners.unshift(listener);
      },
      removeListener(event: string, listener: () => void) {
        const listeners = event === 'newListener' ? newListeners : beforeExitListeners;
        const index = listeners.lastIndexOf(listener);
        if (index >= 0) listeners.splice(index, 1);
      },
    });
    try {
      const loader = await loadDeferredLoaderWithDependencies({
        createContext: () => {
          emitNewListener('beforeExit', autoDestroyListener);
          beforeExitListeners.push(autoDestroyListener);
          return {
            suppressDestroy() {
              throw setupError;
            },
            destroy,
          };
        },
        getDefaultContext: () => {
          throw new Error('default context should not be used');
        },
        instantiateNapiModule: vi.fn(),
        WASI: class {},
      });
      const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));

      await expect(
        loader.createInstance(module, {
          initialMemoryPages: 1,
          maximumMemoryPages: 1,
        }),
      ).rejects.toBe(setupError);
      expect(newListeners).toEqual([foreignNewListenerHook]);
      expect(beforeExitListeners).toEqual([foreignBeforeExitListener]);
      expect(destroy).toHaveBeenCalledOnce();
    } finally {
      vi.unstubAllGlobals();
    }
  });

  test('raises the listener limit for the temporary newListener capture hook', async () => {
    const setupError = new Error('suppressDestroy failed');
    const existingNewListener = () => {};
    const newListeners: Array<(...args: unknown[]) => void> = [existingNewListener];
    const beforeExitListeners: Array<() => void> = [];
    const setMaxListeners = vi.fn();
    vi.stubGlobal('process', {
      getMaxListeners: () => 1,
      setMaxListeners,
      rawListeners(event: string) {
        return event === 'newListener' ? [...newListeners] : [...beforeExitListeners];
      },
      prependListener(_event: string, listener: (...args: unknown[]) => void) {
        newListeners.unshift(listener);
      },
      removeListener(event: string, listener: (...args: unknown[]) => void) {
        const listeners = event === 'newListener' ? newListeners : beforeExitListeners;
        const index = listeners.lastIndexOf(listener);
        if (index >= 0) listeners.splice(index, 1);
      },
    });
    try {
      const loader = await loadDeferredLoaderWithDependencies({
        createContext: () => ({
          suppressDestroy() {
            throw setupError;
          },
          destroy() {},
        }),
        getDefaultContext: () => {
          throw new Error('default context should not be used');
        },
        instantiateNapiModule: vi.fn(),
        WASI: class {},
      });
      const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));

      await expect(
        loader.createInstance(module, {
          initialMemoryPages: 1,
          maximumMemoryPages: 1,
        }),
      ).rejects.toBe(setupError);
      expect(setMaxListeners.mock.calls).toEqual([[2], [1]]);
      expect(newListeners).toEqual([existingNewListener]);
    } finally {
      vi.unstubAllGlobals();
    }
  });

  wasiTest(
    'owns independent concurrent instances with idempotent disposal',
    { timeout: 60_000 },
    async () => {
      const module = await WebAssembly.compile(await readFile(wasmPath));
      const before = getWorkerdRuntimeStats();
      const [first, second] = await Promise.all([
        instantiate(module),
        instantiate(Promise.resolve(module)),
      ]);

      expect(first.memory).not.toBe(second.memory);
      expect(first.memoryBytes).toBeGreaterThanOrEqual(WORKERD_WASM_MEMORY.initialBytes);
      expect(first.exports.getRuntimeCapabilities()).toMatchObject({
        target: 'wasi',
        flavor: 'CurrentThread',
        timers: true,
        watchSupported: false,
      });
      const firstBinding = first.exports;
      const retainedCapabilities = firstBinding.getRuntimeCapabilities;
      const RetainedBundler = firstBinding.BindingBundler;
      for (const privateHostExport of privateManagedHostExports) {
        expect(firstBinding).not.toHaveProperty(privateHostExport);
        expect(second.exports).not.toHaveProperty(privateHostExport);
      }
      expect(getWorkerdRuntimeStats()).toMatchObject({
        createdInstances: before.createdInstances + 2,
        liveInstances: before.liveInstances + 2,
      });

      first.dispose();
      first.dispose();
      expect(first.disposed).toBe(true);
      expect(first.memoryBytes).toBe(0);
      expect(() => first.exports).toThrow(/disposed/);
      expect(() => first.memory).toThrow(/disposed/);
      expect(() => retainedCapabilities()).toThrow(
        'This workerd Rolldown instance has been disposed',
      );
      expect(() => new RetainedBundler()).toThrow(
        'This workerd Rolldown instance has been disposed',
      );
      expect(getWorkerdRuntimeStats().liveInstances).toBe(before.liveInstances + 1);

      second.dispose();
      expect(getWorkerdRuntimeStats().liveInstances).toBe(before.liveInstances);
    },
  );

  test('guards active operations and open binding objects before context destruction', async () => {
    let finishGenerate!: (value: string) => void;
    class BindingBundler {
      generate(): Promise<string> {
        return new Promise((resolve) => {
          finishGenerate = resolve;
        });
      }

      close(): Promise<void> {
        return Promise.resolve();
      }
    }
    const getRuntimeCapabilities = vi.fn(() => ({ target: 'wasi' }));
    const rawBinding = {
      BindingBundler,
      getRuntimeCapabilities,
      registerCurrentThreadTaskHost: vi.fn(),
      registerTimerHost: vi.fn(),
    };
    const destroy = vi.fn();
    const context = {
      suppressDestroy() {},
      destroy,
    };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async (
        _module: WebAssembly.Module,
        options: { beforeInit: (input: { instance: { exports: object } }) => void },
      ) => {
        options.beforeInit({ instance: { exports: {} } });
        return { napiModule: { exports: rawBinding } };
      },
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
    const instance = await loader.createInstance(module, {
      initialMemoryPages: 1,
      maximumMemoryPages: 1,
    });
    const binding = instance.exports;

    expect(loader.instantiate).toBe(loader.createInstance);
    expect(loader).not.toHaveProperty('getDeferredInstanceBinding');
    for (const privateHostExport of privateManagedHostExports) {
      expect(loader).not.toHaveProperty(privateHostExport);
      expect(binding).not.toHaveProperty(privateHostExport);
    }

    const RetainedBundler = binding.BindingBundler;
    const retainedCapabilities = binding.getRuntimeCapabilities;
    const bundler = new RetainedBundler();
    const generation = bundler.generate();

    expect(() => instance.dispose()).toThrow(
      /1 active binding operation and 1 open binding object/,
    );
    expect(destroy).not.toHaveBeenCalled();
    expect(instance.disposed).toBe(false);

    finishGenerate('generated');
    await expect(generation).resolves.toBe('generated');
    expect(() => instance.dispose()).toThrow(/1 open binding object/);
    expect(destroy).not.toHaveBeenCalled();

    await bundler.close();
    instance.dispose();
    expect(destroy).toHaveBeenCalledOnce();
    expect(instance.disposed).toBe(true);
    expect(() => retainedCapabilities()).toThrow(
      'This workerd Rolldown instance has been disposed',
    );
    expect(() => new RetainedBundler()).toThrow('This workerd Rolldown instance has been disposed');
    expect(getRuntimeCapabilities).not.toHaveBeenCalled();
  });

  test('releases only terminally closed objects after close rejection', async () => {
    const retryableCloseError = new Error('retryable close failure');
    const terminalCloseError = new Error('terminal close failure');
    let terminal = false;
    class BindingBundler {
      closed = false;

      close(): Promise<void> {
        this.closed = terminal;
        return Promise.reject(terminal ? terminalCloseError : retryableCloseError);
      }
    }
    const rawBinding = {
      BindingBundler,
      registerCurrentThreadTaskHost() {},
      registerTimerHost() {},
    };
    const context = { suppressDestroy() {}, destroy: vi.fn() };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async () => ({ napiModule: { exports: rawBinding } }),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
    const instance = await loader.createInstance(module, {
      initialMemoryPages: 1,
      maximumMemoryPages: 1,
    });
    const bundler = new instance.exports.BindingBundler();

    await expect(bundler.close()).rejects.toBe(retryableCloseError);
    expect(() => instance.dispose()).toThrow(/1 open binding object/);

    terminal = true;
    await expect(bundler.close()).rejects.toBe(terminalCloseError);
    instance.dispose();

    expect(context.destroy).toHaveBeenCalledOnce();
  });

  test('mediates returned callables and retargets mutable raw function fields', async () => {
    let closeCalls = 0;
    class BindingCallableRecord {
      mutableCallback = () => 'first';

      get accessorCallback(): () => string {
        return () => 'accessor';
      }

      returnCallback(): () => string {
        return () => 'method';
      }

      replaceMutableCallback(): void {
        this.mutableCallback = () => 'second';
      }
    }
    class BindingAccessorClose {
      get close(): () => Promise<void> {
        return () => {
          closeCalls += 1;
          return Promise.resolve();
        };
      }
    }
    const rawBinding = {
      BindingAccessorClose,
      BindingCallableRecord,
      registerCurrentThreadTaskHost() {},
      registerTimerHost() {},
    };
    const context = { suppressDestroy() {}, destroy: vi.fn() };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async () => ({ napiModule: { exports: rawBinding } }),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
    const instance = await loader.createInstance(module, {
      initialMemoryPages: 1,
      maximumMemoryPages: 1,
    });
    const record = new instance.exports.BindingCallableRecord();
    const closable = new instance.exports.BindingAccessorClose();

    const returnedCallback = record.returnCallback();
    const accessorCallback = record.accessorCallback;
    const mutableCallback = record.mutableCallback;
    expect(returnedCallback()).toBe('method');
    expect(accessorCallback()).toBe('accessor');
    expect(mutableCallback()).toBe('first');

    record.replaceMutableCallback();
    expect(record.mutableCallback).toBe(mutableCallback);
    expect(mutableCallback()).toBe('second');

    expect(() => instance.dispose()).toThrow(/1 open binding object/);
    const retainedClose = closable.close;
    await retainedClose();
    expect(closeCalls).toBe(1);

    instance.dispose();
    expect(context.destroy).toHaveBeenCalledOnce();
    for (const callback of [returnedCallback, accessorCallback, mutableCallback, retainedClose]) {
      expect(() => callback()).toThrow('This workerd Rolldown instance has been disposed');
    }
  });

  test('rejects close replacement without releasing the disposal barrier', async () => {
    const close = vi.fn(() => Promise.resolve());
    function BindingBundler() {}
    const closablePrototype = {
      close(): Promise<void> {
        return close();
      },
    };
    Object.setPrototypeOf(BindingBundler.prototype, closablePrototype);
    const rawBinding = {
      BindingBundler,
      registerCurrentThreadTaskHost() {},
      registerTimerHost() {},
    };
    const context = { suppressDestroy() {}, destroy: vi.fn() };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async () => ({ napiModule: { exports: rawBinding } }),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
    const instance = await loader.createInstance(module, {
      initialMemoryPages: 1,
      maximumMemoryPages: 1,
    });
    const Bundler = instance.exports.BindingBundler;
    const originalPrototype = Bundler.prototype;
    const bundler = new Bundler();
    class DerivedBundler extends Bundler {}
    const derived = new DerivedBundler();
    const replacement = () => Promise.resolve();

    expect(Object.getOwnPropertyDescriptor(Bundler, 'prototype')).toMatchObject({
      writable: true,
    });
    expect(() => Reflect.set(Bundler, 'prototype', { close: replacement })).toThrow(
      /Cannot replace or remove close/,
    );
    expect(() =>
      Object.defineProperty(Bundler, 'prototype', {
        value: { close: replacement },
      }),
    ).toThrow(/Cannot replace or remove close/);
    expect(Bundler.prototype).toBe(originalPrototype);
    expect(() => Reflect.set(bundler, 'close', replacement)).toThrow(
      /Cannot replace or remove close/,
    );
    expect(() => Reflect.set(Object.getPrototypeOf(bundler), 'close', replacement)).toThrow(
      /Cannot replace or remove close/,
    );
    expect(() => Object.defineProperty(bundler, 'close', { value: replacement })).toThrow(
      /Cannot replace or remove close/,
    );
    expect(() => Object.setPrototypeOf(bundler, { close: replacement })).toThrow(
      /Cannot replace or remove close/,
    );
    expect(() => Object.setPrototypeOf(Bundler.prototype, {})).toThrow(
      /Cannot replace or remove close/,
    );
    expect(() => Object.preventExtensions(Bundler.prototype)).not.toThrow();

    Object.setPrototypeOf(DerivedBundler.prototype, {});
    expect(() => Object.preventExtensions(derived)).toThrow(/Cannot replace or remove close/);
    expect(Object.isExtensible(derived)).toBe(true);
    expect(() => instance.dispose()).toThrow(/2 open binding objects/);

    await bundler.close();
    await derived.close();
    expect(close).toHaveBeenCalledTimes(2);
    instance.dispose();
    expect(context.destroy).toHaveBeenCalledOnce();
  });

  test('preserves binding class, prototype, and object reflection invariants', async () => {
    class BindingBundler {
      static kind = 'bundler';
      readonly ownValue = 1;

      close(): void {}
    }
    const rawBinding = {
      BindingBundler,
      registerCurrentThreadTaskHost() {},
      registerTimerHost() {},
    };
    const context = { suppressDestroy() {}, destroy: vi.fn() };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async () => ({ napiModule: { exports: rawBinding } }),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
    const instance = await loader.createInstance(module, {
      initialMemoryPages: 1,
      maximumMemoryPages: 1,
    });
    const Bundler = instance.exports.BindingBundler;

    expect(Reflect.ownKeys(Bundler)).toContain('kind');
    expect(Object.getOwnPropertyDescriptor(Bundler, 'kind')).toMatchObject({
      enumerable: true,
      value: 'bundler',
      writable: true,
    });
    expect(Reflect.ownKeys(Bundler.prototype)).toEqual(
      expect.arrayContaining(['constructor', 'close']),
    );
    expect(Object.getOwnPropertyDescriptor(Bundler, 'prototype')).toMatchObject({
      configurable: false,
      enumerable: false,
      writable: false,
    });
    expect(Object.getOwnPropertyDescriptor(Bundler.prototype, 'close')).toMatchObject({
      configurable: true,
      enumerable: false,
      writable: true,
    });

    const bundler = new Bundler();
    expect('close' in bundler).toBe(true);
    expect(Object.getPrototypeOf(bundler)).toBe(Bundler.prototype);
    expect(Object.getOwnPropertyDescriptor(bundler, 'ownValue')).toMatchObject({
      configurable: true,
      enumerable: true,
      value: 1,
      writable: true,
    });

    Object.defineProperty(bundler, 'fixedExpando', {
      configurable: false,
      enumerable: true,
      value: 2,
      writable: false,
    });
    Reflect.set(bundler, 'temporaryExpando', 3);
    expect(Reflect.ownKeys(bundler)).toEqual(
      expect.arrayContaining(['fixedExpando', 'ownValue', 'temporaryExpando']),
    );
    expect(Reflect.deleteProperty(bundler, 'temporaryExpando')).toBe(true);
    expect('temporaryExpando' in bundler).toBe(false);
    expect(bundler).toHaveProperty('fixedExpando', 2);

    class DerivedBundler extends Bundler {}
    const derived = new DerivedBundler();
    expect(Object.getPrototypeOf(derived)).toBe(DerivedBundler.prototype);
    expect(derived).toBeInstanceOf(DerivedBundler);
    expect(derived).toBeInstanceOf(Bundler);

    expect(() => Object.preventExtensions(bundler)).not.toThrow();
    expect(Object.isExtensible(bundler)).toBe(false);
    expect(Reflect.defineProperty(bundler, 'lateExpando', { value: 4 })).toBe(false);
    expect(() => Object.freeze(bundler)).not.toThrow();
    expect(Object.isFrozen(bundler)).toBe(true);

    const prototype = Bundler.prototype;
    Reflect.set(prototype, 'prototypeExpando', 5);
    expect(bundler).toHaveProperty('prototypeExpando', 5);
    expect(Reflect.deleteProperty(prototype, 'prototypeExpando')).toBe(true);
    Object.defineProperty(prototype, 'fixedPrototypeExpando', {
      configurable: false,
      value: 6,
      writable: false,
    });
    expect(() => Object.freeze(prototype)).not.toThrow();
    expect(Object.isFrozen(prototype)).toBe(true);
    expect(Object.getPrototypeOf(bundler)).toBe(prototype);
    expect('close' in bundler).toBe(true);

    bundler.close();
    derived.close();
    instance.dispose();
    expect(context.destroy).toHaveBeenCalledOnce();
    expect(() => Reflect.ownKeys(Bundler.prototype)).toThrow(/disposed/);
  });

  test('preserves constructor identity when a subclass export precedes its base', async () => {
    class BindingBase {
      static baseOnly = 'base';
      static self = BindingBase;

      close(): void {}
    }
    class BindingDerived extends BindingBase {
      static base = BindingBase;
      static self = BindingDerived;
    }
    const rawBinding = {
      BindingDerived,
      BindingBase,
      registerCurrentThreadTaskHost() {},
      registerTimerHost() {},
    };
    const context = { suppressDestroy() {}, destroy: vi.fn() };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async () => ({ napiModule: { exports: rawBinding } }),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
    const instance = await loader.createInstance(module, {
      initialMemoryPages: 1,
      maximumMemoryPages: 1,
    });
    const exports = instance.exports as unknown as {
      BindingBase: typeof BindingBase;
      BindingDerived: typeof BindingDerived;
    };
    const Base = exports.BindingBase;
    const Derived = exports.BindingDerived;

    expect(Base.prototype.constructor).toBe(Base);
    expect(Derived.prototype.constructor).toBe(Derived);
    expect(Object.getPrototypeOf(Derived.prototype)).toBe(Base.prototype);
    expect(Object.getPrototypeOf(Derived)).toBe(Base);
    expect(Derived.baseOnly).toBe('base');
    expect(Base.self).toBe(Base);
    expect(Derived.base).toBe(Base);
    expect(Derived.self).toBe(Derived);
    const derived = new Derived();
    expect(derived).toBeInstanceOf(Derived);
    expect(derived).toBeInstanceOf(Base);

    derived.close();
    instance.dispose();
    expect(context.destroy).toHaveBeenCalledOnce();
  });

  test('wraps hidden constructor ancestry and accounts for inherited static operations', async () => {
    let resolvePending!: (value: string) => void;
    let rawCreated: HiddenBase | undefined;
    class HiddenBase {
      static create(): HiddenBase {
        return (rawCreated = new HiddenBase());
      }

      static wait(this: void): Promise<string> {
        return new Promise((resolve) => {
          resolvePending = resolve;
        });
      }

      value(this: void): string {
        return 'hidden';
      }

      close(): void {}
    }
    class BindingDerived extends HiddenBase {}
    const rawBinding = {
      BindingDerived,
      registerCurrentThreadTaskHost() {},
      registerTimerHost() {},
    };
    const context = { suppressDestroy() {}, destroy: vi.fn() };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async () => ({ napiModule: { exports: rawBinding } }),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
    const instance = await loader.createInstance(module, {
      initialMemoryPages: 1,
      maximumMemoryPages: 1,
    });
    const Derived = instance.exports.BindingDerived as typeof BindingDerived;
    const Hidden = Object.getPrototypeOf(Derived) as typeof HiddenBase;

    expect(Hidden).not.toBe(HiddenBase);
    expect(Hidden.prototype).toBe(Object.getPrototypeOf(Derived.prototype));
    const retainedWait = Derived.wait;
    expect(retainedWait).toBe(Hidden.wait);

    const pending = retainedWait();
    expect(() => instance.dispose()).toThrow(/1 active binding operation/);
    resolvePending('settled');
    await expect(pending).resolves.toBe('settled');

    const hidden = Derived.create();
    const retainedValue = hidden.value;
    expect(hidden).not.toBe(rawCreated);
    expect(Object.getPrototypeOf(hidden)).toBe(Hidden.prototype);
    expect(retainedValue()).toBe('hidden');
    expect(() => instance.dispose()).toThrow(/1 open binding object/);

    hidden.close();
    instance.dispose();
    expect(context.destroy).toHaveBeenCalledOnce();
    expect(() => retainedWait()).toThrow(/disposed/);
    expect(() => retainedValue()).toThrow(/disposed/);
  });

  test('synchronizes raw binding fields across facade integrity changes', async () => {
    class BindingMutableRecord {
      declare fixed: number;
      value = 1;

      constructor() {
        Object.defineProperty(this, 'fixed', {
          configurable: true,
          enumerable: true,
          value: 1,
          writable: false,
        });
      }

      setValue(value: number): void {
        this.value = value;
      }

      replaceFixed(value: number): void {
        Object.defineProperty(this, 'fixed', {
          value,
        });
      }

      mutateBeforePreventExtensions(): void {
        this.value = 2;
        Object.defineProperty(this, 'late', {
          configurable: true,
          enumerable: true,
          value: 3,
          writable: true,
        });
      }

      mutateAfterPreventExtensions(): void {
        this.value = 4;
        Reflect.set(this, 'late', 5);
        Reflect.set(this, 'afterLock', 6);
      }
    }
    const rawBinding = {
      BindingMutableRecord,
      registerCurrentThreadTaskHost() {},
      registerTimerHost() {},
    };
    const context = { suppressDestroy() {}, destroy: vi.fn() };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async () => ({ napiModule: { exports: rawBinding } }),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
    const instance = await loader.createInstance(module, {
      initialMemoryPages: 1,
      maximumMemoryPages: 1,
    });
    const record = new instance.exports.BindingMutableRecord();

    record.mutateBeforePreventExtensions();
    expect(record.value).toBe(2);
    expect(record.late).toBe(3);
    expect('late' in record).toBe(true);
    expect(Object.hasOwn(record, 'late')).toBe(true);
    expect(Reflect.ownKeys(record)).toContain('late');
    expect(Object.getOwnPropertyDescriptor(record, 'late')).toMatchObject({
      configurable: true,
      enumerable: true,
      value: 3,
      writable: true,
    });

    Object.preventExtensions(record);
    expect(Object.isExtensible(record)).toBe(false);
    record.mutateAfterPreventExtensions();
    expect(record.value).toBe(4);
    expect(record.late).toBe(5);
    expect(record.afterLock).toBeUndefined();
    expect('afterLock' in record).toBe(false);
    expect(Object.hasOwn(record, 'afterLock')).toBe(false);
    expect(Reflect.ownKeys(record)).not.toContain('afterLock');
    expect(Object.getOwnPropertyDescriptor(record, 'afterLock')).toBeUndefined();

    expect(Reflect.set(record, 'fixed', 2)).toBe(false);
    record.replaceFixed(3);
    expect(record.fixed).toBe(3);

    const sealedRecord = new instance.exports.BindingMutableRecord();
    Object.seal(sealedRecord);
    expect(
      Reflect.defineProperty(sealedRecord, 'value', {
        configurable: true,
        value: 1,
      }),
    ).toBe(false);
    sealedRecord.setValue(7);
    expect(sealedRecord.value).toBe(7);
    expect(Object.getOwnPropertyDescriptor(sealedRecord, 'value')).toMatchObject({
      configurable: false,
      value: 7,
      writable: true,
    });

    const frozenRecord = new instance.exports.BindingMutableRecord();
    Object.freeze(frozenRecord);
    expect(() => frozenRecord.setValue(8)).toThrow(TypeError);
    expect(frozenRecord.value).toBe(1);
    expect(Object.getOwnPropertyDescriptor(frozenRecord, 'value')).toMatchObject({
      configurable: false,
      value: 1,
      writable: false,
    });

    instance.dispose();
    expect(context.destroy).toHaveBeenCalledOnce();
  });

  test('accounts for every close-bearing binding class', async () => {
    class AsyncClosable {
      close(): Promise<void> {
        return Promise.resolve();
      }
    }
    class BindingBundler extends AsyncClosable {}
    class BindingDevEngine extends AsyncClosable {}
    class BindingWatcher extends AsyncClosable {}
    class BindingWatcherBundler extends AsyncClosable {}
    class TraceSubscriberGuard {
      close(): void {}
    }
    const rawBinding = {
      BindingBundler,
      BindingDevEngine,
      BindingWatcher,
      BindingWatcherBundler,
      TraceSubscriberGuard,
      registerCurrentThreadTaskHost() {},
      registerTimerHost() {},
    };
    const context = { suppressDestroy() {}, destroy: vi.fn() };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async () => ({ napiModule: { exports: rawBinding } }),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
    const instance = await loader.createInstance(module, {
      initialMemoryPages: 1,
      maximumMemoryPages: 1,
    });
    const binding = instance.exports;
    const resources = [
      new binding.BindingBundler(),
      new binding.BindingDevEngine(),
      new binding.BindingWatcher(),
      new binding.BindingWatcherBundler(),
      new binding.TraceSubscriberGuard(),
    ];

    expect(() => instance.dispose()).toThrow(/5 open binding objects/);
    await resources[0].close.call(resources[1]);
    await resources[0].close();
    await Promise.all(resources.slice(2).map((resource) => resource.close()));
    for (const resource of resources.slice(2)) {
      await resource.close();
    }
    instance.dispose();
    expect(context.destroy).toHaveBeenCalledOnce();
  });

  test('reads and calls custom thenables once while blocking reentrant disposal', async () => {
    let instance!: DeferredRolldownInstance;
    let getterCalls = 0;
    let thenCalls = 0;
    let disposalFailure: unknown;
    class BindingBundler {
      generate() {
        return {
          // oxlint-disable-next-line unicorn/no-thenable -- exercises one-shot custom thenable assimilation
          get then() {
            getterCalls += 1;
            try {
              instance.dispose();
            } catch (error) {
              disposalFailure = error;
            }
            return (resolve: (value: string) => void) => {
              thenCalls += 1;
              resolve('generated');
            };
          },
        };
      }

      close(): void {}
    }
    const rawBinding = {
      BindingBundler,
      registerCurrentThreadTaskHost() {},
      registerTimerHost() {},
    };
    const context = { suppressDestroy() {}, destroy: vi.fn() };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async () => ({ napiModule: { exports: rawBinding } }),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
    instance = await loader.createInstance(module, {
      initialMemoryPages: 1,
      maximumMemoryPages: 1,
    });
    const bundler = new instance.exports.BindingBundler();

    await expect((bundler as unknown as { generate(): Promise<string> }).generate()).resolves.toBe(
      'generated',
    );
    expect(getterCalls).toBe(1);
    expect(thenCalls).toBe(1);
    expect(disposalFailure).toMatchObject({
      message: expect.stringMatching(/1 active binding operation and 1 open binding object/),
    });

    bundler.close();
    instance.dispose();
  });

  test('returns direct accessor-backed non-thenables after one getter read', async () => {
    const getterReceivers: unknown[] = [];
    const terminal = {
      marker: 'terminal',
      // oxlint-disable-next-line unicorn/no-thenable -- verifies accessor-backed non-thenable handling
      get then() {
        getterReceivers.push(this);
        return undefined;
      },
    };
    class BindingInvoker {
      terminal() {
        return terminal;
      }

      close(): void {}
    }
    const rawBinding = {
      BindingInvoker,
      registerCurrentThreadTaskHost() {},
      registerTimerHost() {},
    };
    const context = { suppressDestroy() {}, destroy: vi.fn() };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async () => ({ napiModule: { exports: rawBinding } }),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
    const instance = await loader.createInstance(module, {
      initialMemoryPages: 1,
      maximumMemoryPages: 1,
    });
    const invoker = new instance.exports.BindingInvoker();

    const result = invoker.terminal();
    expect(result).not.toBeInstanceOf(Promise);
    expect(Object.getOwnPropertyDescriptor(result, 'marker')?.value).toBe('terminal');
    expect(getterReceivers).toEqual([terminal]);

    invoker.close();
    instance.dispose();
    expect(context.destroy).toHaveBeenCalledOnce();
  });

  test('assimilates nested accessor-backed then values', async () => {
    let instance!: DeferredRolldownInstance;
    let terminalGetterCalls = 0;
    let callableGetterCalls = 0;
    let callableThenCalls = 0;
    let disposalFailure: unknown;
    const getterError = new TypeError('nested then getter failed');
    class Terminal {
      #marker = 'terminal';

      // oxlint-disable-next-line unicorn/no-thenable -- verifies accessor-backed non-thenable identity
      get then() {
        terminalGetterCalls += 1;
        return undefined;
      }

      marker(): string {
        return this.#marker;
      }
    }
    const terminal = new Terminal();
    const identities = new WeakMap([[terminal, 'terminal']]);
    // oxlint-disable-next-line unicorn/no-thenable -- verifies accessor-backed thenable assimilation
    const callable = Object.defineProperty({}, 'then', {
      get() {
        callableGetterCalls += 1;
        return (resolve: (value: string) => void) => {
          callableThenCalls += 1;
          try {
            instance.dispose();
          } catch (error) {
            disposalFailure = error;
          }
          resolve('accessor-settled');
        };
      },
    });
    // oxlint-disable-next-line unicorn/no-thenable -- verifies accessor getter rejection identity
    const throwing = Object.defineProperty({}, 'then', {
      get() {
        throw getterError;
      },
    });
    class BindingInvoker {
      terminal() {
        return {
          // oxlint-disable-next-line unicorn/no-thenable -- verifies nested accessor assimilation
          then(resolve: (value: Terminal) => void) {
            resolve(terminal);
          },
        };
      }

      callable() {
        return {
          // oxlint-disable-next-line unicorn/no-thenable -- verifies nested accessor assimilation
          then(resolve: (value: typeof callable) => void) {
            resolve(callable);
          },
        };
      }

      throwing() {
        return {
          // oxlint-disable-next-line unicorn/no-thenable -- verifies nested accessor rejection identity
          then(resolve: (value: typeof throwing) => void) {
            resolve(throwing);
          },
        };
      }

      close(): void {}
    }
    const rawBinding = {
      BindingInvoker,
      registerCurrentThreadTaskHost() {},
      registerTimerHost() {},
    };
    const context = { suppressDestroy() {}, destroy: vi.fn() };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async () => ({ napiModule: { exports: rawBinding } }),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
    instance = await loader.createInstance(module, {
      initialMemoryPages: 1,
      maximumMemoryPages: 1,
    });
    const ManagedBindingInvoker = (
      instance.exports as unknown as {
        BindingInvoker: typeof BindingInvoker;
      }
    ).BindingInvoker;
    const invoker = new ManagedBindingInvoker();

    await expect(invoker.terminal()).resolves.toBe(terminal);
    expect(terminalGetterCalls).toBeGreaterThan(0);
    expect(terminal.marker()).toBe('terminal');
    expect(identities.get(terminal)).toBe('terminal');

    await expect(invoker.callable()).resolves.toBe('accessor-settled');
    expect(callableGetterCalls).toBe(1);
    expect(callableThenCalls).toBe(1);
    expect(disposalFailure).toMatchObject({
      message: expect.stringMatching(/1 active binding operation and 1 open binding object/),
    });

    await expect(invoker.throwing()).rejects.toBe(getterError);

    invoker.close();
    instance.dispose();
    expect(context.destroy).toHaveBeenCalledOnce();
  });

  test('defers custom then invocation for binding results and input callbacks', async () => {
    const events: string[] = [];
    const createThenable = (label: string) => ({
      // oxlint-disable-next-line unicorn/no-thenable -- verifies Promise-compatible timing
      then(resolve: (value: string) => void) {
        events.push(`${label}:then`);
        resolve(label);
      },
    });
    class BindingInvoker {
      returnThenable() {
        events.push('binding:call');
        return createThenable('binding');
      }

      invoke(callback: () => unknown) {
        events.push('input:call');
        return callback();
      }

      close(): void {}
    }
    const rawBinding = {
      BindingInvoker,
      registerCurrentThreadTaskHost() {},
      registerTimerHost() {},
    };
    const context = { suppressDestroy() {}, destroy: vi.fn() };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async () => ({ napiModule: { exports: rawBinding } }),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
    const instance = await loader.createInstance(module, {
      initialMemoryPages: 1,
      maximumMemoryPages: 1,
    });
    const invoker = new instance.exports.BindingInvoker();

    const bindingResult = invoker.returnThenable();
    events.push('binding:after');
    expect(events).toEqual(['binding:call', 'binding:after']);
    await expect(bindingResult).resolves.toBe('binding');
    expect(events).toEqual(['binding:call', 'binding:after', 'binding:then']);

    const inputResult = invoker.invoke(() => {
      events.push('input:callback');
      return createThenable('input');
    });
    events.push('input:after');
    expect(events).toEqual([
      'binding:call',
      'binding:after',
      'binding:then',
      'input:call',
      'input:callback',
      'input:after',
    ]);
    await expect(inputResult).resolves.toBe('input');
    expect(events.at(-1)).toBe('input:then');

    invoker.close();
    instance.dispose();
    expect(context.destroy).toHaveBeenCalledOnce();
  });

  test('permits a custom thenable to remove then before resolving itself', async () => {
    class BindingBundler {
      generate() {
        const result: {
          marker: string;
          then?: (resolve: (value: unknown) => void) => void;
        } = {
          marker: 'settled',
          // oxlint-disable-next-line unicorn/no-thenable -- verifies native mutable-thenable semantics
          then(resolve) {
            delete result.then;
            resolve(result);
          },
        };
        return result;
      }

      close(): void {}
    }
    const rawBinding = {
      BindingBundler,
      registerCurrentThreadTaskHost() {},
      registerTimerHost() {},
    };
    const context = { suppressDestroy() {}, destroy: vi.fn() };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async () => ({ napiModule: { exports: rawBinding } }),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
    const instance = await loader.createInstance(module, {
      initialMemoryPages: 1,
      maximumMemoryPages: 1,
    });
    const bundler = new instance.exports.BindingBundler();

    await expect(
      (bundler as unknown as { generate(): Promise<{ marker: string }> }).generate(),
    ).resolves.toMatchObject({ marker: 'settled' });

    bundler.close();
    instance.dispose();
    expect(context.destroy).toHaveBeenCalledOnce();
  });

  test('reads a self-removing then accessor once and preserves terminal identity', async () => {
    let getterCalls = 0;
    let terminal!: MutableAccessorTerminal;
    class MutableAccessorTerminal {
      #marker = 'settled';

      constructor() {
        // oxlint-disable-next-line unicorn/no-thenable -- verifies one-read mutable accessor semantics
        Object.defineProperty(this, 'then', {
          configurable: true,
          get: () => {
            getterCalls += 1;
            Reflect.deleteProperty(this, 'then');
            return (resolve: (value: MutableAccessorTerminal) => void) => resolve(this);
          },
        });
      }

      marker(): string {
        return this.#marker;
      }
    }
    class BindingBundler {
      generate() {
        terminal = new MutableAccessorTerminal();
        return terminal;
      }

      close(): void {}
    }
    const rawBinding = {
      BindingBundler,
      registerCurrentThreadTaskHost() {},
      registerTimerHost() {},
    };
    const context = { suppressDestroy() {}, destroy: vi.fn() };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async () => ({ napiModule: { exports: rawBinding } }),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
    const instance = await loader.createInstance(module, {
      initialMemoryPages: 1,
      maximumMemoryPages: 1,
    });
    const bundler = new instance.exports.BindingBundler();

    await expect(
      (bundler as unknown as { generate(): Promise<MutableAccessorTerminal> }).generate(),
    ).resolves.toBe(terminal);
    expect(getterCalls).toBe(1);
    expect(terminal.marker()).toBe('settled');

    bundler.close();
    instance.dispose();
    expect(context.destroy).toHaveBeenCalledOnce();
  });

  test('releases a resolved close thenable before its later user microtasks', async () => {
    let instance!: DeferredRolldownInstance;
    let disposalError: unknown;
    class BindingResource {
      close() {
        return {
          // oxlint-disable-next-line unicorn/no-thenable -- verifies resolving-function job order
          then(resolve: () => void) {
            resolve();
            queueMicrotask(() => {
              try {
                instance.dispose();
              } catch (error) {
                disposalError = error;
              }
            });
          },
        };
      }
    }
    const rawBinding = {
      BindingResource,
      registerCurrentThreadTaskHost() {},
      registerTimerHost() {},
    };
    const context = { suppressDestroy() {}, destroy: vi.fn() };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async () => ({ napiModule: { exports: rawBinding } }),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
    instance = await loader.createInstance(module, {
      initialMemoryPages: 1,
      maximumMemoryPages: 1,
    });
    const Resource = (
      instance.exports as unknown as {
        BindingResource: typeof BindingResource;
      }
    ).BindingResource;
    const resource = new Resource();

    await resource.close();
    expect(disposalError).toBeUndefined();
    expect(instance.disposed).toBe(true);
    expect(context.destroy).toHaveBeenCalledOnce();
  });

  test('rejects a managed thenable that resolves to the public returned promise', async () => {
    let publicPromise!: Promise<unknown>;
    class BindingInvoker {
      invoke() {
        return {
          // oxlint-disable-next-line unicorn/no-thenable -- verifies public-promise self-resolution
          then(resolve: (value: unknown) => void) {
            resolve(publicPromise);
          },
        };
      }

      close(): void {}
    }
    const rawBinding = {
      BindingInvoker,
      registerCurrentThreadTaskHost() {},
      registerTimerHost() {},
    };
    const context = { suppressDestroy() {}, destroy: vi.fn() };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async () => ({ napiModule: { exports: rawBinding } }),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
    const instance = await loader.createInstance(module, {
      initialMemoryPages: 1,
      maximumMemoryPages: 1,
    });
    const invoker = new instance.exports.BindingInvoker();

    publicPromise = (invoker as unknown as { invoke(): Promise<unknown> }).invoke();
    await expect(publicPromise).rejects.toThrow(
      /Thenable cycle detected while settling a managed workerd call/,
    );

    invoker.close();
    instance.dispose();
    expect(context.destroy).toHaveBeenCalledOnce();
  });

  test.each([
    {
      name: 'self-resolving',
      setup: `
const cycle = {}
cycle.then = (resolve) => resolve(cycle)
`,
      extraAssertion: '',
    },
    {
      name: 'mutually recursive',
      setup: `
const first = {}
const second = {}
first.then = (resolve) => resolve(second)
second.then = (resolve) => resolve(first)
const cycle = first
`,
      extraAssertion: '',
    },
    {
      name: 'alternating-getter',
      setup: `
let thenReads = 0
const first = {}
const second = {
  then(resolve) {
    resolve(first)
  },
}
Object.defineProperty(first, 'then', {
  get() {
    thenReads += 1
    return thenReads % 2 === 1
      ? (resolve) => resolve(second)
      : (resolve) => resolve(first)
  },
})
const cycle = first
`,
      extraAssertion: 'assert.equal(thenReads, 1)',
    },
  ])(
    'rejects $name callback thenable cycles without hanging',
    { timeout: 10_000 },
    ({ setup, extraAssertion }) => {
      const child = spawnSync(
        process.execPath,
        [
          '--input-type=module',
          '--eval',
          `
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const loaderPath = ${JSON.stringify(fileURLToPath(deferredLoaderPath))}
const source = await readFile(loaderPath, 'utf8')
const dependencyKey = '__rolldownWorkerdThenableCycleTest'
class BindingInvoker {
  constructor(callback) {
    this.callback = callback
  }

  invoke() {
    return this.callback()
  }
}
const __liveHosts = new Set()
let __nextHostRegistration = 1
const rawBinding = {
  BindingInvoker,
  getCurrentThreadTaskHostContractVersion() {
    return 4
  },
  isCurrentThreadHostRegistrationActive(_high, low) {
    return __liveHosts.has(low)
  },
  reserveCurrentThreadHostRegistration() {
    return { high: 0, low: __nextHostRegistration++ }
  },
  registerCurrentThreadTaskHost(_high, low) {
    __liveHosts.add(low)
  },
  unregisterCurrentThreadTaskHost(_high, low) {
    __liveHosts.delete(low)
  },
  registerTimerHost(_high, low) {
    __liveHosts.add(low)
  },
  unregisterTimerHost(_high, low) {
    __liveHosts.delete(low)
  },
}
const context = { feature: {}, suppressDestroy() {}, destroy() {} }
globalThis[dependencyKey] = {
  Buffer,
  createContext: () => context,
  getDefaultContext: () => context,
  instantiateNapiModule: async () => ({ napiModule: { exports: rawBinding } }),
  WASI: class {},
}
const transformed = source
  .replace(
    /import \\{[\\s\\S]*?\\} from '@napi-rs\\/wasm-runtime'\\nimport \\{ createContext as __emnapiCreateContext \\} from '@emnapi\\/runtime'\\n/,
    \`const {
  getDefaultContext: __emnapiGetDefaultContext,
  instantiateNapiModule: __emnapiInstantiateNapiModule,
  WASI: __WASI,
  createContext: __emnapiCreateContext,
  Buffer,
} = globalThis[\${JSON.stringify(dependencyKey)}]\\n\`,
  )
  .replace("import { Buffer } from 'buffer'\\n", '')
const loader = await import(
  \`data:text/javascript;base64,\${Buffer.from(transformed).toString('base64')}\`
)
delete globalThis[dependencyKey]
const module = await WebAssembly.compile(
  new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]),
)
const instance = await loader.createInstance(module, {
  initialMemoryPages: 1,
  maximumMemoryPages: 1,
})
${setup}
const invoker = new instance.exports.BindingInvoker(() => cycle)
await assert.rejects(
  () => invoker.invoke(),
  (error) => {
    assert.equal(error?.name, 'TypeError')
    assert.match(error?.message ?? '', /Thenable cycle detected/)
    return true
  },
)
${extraAssertion}
instance.dispose()
console.log('callback thenable cycle rejected')
`,
        ],
        {
          encoding: 'utf8',
          timeout: 3_000,
        },
      );

      expect(child.error).toBeUndefined();
      expect(child.signal).toBeNull();
      expect(child.status, child.stderr || child.stdout).toBe(0);
      expect(child.stdout).toContain('callback thenable cycle rejected');
    },
  );

  test.each([
    {
      name: 'self-cyclic',
      setup: `
let cycle
cycle = new Proxy({}, {
  getPrototypeOf() {
    return cycle
  },
})
`,
      expectedError: 'Cyclic prototype chain detected',
    },
    {
      name: 'fresh-proxy',
      setup: `
const freshPrototypeHandler = {
  getPrototypeOf() {
    return new Proxy({}, freshPrototypeHandler)
  },
}
const cycle = new Proxy({}, freshPrototypeHandler)
`,
      expectedError: 'prototype chain exceeds the traversal limit',
    },
  ])(
    'rejects $name facade prototype chains without hanging',
    { timeout: 10_000 },
    ({ setup, expectedError }) => {
      const child = spawnSync(
        process.execPath,
        [
          '--input-type=module',
          '--eval',
          `
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const loaderPath = ${JSON.stringify(fileURLToPath(deferredLoaderPath))}
const source = await readFile(loaderPath, 'utf8')
const dependencyKey = '__rolldownWorkerdPrototypeCycleTest'
class BindingInvoker {
  accept(_value) {}

  returnCycle() {
    return cycle
  }

  invoke(callback) {
    return callback(cycle)
  }
}
const __liveHosts = new Set()
let __nextHostRegistration = 1
const rawBinding = {
  BindingInvoker,
  getCurrentThreadTaskHostContractVersion() {
    return 4
  },
  isCurrentThreadHostRegistrationActive(_high, low) {
    return __liveHosts.has(low)
  },
  reserveCurrentThreadHostRegistration() {
    return { high: 0, low: __nextHostRegistration++ }
  },
  registerCurrentThreadTaskHost(_high, low) {
    __liveHosts.add(low)
  },
  unregisterCurrentThreadTaskHost(_high, low) {
    __liveHosts.delete(low)
  },
  registerTimerHost(_high, low) {
    __liveHosts.add(low)
  },
  unregisterTimerHost(_high, low) {
    __liveHosts.delete(low)
  },
}
const context = { feature: {}, suppressDestroy() {}, destroy() {} }
globalThis[dependencyKey] = {
  Buffer,
  createContext: () => context,
  getDefaultContext: () => context,
  instantiateNapiModule: async () => ({ napiModule: { exports: rawBinding } }),
  WASI: class {},
}
const transformed = source
  .replace(
    /import \\{[\\s\\S]*?\\} from '@napi-rs\\/wasm-runtime'\\nimport \\{ createContext as __emnapiCreateContext \\} from '@emnapi\\/runtime'\\n/,
    \`const {
  getDefaultContext: __emnapiGetDefaultContext,
  instantiateNapiModule: __emnapiInstantiateNapiModule,
  WASI: __WASI,
  createContext: __emnapiCreateContext,
  Buffer,
} = globalThis[\${JSON.stringify(dependencyKey)}]\\n\`,
  )
  .replace("import { Buffer } from 'buffer'\\n", '')
const loader = await import(
  \`data:text/javascript;base64,\${Buffer.from(transformed).toString('base64')}\`
)
delete globalThis[dependencyKey]
const module = await WebAssembly.compile(
  new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]),
)
const instance = await loader.createInstance(module, {
  initialMemoryPages: 1,
  maximumMemoryPages: 1,
})
${setup}
const expectedPrototypeError = new RegExp(${JSON.stringify(expectedError)})
const invoker = new instance.exports.BindingInvoker()
assert.throws(
  () => invoker.accept(cycle),
  expectedPrototypeError,
)
await assert.rejects(
  () => invoker.returnCycle(),
  expectedPrototypeError,
)
assert.throws(
  () => invoker.invoke(() => {}),
  expectedPrototypeError,
)
instance.dispose()
console.log('proxy prototype chain rejected')
`,
        ],
        {
          encoding: 'utf8',
          timeout: 3_000,
        },
      );

      expect(child.error).toBeUndefined();
      expect(child.signal).toBeNull();
      expect(child.status, child.stderr || child.stdout).toBe(0);
      expect(child.stdout).toContain('proxy prototype chain rejected');
    },
  );

  test('passes native Buffer and foreign or subclassed views to the raw binding unchanged', async () => {
    const require = createRequire(import.meta.url);
    const { Buffer: EmbeddedBuffer } = require('buffer/') as {
      Buffer: typeof NodeBuffer;
    };
    const nativeBuffer = NodeBuffer.from([0, 1, 255]);
    const foreignView = runInNewContext('new Uint16Array([257, 65535])') as Uint16Array;
    class DerivedUint8Array extends Uint8Array {}
    const derivedView = new DerivedUint8Array([2, 3, 4]);
    const views = [nativeBuffer, foreignView, derivedView];
    let receivedViews: unknown[] | undefined;
    class BindingInvoker {
      accept(input: { views: unknown[] }): boolean[] {
        receivedViews = input.views;
        return input.views.map((value) => ArrayBuffer.isView(value));
      }
    }
    const rawBinding = {
      BindingInvoker,
      registerCurrentThreadTaskHost() {},
      registerTimerHost() {},
    };
    const context = { suppressDestroy() {}, destroy: vi.fn() };
    const loader = await loadDeferredLoaderWithDependencies({
      Buffer: EmbeddedBuffer,
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async () => ({ napiModule: { exports: rawBinding } }),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
    const instance = await loader.createInstance(module, {
      initialMemoryPages: 1,
      maximumMemoryPages: 1,
    });

    try {
      expect(EmbeddedBuffer.prototype).not.toBe(NodeBuffer.prototype);
      expect(new instance.exports.BindingInvoker().accept({ views })).toEqual([true, true, true]);
      expect(receivedViews).not.toBe(views);
      expect(receivedViews).toHaveLength(views.length);
      for (const [index, view] of views.entries()) {
        expect(receivedViews?.[index]).toBe(view);
        expect(ArrayBuffer.isView(receivedViews?.[index])).toBe(true);
      }
    } finally {
      instance.dispose();
    }
    expect(context.destroy).toHaveBeenCalledOnce();
  });

  test('passes foreign and subclassed ArrayBuffers to the raw binding unchanged', async () => {
    // oxlint-disable-next-line typescript/unbound-method -- invoked with candidate buffers through Reflect.apply
    const arrayBufferByteLength = Object.getOwnPropertyDescriptor(
      ArrayBuffer.prototype,
      'byteLength',
    )!.get!;
    // oxlint-disable-next-line typescript/unbound-method -- invoked with candidate buffers through Reflect.apply
    const sharedArrayBufferByteLength = Object.getOwnPropertyDescriptor(
      SharedArrayBuffer.prototype,
      'byteLength',
    )!.get!;
    const readByteLength = (value: unknown): number => {
      try {
        return Reflect.apply(arrayBufferByteLength, value, []);
      } catch {
        return Reflect.apply(sharedArrayBufferByteLength, value, []);
      }
    };
    class DerivedArrayBuffer extends ArrayBuffer {}
    class DerivedSharedArrayBuffer extends SharedArrayBuffer {}
    const buffers = [
      new ArrayBuffer(1),
      runInNewContext('new ArrayBuffer(2)') as ArrayBuffer,
      new DerivedArrayBuffer(3),
      new SharedArrayBuffer(4),
      runInNewContext('new SharedArrayBuffer(5)') as SharedArrayBuffer,
      new DerivedSharedArrayBuffer(6),
    ];
    let receivedBuffers: unknown[] | undefined;
    class BindingInvoker {
      accept(input: { buffers: unknown[] }): number[] {
        receivedBuffers = input.buffers;
        return input.buffers.map(readByteLength);
      }
    }
    const rawBinding = {
      BindingInvoker,
      registerCurrentThreadTaskHost() {},
      registerTimerHost() {},
    };
    const context = { suppressDestroy() {}, destroy: vi.fn() };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async () => ({ napiModule: { exports: rawBinding } }),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
    const instance = await loader.createInstance(module, {
      initialMemoryPages: 1,
      maximumMemoryPages: 1,
    });

    try {
      expect(new instance.exports.BindingInvoker().accept({ buffers })).toEqual([1, 2, 3, 4, 5, 6]);
      expect(receivedBuffers).not.toBe(buffers);
      expect(receivedBuffers).toHaveLength(buffers.length);
      for (const [index, buffer] of buffers.entries()) {
        expect(receivedBuffers?.[index]).toBe(buffer);
        expect(readByteLength(receivedBuffers?.[index])).toBe(index + 1);
      }
    } finally {
      instance.dispose();
    }
    expect(context.destroy).toHaveBeenCalledOnce();
  });

  test('mediates callback-delivered binding objects for intrinsic subclasses', async () => {
    let retainedContext: BindingContext | undefined;
    class BindingContext {
      getModuleIds(): string[] {
        return ['virtual:entry'];
      }
    }
    class BindingInvoker {
      invoke(plugin: { receiveContext(context: BindingContext): void }): void {
        plugin.receiveContext(new BindingContext());
      }
    }
    class DatePlugin extends Date {
      observedTime: number | undefined;

      receiveContext(context: BindingContext): void {
        this.observedTime = this.getTime();
        retainedContext = context;
      }
    }
    const rawBinding = {
      BindingContext,
      BindingInvoker,
      registerCurrentThreadTaskHost() {},
      registerTimerHost() {},
    };
    const context = { suppressDestroy() {}, destroy: vi.fn() };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async () => ({ napiModule: { exports: rawBinding } }),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
    const instance = await loader.createInstance(module, {
      initialMemoryPages: 1,
      maximumMemoryPages: 1,
    });
    const plugin = new DatePlugin(123);
    const invoker = new instance.exports.BindingInvoker();

    invoker.invoke(plugin);
    expect(plugin.observedTime).toBe(123);
    expect(retainedContext?.getModuleIds()).toEqual(['virtual:entry']);

    instance.dispose();
    expect(context.destroy).toHaveBeenCalledOnce();
    expect(() => retainedContext?.getModuleIds()).toThrow(
      'This workerd Rolldown instance has been disposed',
    );
  });

  test('mediates constructors, nested results, and callback-delivered binding objects', async () => {
    let retainedWatcherEvent: InstanceType<typeof BindingWatcherEvent> | undefined;
    let retainedWatcherBundler: InstanceType<typeof BindingWatcherBundler> | undefined;
    let rawChunkCalls = 0;
    class BindingOutputChunk {
      getCode(): string {
        rawChunkCalls += 1;
        return 'export default 1';
      }
    }
    class BindingBundler {
      generate(): Promise<{ chunks: BindingOutputChunk[] }> {
        return Promise.resolve({ chunks: [new BindingOutputChunk()] });
      }

      close(): void {}
    }
    class BindingWatcherBundler {
      close(): void {}
    }
    class BindingWatcherEvent {
      get result(): BindingWatcherBundler {
        return new BindingWatcherBundler();
      }
    }
    class BindingWatcher {
      constructor(
        _options: unknown[],
        private readonly listener: (event: BindingWatcherEvent) => void,
      ) {}

      run(): Promise<void> {
        this.listener(new BindingWatcherEvent());
        return Promise.resolve();
      }

      close(): void {}
    }
    const rawBinding = {
      BindingBundler,
      BindingOutputChunk,
      BindingWatcher,
      BindingWatcherBundler,
      BindingWatcherEvent,
      registerCurrentThreadTaskHost() {},
      registerTimerHost() {},
    };
    const context = { suppressDestroy() {}, destroy: vi.fn() };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async () => ({ napiModule: { exports: rawBinding } }),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
    const instance = await loader.createInstance(module, {
      initialMemoryPages: 1,
      maximumMemoryPages: 1,
    });
    const binding = instance.exports;
    const Bundler = binding.BindingBundler;
    const BoundBundler = Bundler.bind(undefined);
    const bundler = new BoundBundler();
    expect(bundler.constructor).toBe(Bundler);
    expect(Object.getPrototypeOf(bundler)).toBe(Bundler.prototype);
    expect(bundler).toBeInstanceOf(Bundler);

    const output = await bundler.generate();
    const chunk = output.chunks[0];
    const retainedGetCode = chunk.getCode;
    expect(chunk.constructor).toBe(binding.BindingOutputChunk);
    expect(chunk.getCode()).toBe('export default 1');

    const watcher = new binding.BindingWatcher(
      [],
      (event: InstanceType<typeof BindingWatcherEvent>) => {
        retainedWatcherEvent = event;
        retainedWatcherBundler = event.result;
      },
    );
    await watcher.run();
    expect(retainedWatcherEvent?.constructor).toBe(binding.BindingWatcherEvent);
    expect(retainedWatcherBundler?.constructor).toBe(binding.BindingWatcherBundler);
    expect(() => instance.dispose()).toThrow(/3 open binding objects/);

    bundler.close();
    watcher.close();
    retainedWatcherBundler?.close();
    instance.dispose();
    expect(rawChunkCalls).toBe(1);
    expect(() => chunk.getCode()).toThrow('This workerd Rolldown instance has been disposed');
    expect(() => retainedGetCode()).toThrow('This workerd Rolldown instance has been disposed');
    expect(() => retainedWatcherEvent?.result).toThrow(
      'This workerd Rolldown instance has been disposed',
    );
    expect(() => new BoundBundler()).toThrow('This workerd Rolldown instance has been disposed');
  });

  wasiTest(
    'uses a replaced writable production constructor prototype for new instances',
    { timeout: 30_000 },
    async () => {
      const module = await WebAssembly.compile(await readFile(wasmPath));
      const instance = await createInstance(module);
      const MagicString = instance.exports.BindingMagicString;
      const replacementPrototype = { marker: true } as unknown as typeof MagicString.prototype;

      expect(Object.getOwnPropertyDescriptor(MagicString, 'prototype')).toMatchObject({
        configurable: false,
        writable: true,
      });
      MagicString.prototype = replacementPrototype;
      const magicString = new MagicString('export default 1');

      expect(MagicString.prototype).toBe(replacementPrototype);
      expect(Object.getPrototypeOf(magicString) === replacementPrototype).toBe(true);
      expect(magicString instanceof MagicString).toBe(true);

      instance.dispose();
    },
  );

  test(
    'allows dropped binding objects and repeated closed objects to be collected',
    { timeout: 30_000 },
    () => {
      const child = spawnSync(
        process.execPath,
        [
          '--expose-gc',
          '--input-type=module',
          '--eval',
          `
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const loaderPath = ${JSON.stringify(fileURLToPath(deferredLoaderPath))}
const source = await readFile(loaderPath, 'utf8')
const dependencyKey = '__rolldownWorkerdGcTest'
class BindingBundler {
  close() {}
}
let nextRegistration = 1
const createRawBinding = () => {
  const liveHosts = new Set()
  return {
    BindingBundler,
    getCurrentThreadTaskHostContractVersion() {
      return 4
    },
    isCurrentThreadHostRegistrationActive(_high, low) {
      return liveHosts.has(low)
    },
    reserveCurrentThreadHostRegistration() {
      return { high: 0, low: nextRegistration++ }
    },
    registerCurrentThreadTaskHost(_high, low) {
      liveHosts.add(low)
    },
    unregisterCurrentThreadTaskHost(_high, low) {
      liveHosts.delete(low)
    },
    registerTimerHost(_high, low) {
      liveHosts.add(low)
    },
    unregisterTimerHost(_high, low) {
      liveHosts.delete(low)
    },
  }
}
const context = { feature: {}, suppressDestroy() {}, destroy() {} }
globalThis[dependencyKey] = {
  Buffer,
  createContext: () => context,
  getDefaultContext: () => context,
  instantiateNapiModule: async () => ({
    napiModule: { exports: createRawBinding() },
  }),
  WASI: class {},
}
const transformed = source
  .replace(
    /import \\{[\\s\\S]*?\\} from '@napi-rs\\/wasm-runtime'\\nimport \\{ createContext as __emnapiCreateContext \\} from '@emnapi\\/runtime'\\n/,
    \`const {
  getDefaultContext: __emnapiGetDefaultContext,
  instantiateNapiModule: __emnapiInstantiateNapiModule,
  WASI: __WASI,
  createContext: __emnapiCreateContext,
  Buffer,
} = globalThis[\${JSON.stringify(dependencyKey)}]\\n\`,
  )
  .replace("import { Buffer } from 'buffer'\\n", '')
const loader = await import(
  \`data:text/javascript;base64,\${Buffer.from(transformed).toString('base64')}\`
)
delete globalThis[dependencyKey]
const module = await WebAssembly.compile(
  new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]),
)
const instance = await loader.createInstance(module, {
  initialMemoryPages: 1,
  maximumMemoryPages: 1,
})
let dropped = new instance.exports.BindingBundler()
const droppedRef = new WeakRef(dropped)
dropped = undefined
let droppedCollected = false
for (let attempt = 0; attempt < 100; attempt += 1) {
  globalThis.gc()
  await new Promise(setImmediate)
  if (droppedRef.deref() === undefined) {
    droppedCollected = true
    break
  }
  await new Promise(setImmediate)
}
assert.equal(droppedCollected, true)
for (let attempt = 0; attempt < 100 && !instance.disposed; attempt += 1) {
  globalThis.gc()
  await new Promise(setImmediate)
  try {
    instance.dispose()
  } catch (error) {
    assert.match(error.message, /open binding object/)
  }
}
assert.equal(instance.disposed, true)

const second = await loader.createInstance(module, {
  initialMemoryPages: 1,
  maximumMemoryPages: 1,
})
const refs = []
for (let index = 0; index < 256; index += 1) {
  let resource = new second.exports.BindingBundler()
  refs.push(new WeakRef(resource))
  resource.close()
  resource = undefined
}
for (let attempt = 0; attempt < 100; attempt += 1) {
  globalThis.gc()
  await new Promise(setImmediate)
}
assert.equal(
  refs.filter((ref) => ref.deref() !== undefined).length,
  0,
  'closed binding wrappers remained strongly retained',
)
second.dispose()
console.log('managed binding wrappers collected')
`,
        ],
        {
          encoding: 'utf8',
          timeout: 20_000,
        },
      );

      expect(child.error).toBeUndefined();
      expect(child.signal).toBeNull();
      expect(child.status, child.stderr || child.stdout).toBe(0);
      expect(child.stdout).toContain('managed binding wrappers collected');
    },
  );

  test('reports managed instance counts per evaluated loader module', async () => {
    const createLoader = () => {
      const context = {
        suppressDestroy() {},
        destroy() {},
      };
      return loadDeferredLoaderWithDependencies({
        createContext: () => context,
        getDefaultContext: () => context,
        instantiateNapiModule: async (
          _module: WebAssembly.Module,
          options: { beforeInit: (input: { instance: { exports: object } }) => void },
        ) => {
          options.beforeInit({ instance: { exports: {} } });
          return {
            napiModule: {
              exports: {
                registerCurrentThreadTaskHost() {},
                registerTimerHost() {},
              },
            },
          };
        },
        WASI: class {},
      });
    };
    const [firstLoader, secondLoader] = await Promise.all([createLoader(), createLoader()]);
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
    const first = await firstLoader.createInstance(module, {
      initialMemoryPages: 1,
      maximumMemoryPages: 1,
    });
    const second = await secondLoader.createInstance(module, {
      initialMemoryPages: 1,
      maximumMemoryPages: 1,
    });

    expect(firstLoader.getDeferredRuntimeStats()).toMatchObject({
      createdInstances: 1,
      liveInstances: 1,
    });
    expect(secondLoader.getDeferredRuntimeStats()).toMatchObject({
      createdInstances: 1,
      liveInstances: 1,
    });

    first.dispose();
    expect(firstLoader.getDeferredRuntimeStats().liveInstances).toBe(0);
    expect(secondLoader.getDeferredRuntimeStats().liveInstances).toBe(1);
    second.dispose();
  });

  wasiTest(
    'lets an active build settle before requiring its bundler to close for disposal',
    { timeout: 30_000 },
    async () => {
      const module = await WebAssembly.compile(await readFile(wasmPath));
      const instance = await createInstance(module);
      const bundler = new instance.exports.BindingBundler();
      let enterLoad!: () => void;
      let releaseLoad!: () => void;
      const loadEntered = new Promise<void>((resolve) => {
        enterLoad = resolve;
      });
      const loadGate = new Promise<void>((resolve) => {
        releaseLoad = resolve;
      });
      const generation = bundler.generate({
        inputOptions: {
          input: [{ import: 'virtual:entry' }],
          plugins: [
            {
              name: 'workerd-dispose-active-build',
              hookUsage: 11,
              resolveId(_ctx, id) {
                if (id === 'virtual:entry') return { id };
              },
              async load(_ctx, id) {
                if (id !== 'virtual:entry') return;
                enterLoad();
                await loadGate;
                return { code: 'export default 1' };
              },
            },
          ],
          cwd: '/',
          logLevel: 0,
          onLog() {},
        },
        outputOptions: { format: 'es', plugins: [] },
      });

      await loadEntered;
      expect(() => instance.dispose()).toThrow(
        /2 active binding operations and 1 open binding object/,
      );
      expect(instance.disposed).toBe(false);

      releaseLoad();
      await expect(generation).resolves.not.toHaveProperty('isBindingErrors', true);
      expect(() => instance.dispose()).toThrow(/1 open binding object/);

      await bundler.close();
      instance.dispose();
      expect(instance.disposed).toBe(true);
    },
  );

  wasiTest.each([
    {
      name: 'own then',
      pluginSource: `({
  name: 'workerd-own-then-plugin',
  hookUsage: 11,
  then() {},
  resolveId(ctx, id) {
    retainedContext = ctx
    if (id === 'virtual:entry') return { id }
  },
  load(_ctx, id) {
    if (id === 'virtual:entry') return { code: 'export default 1' }
  },
})`,
    },
    {
      name: 'inherited then',
      pluginSource: `new (class ThenablePlugin {
  name = 'workerd-inherited-then-plugin'
  hookUsage = 11

  then() {}

  resolveId(ctx, id) {
    retainedContext = ctx
    if (id === 'virtual:entry') return { id }
  }

  load(_ctx, id) {
    if (id === 'virtual:entry') return { code: 'export default 1' }
  }
})()`,
    },
    {
      name: 'Symbol.toStringTag',
      pluginSource: `new (class TaggedPlugin {
  name = 'workerd-tagged-plugin'
  hookUsage = 11

  get [Symbol.toStringTag]() {
    return 'RolldownPlugin'
  }

  resolveId(ctx, id) {
    retainedContext = ctx
    if (id === 'virtual:entry') return { id }
  }

  load(_ctx, id) {
    if (id === 'virtual:entry') return { code: 'export default 1' }
  }
})()`,
    },
  ])(
    'mediates $name input records after real workerd disposal',
    { timeout: 30_000 },
    ({ pluginSource }) => {
      const tsxLoader = createRequire(import.meta.url).resolve('tsx');
      const workerdUrl = new URL('../src/workerd.ts', import.meta.url).href;
      const child = spawnSync(
        process.execPath,
        [
          '--import',
          tsxLoader,
          '--input-type=module',
          '--eval',
          `
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const { createInstance } = await import(${JSON.stringify(workerdUrl)})
const module = await WebAssembly.compile(await readFile(${JSON.stringify(fileURLToPath(wasmPath))}))
const instance = await createInstance(module)
const bundler = new instance.exports.BindingBundler()
let retainedContext
const plugin = ${pluginSource}
const output = await bundler.generate({
  inputOptions: {
    input: [{ import: 'virtual:entry' }],
    plugins: [plugin],
    cwd: '/',
    logLevel: 0,
    onLog() {},
  },
  outputOptions: { format: 'es', plugins: [] },
})
assert.notEqual(output.isBindingErrors, true)
assert.ok(retainedContext)
await bundler.close()
instance.dispose()
assert.throws(
  () => retainedContext.getModuleIds(),
  /This workerd Rolldown instance has been disposed/,
)
console.log('input record context invalidated')
`,
        ],
        {
          encoding: 'utf8',
          timeout: 20_000,
        },
      );

      expect(child.error).toBeUndefined();
      expect(child.signal).toBeNull();
      expect(child.status, child.stderr || child.stdout).toBe(0);
      expect(child.stdout).toContain('input record context invalidated');
    },
  );

  wasiTest(
    'mediates inherited class plugin hooks after real workerd disposal',
    { timeout: 30_000 },
    () => {
      const tsxLoader = createRequire(import.meta.url).resolve('tsx');
      const workerdUrl = new URL('../src/workerd.ts', import.meta.url).href;
      const child = spawnSync(
        process.execPath,
        [
          '--import',
          tsxLoader,
          '--input-type=module',
          '--eval',
          `
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const { createInstance } = await import(${JSON.stringify(workerdUrl)})
const module = await WebAssembly.compile(await readFile(${JSON.stringify(fileURLToPath(wasmPath))}))
const instance = await createInstance(module)
const bundler = new instance.exports.BindingBundler()
let retainedContext
class ClassPlugin {
  #hookCalls = 0
  name = 'workerd-class-plugin'
  hookUsage = 11

  get hookCalls() {
    return this.#hookCalls
  }

  resolveId(ctx, id) {
    this.#hookCalls += 1
    retainedContext = ctx
    if (id === 'virtual:entry') return { id }
  }

  load(_ctx, id) {
    this.#hookCalls += 1
    if (id === 'virtual:entry') return { code: 'export default 1' }
  }
}
const plugin = new ClassPlugin()
const output = await bundler.generate({
  inputOptions: {
    input: [{ import: 'virtual:entry' }],
    plugins: [plugin],
    cwd: '/',
    logLevel: 0,
    onLog() {},
  },
  outputOptions: { format: 'es', plugins: [] },
})
assert.notEqual(output.isBindingErrors, true)
assert.ok(retainedContext)
assert.equal(plugin.hookCalls, 2)
await bundler.close()
instance.dispose()
assert.throws(
  () => retainedContext.getModuleIds(),
  /This workerd Rolldown instance has been disposed/,
)
console.log('class plugin context invalidated')
`,
        ],
        {
          encoding: 'utf8',
          timeout: 20_000,
        },
      );

      expect(child.error).toBeUndefined();
      expect(child.signal).toBeNull();
      expect(child.status, child.stderr || child.stdout).toBe(0);
      expect(child.stdout).toContain('class plugin context invalidated');
    },
  );

  wasiTest(
    'keeps the measured 64 MiB initial floor through repeated representative builds',
    { timeout: 60_000 },
    async () => {
      expect(WORKERD_WASM_MEMORY).toMatchObject({
        initialPages: 1024,
        initialBytes: 64 * 1024 * 1024,
      });

      const module = await WebAssembly.compile(await readFile(wasmPath));
      const moduleCount = 256;
      for (let round = 0; round < 3; round += 1) {
        const instance = await createInstance(module);
        expect(instance.memoryBytes).toBeGreaterThanOrEqual(64 * 1024 * 1024);
        expect(instance.memoryBytes).toBeLessThanOrEqual(65 * 1024 * 1024);
        const bundler = new instance.exports.BindingBundler();
        try {
          const result = await bundler.generate({
            inputOptions: {
              input: [{ import: 'virtual:0' }],
              plugins: [
                {
                  name: 'workerd-memory-floor',
                  hookUsage: 11,
                  resolveId(_ctx, id) {
                    if (id.startsWith('virtual:')) return { id };
                  },
                  load(_ctx, id) {
                    if (!id.startsWith('virtual:')) return;
                    const index = Number(id.slice('virtual:'.length));
                    return index + 1 < moduleCount
                      ? {
                          code: `import value from 'virtual:${index + 1}'; export default value + ${index};`,
                        }
                      : { code: 'export default 1' };
                  },
                },
              ],
              cwd: '/',
              logLevel: 0,
              onLog() {},
            },
            outputOptions: { format: 'es', plugins: [] },
          });
          if ('isBindingErrors' in result) {
            throw new Error(JSON.stringify(result.errors));
          }
          expect(result.chunks.length + result.assets.length).toBe(1);
          expect(instance.memoryBytes).toBeLessThanOrEqual(128 * 1024 * 1024);
        } finally {
          try {
            await bundler.close();
          } finally {
            instance.dispose();
          }
        }
        expect(instance.disposed).toBe(true);
      }
    },
  );

  wasiTest('keeps the 64 MiB floor above the Wasm env.memory import minimum', async () => {
    const module = await WebAssembly.compile(await readFile(wasmPath));
    const imports = WebAssembly.Module.imports(module);
    const importObject: WebAssembly.Imports = {};
    for (const descriptor of imports) {
      const namespace = (importObject[descriptor.module] ??= {});
      if (descriptor.kind === 'function') {
        namespace[descriptor.name] = () => 0;
      }
    }
    const instantiateWithPages = (initial: number) => {
      (importObject.env ??= {}).memory = new WebAssembly.Memory({
        initial,
        maximum: WORKERD_WASM_MEMORY.maximumPages,
      });
      return new WebAssembly.Instance(module, importObject);
    };

    let belowMinimumError: unknown;
    try {
      instantiateWithPages(1);
    } catch (error) {
      belowMinimumError = error;
    }
    const minimumMatch = String(belowMinimumError).match(
      /smaller than the declared initial of (\d+)/,
    );
    expect(minimumMatch).not.toBeNull();
    const importedMinimum = Number(minimumMatch![1]);

    expect(() => instantiateWithPages(importedMinimum)).not.toThrow();
    expect(WORKERD_WASM_MEMORY.initialPages).toBeGreaterThan(importedMinimum);
  });

  test('rejects inputs that would require dynamic Wasm compilation', async () => {
    const beforeStats = getWorkerdRuntimeStats();
    const beforeListeners = process.rawListeners('beforeExit').length;

    await expect(
      instantiate(new Uint8Array([0, 97, 115, 109]) as unknown as WebAssembly.Module),
    ).rejects.toThrow(/precompiled WebAssembly\.Module/);

    expect(getWorkerdRuntimeStats()).toEqual(beforeStats);
    expect(process.rawListeners('beforeExit')).toHaveLength(beforeListeners);
  });

  wasiTest('rejects shared memory from the current or another JavaScript realm', async () => {
    const module = await WebAssembly.compile(await readFile(wasmPath));
    const sharedMemory = new WebAssembly.Memory({
      initial: 1,
      maximum: 1,
      shared: true,
    });
    const crossRealmSharedMemory = runInNewContext(
      'new WebAssembly.Memory({ initial: 1, maximum: 1, shared: true })',
    ) as WebAssembly.Memory;

    expect(crossRealmSharedMemory.buffer).not.toBeInstanceOf(SharedArrayBuffer);
    await expect(instantiate(module, { memory: sharedMemory })).rejects.toThrow(
      /requires an unshared WebAssembly\.Memory/,
    );
    await expect(instantiate(module, { memory: crossRealmSharedMemory })).rejects.toThrow(
      /requires an unshared WebAssembly\.Memory/,
    );
  });

  wasiTest(
    'uses an intrinsic memory-buffer brand check instead of Symbol.toStringTag',
    async () => {
      const module = await WebAssembly.compile(await readFile(wasmPath));
      const unsharedMemory = new WebAssembly.Memory({
        initial: WORKERD_WASM_MEMORY.initialPages,
        maximum: WORKERD_WASM_MEMORY.maximumPages,
      });
      const sharedMemory = new WebAssembly.Memory({
        initial: 1,
        maximum: 1,
        shared: true,
      });
      Object.defineProperty(unsharedMemory.buffer, Symbol.toStringTag, {
        configurable: true,
        value: 'SharedArrayBuffer',
      });
      const sharedBufferPrototype = Object.getPrototypeOf(sharedMemory.buffer) as object;
      const originalSharedTag = Object.getOwnPropertyDescriptor(
        sharedBufferPrototype,
        Symbol.toStringTag,
      );
      Object.defineProperty(sharedBufferPrototype, Symbol.toStringTag, {
        configurable: true,
        value: 'ArrayBuffer',
      });

      try {
        const instance = await instantiate(module, { memory: unsharedMemory });
        instance.dispose();
        await expect(instantiate(module, { memory: sharedMemory })).rejects.toThrow(
          /requires an unshared WebAssembly\.Memory/,
        );
      } finally {
        if (originalSharedTag) {
          Object.defineProperty(sharedBufferPrototype, Symbol.toStringTag, originalSharedTag);
        } else {
          Reflect.deleteProperty(sharedBufferPrototype, Symbol.toStringTag);
        }
      }
    },
  );

  wasiTest('rejects concurrent and sequential reuse of caller-provided memory', async () => {
    const module = await WebAssembly.compile(await readFile(wasmPath));
    const memory = new WebAssembly.Memory({
      initial: WORKERD_WASM_MEMORY.initialPages,
      maximum: WORKERD_WASM_MEMORY.maximumPages,
    });

    const concurrent = await Promise.allSettled([
      instantiate(module, { memory }),
      instantiate(module, { memory }),
    ]);
    const fulfilled = concurrent.filter(
      (result): result is PromiseFulfilledResult<Awaited<ReturnType<typeof instantiate>>> =>
        result.status === 'fulfilled',
    );
    const rejected = concurrent.filter(
      (result): result is PromiseRejectedResult => result.status === 'rejected',
    );

    expect(fulfilled).toHaveLength(1);
    expect(rejected).toHaveLength(1);
    expect(rejected[0].reason).toMatchObject({
      message: expect.stringMatching(/initialization attempt/),
    });

    fulfilled[0].value.dispose();
    await expect(instantiate(module, { memory })).rejects.toThrow(/initialization attempt/);
  });

  test('coordinates memory claims across JavaScript realms', async () => {
    const source = await readFile(deferredLoaderPath, 'utf8');
    const claimSource = source
      .slice(
        source.indexOf('const __managedMemoryClaimsKey'),
        source.indexOf('function __attachCleanupError'),
      )
      .replaceAll('export ', '');
    const createClaim = () =>
      runInNewContext(`${claimSource}\n__claimManagedMemoryForAttempt`) as (
        memory: WebAssembly.Memory,
      ) => void;
    const memory = new WebAssembly.Memory({ initial: 1, maximum: 1 });

    createClaim()(memory);
    expect(() => createClaim()(memory)).toThrow(/initialization attempt/);
  });

  test('does not expose mutable managed memory claim membership', async () => {
    const source = await readFile(deferredLoaderPath, 'utf8');
    const claimSource = source
      .slice(
        source.indexOf('const __managedMemoryClaimsKey'),
        source.indexOf('function __attachCleanupError'),
      )
      .replaceAll('export ', '');
    const claim = runInNewContext(`${claimSource}\n__claimManagedMemoryForAttempt`) as (
      memory: WebAssembly.Memory,
    ) => void;
    const memory = new WebAssembly.Memory({ initial: 1, maximum: 1 });
    const claimKey = Symbol.for('@rolldown/browser/workerd/managed-memory-claims/v1');

    claim(memory);
    const exposedClaimState = Object.getOwnPropertyDescriptor(memory, claimKey)?.value;
    expect(exposedClaimState).toBeDefined();
    expect(() => {
      try {
        WeakSet.prototype.delete.call(exposedClaimState, memory);
      } catch {}
      claim(memory);
    }).toThrow(/initialization attempt/);
  });

  test.each([
    { name: 'extensible', preventExtensions: false },
    { name: 'non-extensible', preventExtensions: true },
  ])('rejects a non-monotonic forged claim on $name memory', async ({ preventExtensions }) => {
    const source = await readFile(deferredLoaderPath, 'utf8');
    const claimSource = source
      .slice(
        source.indexOf('const __managedMemoryClaimsKey'),
        source.indexOf('function __attachCleanupError'),
      )
      .replaceAll('export ', '');
    const claim = runInNewContext(`${claimSource}\n__claimManagedMemoryForAttempt`) as (
      memory: WebAssembly.Memory,
    ) => void;
    const memory = new WebAssembly.Memory({ initial: 1, maximum: 1 });
    const claimKey = Symbol.for('@rolldown/browser/workerd/managed-memory-claims/v1');
    let claimHost: object = memory;
    if (preventExtensions) {
      claimHost = Object.create(Object.getPrototypeOf(memory));
      Object.setPrototypeOf(memory, claimHost);
      Object.preventExtensions(memory);
    }
    Object.defineProperty(claimHost, claimKey, {
      value: () => true,
      configurable: false,
      enumerable: false,
      writable: false,
    });

    expect(() => claim(memory)).toThrow(/claim registry is incompatible/);
  });

  test('anchors memory claims independently of stateful prototype proxies', async () => {
    const source = await readFile(deferredLoaderPath, 'utf8');
    const claimSource = source
      .slice(
        source.indexOf('const __managedMemoryClaimsKey'),
        source.indexOf('function __attachCleanupError'),
      )
      .replaceAll('export ', '');
    const claim = runInNewContext(`${claimSource}\n__claimManagedMemoryForAttempt`) as (
      memory: WebAssembly.Memory,
    ) => void;
    const memory = new WebAssembly.Memory({ initial: 1, maximum: 1 });
    const nativePrototype = Object.getPrototypeOf(memory);
    // oxlint-disable-next-line typescript/unbound-method -- the accessor is deliberately rebound to the test memory
    const nativeBufferGetter = Object.getOwnPropertyDescriptor(nativePrototype, 'buffer')!.get!;
    const visitedPrototypeHosts = new Set<object>();
    const statefulPrototype = new Proxy(
      {},
      {
        getPrototypeOf() {
          const prototypeHost = Object.create(null, {
            buffer: {
              configurable: true,
              get() {
                return Reflect.apply(nativeBufferGetter, this, []);
              },
            },
          });
          visitedPrototypeHosts.add(prototypeHost);
          return prototypeHost;
        },
      },
    );
    Object.setPrototypeOf(memory, statefulPrototype);

    claim(memory);
    expect(() => claim(memory)).toThrow(/initialization attempt/);
    expect(visitedPrototypeHosts.size).toBe(2);
  });

  test('pins memory claims across ArrayBuffer prototype replacement', async () => {
    const source = await readFile(deferredLoaderPath, 'utf8');
    const claimSource = source
      .slice(
        source.indexOf('const __managedMemoryClaimsKey'),
        source.indexOf('function __attachCleanupError'),
      )
      .replaceAll('export ', '');
    const claim = runInNewContext(`${claimSource}\n__claimManagedMemoryForAttempt`) as (
      memory: WebAssembly.Memory,
    ) => void;
    const memory = new WebAssembly.Memory({ initial: 1, maximum: 1 });
    const buffer = memory.buffer;
    const originalPrototype = Object.getPrototypeOf(buffer);
    const replacementPrototype = {};

    claim(memory);
    Object.setPrototypeOf(buffer, replacementPrototype);

    expect(() => claim(memory)).toThrow(/initialization attempt/);
    expect(
      Object.getOwnPropertyDescriptor(
        replacementPrototype,
        Symbol.for('@rolldown/browser/workerd/managed-memory-claims/v1'),
      ),
    ).toBeUndefined();
    Object.setPrototypeOf(buffer, originalPrototype);
  });

  test.each([
    { name: 'extensible', preventExtensions: false },
    { name: 'non-extensible', preventExtensions: true },
  ])(
    'keeps $name grown memory consumed after buffer prototype replacement',
    async ({ preventExtensions }) => {
      const source = await readFile(deferredLoaderPath, 'utf8');
      const claimSource = source
        .slice(
          source.indexOf('const __managedMemoryClaimsKey'),
          source.indexOf('function __attachCleanupError'),
        )
        .replaceAll('export ', '');
      const claim = runInNewContext(`${claimSource}\n__claimManagedMemoryForAttempt`) as (
        memory: WebAssembly.Memory,
      ) => void;
      const memory = new WebAssembly.Memory({ initial: 1, maximum: 2 });
      if (preventExtensions) Object.preventExtensions(memory);
      const originalBuffer = memory.buffer;

      claim(memory);
      memory.grow(1);
      const grownBuffer = memory.buffer;
      Object.setPrototypeOf(grownBuffer, {});

      expect(grownBuffer).not.toBe(originalBuffer);
      expect(() => claim(memory)).toThrow(/initialization attempt/);
      const pinnedDescriptor = Object.getOwnPropertyDescriptor(
        memory,
        Symbol.for('@rolldown/browser/workerd/managed-memory-claims/v1'),
      );
      if (preventExtensions) {
        expect(pinnedDescriptor).toBeUndefined();
      } else {
        expect(pinnedDescriptor).toMatchObject({
          configurable: false,
          enumerable: false,
          writable: false,
        });
      }
    },
  );

  test.each([
    {
      name: 'self-cyclic',
      setup: `
let prototype
prototype = new Proxy({}, {
  getPrototypeOf() {
    return prototype
  },
})
`,
      expectedError: 'Cyclic prototype chain detected',
    },
    {
      name: 'fresh-proxy',
      setup: `
const freshPrototypeHandler = {
  getPrototypeOf() {
    return new Proxy({}, freshPrototypeHandler)
  },
}
const prototype = new Proxy({}, freshPrototypeHandler)
`,
      expectedError: 'prototype chain exceeds the traversal limit',
    },
  ])(
    'rejects $name memory prototype chains without hanging',
    { timeout: 10_000 },
    ({ setup, expectedError }) => {
      const child = spawnSync(
        process.execPath,
        [
          '--input-type=module',
          '--eval',
          `
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { runInNewContext } from 'node:vm'

const loaderPath = ${JSON.stringify(fileURLToPath(deferredLoaderPath))}
const source = await readFile(loaderPath, 'utf8')
const claimSource = source
  .slice(
    source.indexOf('const __managedMemoryClaimsKey'),
    source.indexOf('function __attachCleanupError'),
  )
  .replaceAll('export ', '')
const claim = runInNewContext(
  \`\${claimSource}\\n__claimManagedMemoryForAttempt\`,
)
const memory = new WebAssembly.Memory({ initial: 1, maximum: 1 })
${setup}
Object.setPrototypeOf(memory, prototype)
assert.throws(
  () => claim(memory),
  new RegExp(${JSON.stringify(expectedError)}),
)
console.log('memory proxy prototype chain rejected')
`,
        ],
        {
          encoding: 'utf8',
          timeout: 3_000,
        },
      );

      expect(child.error).toBeUndefined();
      expect(child.signal).toBeNull();
      expect(child.status, child.stderr || child.stdout).toBe(0);
      expect(child.stdout).toContain('memory proxy prototype chain rejected');
    },
  );

  test('keeps failed disposal live and retries cleanup', async () => {
    let destroyCalls = 0;
    let instance: DeferredRolldownInstance | undefined;
    const cleanupError = new Error('cleanup failed');
    const context = {
      suppressDestroy() {},
      destroy() {
        destroyCalls += 1;
        if (destroyCalls === 1) throw cleanupError;
        instance?.dispose();
      },
    };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async (
        _module: WebAssembly.Module,
        options: { beforeInit: (input: { instance: { exports: object } }) => void },
      ) => {
        options.beforeInit({ instance: { exports: {} } });
        return {
          napiModule: {
            exports: {
              registerCurrentThreadTaskHost() {},
              registerTimerHost() {},
            },
          },
        };
      },
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
    const before = loader.getDeferredRuntimeStats();
    const managedInstance = (await loader.createInstance(module, {
      initialMemoryPages: 1,
      maximumMemoryPages: 1,
    })) as DeferredRolldownInstance;
    instance = managedInstance;

    expect(() => managedInstance.dispose()).toThrow(cleanupError);
    expect(managedInstance.disposed).toBe(false);
    expect(managedInstance.memoryBytes).toBeGreaterThan(0);
    expect(() => managedInstance.exports).toThrow(/disposal has started/);
    expect(loader.getDeferredRuntimeStats().liveInstances).toBe(before.liveInstances + 1);

    expect(() => managedInstance.dispose()).not.toThrow();
    expect(destroyCalls).toBe(2);
    expect(managedInstance.disposed).toBe(true);
    expect(loader.getDeferredRuntimeStats().liveInstances).toBe(before.liveInstances);
  });

  test('evicts exact task and timer hosts before a failed destroy and preserves fallback hosts', async () => {
    vi.useFakeTimers();
    try {
      type Host = { label: string };
      const taskHosts = new Map<number, Host>();
      const timerHosts = new Map<number, Host>();
      const unregisterCalls: string[] = [];
      const timerRelays = new Map<string, Promise<void>>();
      let nextRegistration = 1;
      const createRawBinding = (label: string) => {
        return {
          getCurrentThreadTaskHostContractVersion: () => 4,
          isCurrentThreadHostRegistrationActive: (_high: number, low: number) =>
            taskHosts.has(low) || timerHosts.has(low),
          reserveCurrentThreadHostRegistration: () => ({ high: 0, low: nextRegistration++ }),
          registerCurrentThreadTaskHost(_high: number, low: number) {
            taskHosts.set(low, { label });
          },
          unregisterCurrentThreadTaskHost(_high: number, low: number) {
            unregisterCalls.push(`task:${label}:${low}`);
            taskHosts.delete(low);
          },
          registerTimerHost(
            _high: number,
            low: number,
            schedule: (id: number, ms: number) => Promise<void>,
            _cancel: (id: number) => void,
          ) {
            timerHosts.set(low, { label });
            timerRelays.set(label, schedule(low, 60_000));
          },
          unregisterTimerHost(_high: number, low: number) {
            unregisterCalls.push(`timer:${label}:${low}`);
            timerHosts.delete(low);
          },
        };
      };
      let secondDestroyCalls = 0;
      const contexts = [
        { suppressDestroy() {}, destroy: vi.fn() },
        {
          suppressDestroy() {},
          destroy() {
            secondDestroyCalls += 1;
            if (secondDestroyCalls === 1) throw new Error('cleanup hook failed');
          },
        },
      ];
      const rawBindings = [createRawBinding('first'), createRawBinding('second')];
      let contextIndex = 0;
      let bindingIndex = 0;
      const loader = await loadDeferredLoaderWithDependencies({
        createContext: () => contexts[contextIndex++],
        getDefaultContext: vi.fn(),
        instantiateNapiModule: async () => ({
          napiModule: {
            exports: rawBindings[bindingIndex++],
          },
        }),
        WASI: class {},
      });
      const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
      const first = await loader.createInstance(module, {
        initialMemoryPages: 1,
        maximumMemoryPages: 1,
      });
      const second = await loader.createInstance(module, {
        initialMemoryPages: 1,
        maximumMemoryPages: 1,
      });

      expect([...taskHosts.values()].at(-1)?.label).toBe('second');
      expect([...timerHosts.values()].at(-1)?.label).toBe('second');
      expect(vi.getTimerCount()).toBe(2);

      expect(() => second.dispose()).toThrow('cleanup hook failed');
      expect(second.disposed).toBe(false);
      expect([...taskHosts.values()].map(({ label }) => label)).toEqual(['first']);
      expect([...timerHosts.values()].map(({ label }) => label)).toEqual(['first']);
      expect(unregisterCalls.map((call) => call.split(':').slice(0, 2).join(':'))).toEqual([
        'timer:second',
        'task:second',
      ]);
      await timerRelays.get('second');
      expect(vi.getTimerCount()).toBe(1);
      await vi.runAllTimersAsync();
      expect(vi.getTimerCount()).toBe(0);

      second.dispose();
      expect(second.disposed).toBe(true);
      expect(secondDestroyCalls).toBe(2);
      expect(unregisterCalls).toHaveLength(2);

      first.dispose();
      expect(first.disposed).toBe(true);
      expect(taskHosts.size).toBe(0);
      expect(timerHosts.size).toBe(0);
    } finally {
      vi.useRealTimers();
    }
  });

  test('retries failed initialization cleanup without masking the primary error', async () => {
    const initializationError = new Error('initialization failed');
    const cleanupError = new Error('cleanup failed');
    let destroyCalls = 0;
    const context = {
      suppressDestroy() {},
      destroy() {
        destroyCalls += 1;
        if (destroyCalls === 1) throw cleanupError;
      },
    };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async () => {
        throw initializationError;
      },
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));

    await expect(
      loader.createInstance(module, {
        initialMemoryPages: 1,
        maximumMemoryPages: 1,
      }),
    ).rejects.toBe(initializationError);
    expect(destroyCalls).toBe(2);
    expect(initializationError.cause).toBeUndefined();
  });

  test('retains cleanup diagnostics when initialization cleanup retry fails', async () => {
    const initializationError = new Error('initialization failed');
    const cleanupErrors = [new Error('cleanup failed once'), new Error('cleanup failed twice')];
    const context = {
      suppressDestroy() {},
      destroy() {
        throw cleanupErrors.shift();
      },
    };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: async () => {
        throw initializationError;
      },
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));

    const failure = await loader
      .createInstance(module, {
        initialMemoryPages: 1,
        maximumMemoryPages: 1,
      })
      .then(
        () => {
          throw new Error('Expected deferred workerd initialization to fail');
        },
        (error: unknown) => error,
      );
    expectCleanupFailure(failure, initializationError, 'Managed workerd context cleanup failed');
  });

  test('retains cleanup diagnostics for primitive initialization failures', async () => {
    const cleanupErrors = [new Error('cleanup failed once'), new Error('cleanup failed twice')];
    const context = {
      suppressDestroy() {},
      destroy() {
        throw cleanupErrors.shift();
      },
    };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
      getDefaultContext: () => context,
      instantiateNapiModule: () => Promise.reject('primitive initialization failure'),
      WASI: class {},
    });
    const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));

    await expect(
      loader.createInstance(module, {
        initialMemoryPages: 1,
        maximumMemoryPages: 1,
      }),
    ).rejects.toMatchObject({
      cause: 'primitive initialization failure',
      errors: [
        'primitive initialization failure',
        expect.objectContaining({
          message: 'Managed workerd context cleanup failed',
          errors: [expect.any(Error), expect.any(Error)],
        }),
      ],
    });
  });

  test.each([
    {
      name: 'occupied cause',
      createPrimaryError: () =>
        new Error('initialization failed', { cause: new Error('existing cause') }),
    },
    {
      name: 'non-writable cause',
      createPrimaryError: () =>
        Object.defineProperty(new Error('initialization failed'), 'cause', {
          value: undefined,
          writable: false,
        }),
    },
    {
      name: 'throwing cause getter',
      createPrimaryError: () =>
        Object.defineProperty(new Error('initialization failed'), 'cause', {
          get() {
            throw new Error('cause getter failed');
          },
        }),
    },
    {
      name: 'stateful cause accessor',
      createPrimaryError: () => {
        let reads = 0;
        let assigned: unknown;
        return Object.defineProperty(new Error('initialization failed'), 'cause', {
          get() {
            reads += 1;
            return reads === 2 ? assigned : undefined;
          },
          set(value: unknown) {
            assigned = value;
          },
        });
      },
    },
  ])('retains deferred cleanup diagnostics for $name', async ({ createPrimaryError }) => {
    const primaryError = createPrimaryError();
    expectCleanupFailure(
      await getDeferredInitializationFailure(primaryError),
      primaryError,
      'Managed workerd context cleanup failed',
    );
  });

  wasiTest('does not consume caller memory when module validation fails', async () => {
    const module = await WebAssembly.compile(await readFile(wasmPath));
    const memory = new WebAssembly.Memory({
      initial: WORKERD_WASM_MEMORY.initialPages,
      maximum: WORKERD_WASM_MEMORY.maximumPages,
    });

    await expect(
      instantiate(new Uint8Array([0, 97, 115, 109]) as unknown as WebAssembly.Module, { memory }),
    ).rejects.toThrow(/precompiled WebAssembly\.Module/);

    const instance = await instantiate(module, { memory });
    instance.dispose();
  });

  wasiTest('keeps caller memory consumed after initialization fails', async () => {
    const incompatibleModule = await WebAssembly.compile(
      new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]),
    );
    const module = await WebAssembly.compile(await readFile(wasmPath));
    const memory = new WebAssembly.Memory({
      initial: WORKERD_WASM_MEMORY.initialPages,
      maximum: WORKERD_WASM_MEMORY.maximumPages,
    });
    const beforeStats = getWorkerdRuntimeStats();
    const beforeListeners = process.rawListeners('beforeExit').length;

    await expect(instantiate(incompatibleModule, { memory })).rejects.toThrow();
    expect(getWorkerdRuntimeStats()).toEqual(beforeStats);
    expect(process.rawListeners('beforeExit')).toHaveLength(beforeListeners);

    await expect(instantiate(module, { memory })).rejects.toThrow(/initialization attempt/);
    expect(getWorkerdRuntimeStats()).toEqual(beforeStats);
    expect(process.rawListeners('beforeExit')).toHaveLength(beforeListeners);
  });

  wasiTest('accepts Buffer asset inputs without a Buffer global', async () => {
    const module = await WebAssembly.compile(await readFile(wasmPath));
    vi.stubGlobal('Buffer', undefined);

    let instance: Awaited<ReturnType<typeof instantiate>> | undefined;
    try {
      instance = await instantiate(module);
      const bundler = new instance.exports.BindingBundler();
      try {
        const result = await bundler.generate({
          inputOptions: {
            input: [{ import: 'virtual:entry' }],
            plugins: [
              {
                name: 'binary-asset',
                hookUsage: 11,
                buildStart(ctx) {
                  ctx.emitFile({
                    fileName: 'asset.bin',
                    source: { inner: NodeBuffer.from([0, 1, 255]) },
                  });
                },
                resolveId(_ctx, id) {
                  if (id === 'virtual:entry') return { id };
                },
                load(_ctx, id) {
                  if (id === 'virtual:entry') return { code: 'export default 1' };
                },
              },
            ],
            cwd: '/',
            logLevel: 0,
            onLog() {},
          },
          outputOptions: {
            format: 'es',
            plugins: [],
          },
        });
        if ('isBindingErrors' in result) {
          throw new Error(JSON.stringify(result.errors));
        }

        const source = result.assets[0].getSource().inner;
        expect(globalThis.Buffer).toBeUndefined();
        expect(source.constructor).toBe(Uint8Array);
        if (!(source instanceof Uint8Array)) throw new TypeError('Expected a binary asset');
        expect(Array.from(source)).toEqual([0, 1, 255]);
      } finally {
        await bundler.close();
      }
    } finally {
      instance?.dispose();
      vi.unstubAllGlobals();
    }
  });

  wasiTest('does not retain Node beforeExit listeners after managed disposal', async () => {
    const module = await WebAssembly.compile(await readFile(wasmPath));
    const before = process.rawListeners('beforeExit').length;

    for (let index = 0; index < 3; index += 1) {
      const instance = await instantiate(module);
      instance.dispose();
    }

    expect(process.rawListeners('beforeExit')).toHaveLength(before);
  });

  wasiTest('skips deferred emnapi TSFN drains after managed disposal', { timeout: 30_000 }, () => {
    const require = createRequire(import.meta.url);
    const wasmRuntimeUrl = pathToFileURL(require.resolve('@napi-rs/wasm-runtime')).href;
    const emnapiRuntimeUrl = pathToFileURL(require.resolve('@emnapi/runtime')).href;
    const child = spawnSync(
      process.execPath,
      [
        '--input-type=module',
        '--eval',
        `
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const realSetImmediate = globalThis.setImmediate
const immediateQueue = []
globalThis.setImmediate = (callback, ...args) => {
  immediateQueue.push(() => callback(...args))
  return immediateQueue.length
}

try {
  const [{ instantiateNapiModule, getDefaultContext, WASI }, { createContext }] =
    await Promise.all([
      import(${JSON.stringify(wasmRuntimeUrl)}),
      import(${JSON.stringify(emnapiRuntimeUrl)}),
    ])
  const source = await readFile(${JSON.stringify(fileURLToPath(deferredLoaderPath))}, 'utf8')
  const dependencyKey = '__rolldownManagedTsfnDisposalTest'
  let rawBinding
  globalThis[dependencyKey] = {
    Buffer,
    createContext,
    getDefaultContext,
    instantiateNapiModule: async (...args) => {
      const result = await instantiateNapiModule(...args)
      rawBinding = result.napiModule.exports
      return result
    },
    WASI,
  }
  const transformed = source
    .replace(
      /import \\{[\\s\\S]*?\\} from '@napi-rs\\/wasm-runtime'\\nimport \\{ createContext as __emnapiCreateContext \\} from '@emnapi\\/runtime'\\n/,
      \`const {
  getDefaultContext: __emnapiGetDefaultContext,
  instantiateNapiModule: __emnapiInstantiateNapiModule,
  WASI: __WASI,
  createContext: __emnapiCreateContext,
  Buffer,
} = globalThis[\${JSON.stringify(dependencyKey)}]\\n\`,
    )
    .replace("import { Buffer } from 'buffer'\\n", '')
  const loader = await import(
    \`data:text/javascript;base64,\${Buffer.from(transformed).toString('base64')}\`
  )
  delete globalThis[dependencyKey]
  const module = await WebAssembly.compile(
    await readFile(${JSON.stringify(fileURLToPath(wasmPath))}),
  )
  const instance = await loader.createInstance(module)
  assert.equal(immediateQueue.length, 0)

  // Bypass the managed facade only to deterministically leave one native
  // future queued while exercising the managed handle's real disposal path.
  const bundler = new rawBinding.BindingBundler()
  const pendingBuild = bundler.generate({
    inputOptions: {
      input: [{ import: '/missing.js' }],
      plugins: [],
      cwd: '/',
      logLevel: 0,
      onLog() {},
    },
    outputOptions: { format: 'es', plugins: [] },
  })
  pendingBuild.catch(() => {})
  assert.equal(immediateQueue.length, 1)
  const outerTurn = immediateQueue.shift()

  instance.dispose()
  assert.equal(instance.disposed, true)
  const cleanupTurnCount = immediateQueue.length
  assert.ok(cleanupTurnCount > 0)

  // The outer TSFN turn was already accepted, so it observes the function as
  // live and queues the nested drain behind finalization.
  outerTurn()
  assert.equal(immediateQueue.length, cleanupTurnCount + 1)
  const nestedTurn = immediateQueue.pop()
  let cleanupRuns = 0
  while (immediateQueue.length > 0) {
    assert.ok(cleanupRuns++ < 100)
    immediateQueue.shift()()
  }

  const originalExchange = Atomics.exchange
  let postFinalizeExchanges = 0
  Atomics.exchange = (...args) => {
    postFinalizeExchanges += 1
    return Reflect.apply(originalExchange, Atomics, args)
  }
  try {
    nestedTurn()
  } finally {
    Atomics.exchange = originalExchange
  }
  assert.equal(postFinalizeExchanges, 0)
  console.log('deferred TSFN drain skipped after managed disposal')
} finally {
  globalThis.setImmediate = realSetImmediate
}
`,
      ],
      {
        encoding: 'utf8',
        timeout: 20_000,
      },
    );

    expect(child.error).toBeUndefined();
    expect(child.signal).toBeNull();
    expect(child.status, child.stderr || child.stdout).toBe(0);
    expect(child.stdout).toContain('deferred TSFN drain skipped after managed disposal');
  });

  wasiTest('does not claim timer support when the host has no setTimeout', async () => {
    const module = await WebAssembly.compile(await readFile(wasmPath));
    vi.stubGlobal('setTimeout', undefined);

    try {
      const instance = await instantiate(module);
      try {
        expect(instance.exports.getRuntimeCapabilities().timers).toBe(false);
      } finally {
        instance.dispose();
      }
    } finally {
      vi.unstubAllGlobals();
    }
  });

  wasiTest('does not claim timer support when the host cannot cancel timeouts', async () => {
    const module = await WebAssembly.compile(await readFile(wasmPath));
    vi.stubGlobal('clearTimeout', undefined);

    try {
      const instance = await instantiate(module);
      try {
        expect(instance.exports.getRuntimeCapabilities().timers).toBe(false);
      } finally {
        instance.dispose();
      }
    } finally {
      vi.unstubAllGlobals();
    }
  });

  test('rejects a mismatched managed task-host ABI before registration', () => {
    const registerCurrentThreadTaskHost = vi.fn();
    expect(() =>
      registerWorkerdCurrentThreadTaskHost({
        getCurrentThreadTaskHostContractVersion: () => 1,
        isCurrentThreadHostRegistrationActive: vi.fn(),
        registerCurrentThreadTaskHost,
        reserveCurrentThreadHostRegistration: vi.fn(),
        unregisterCurrentThreadTaskHost: vi.fn(),
      }),
    ).toThrow(/contract version 1.*version 4/);
    expect(registerCurrentThreadTaskHost).not.toHaveBeenCalled();
  });

  test.each([
    ['missing registration', undefined],
    ['null registration', null],
    ['missing high word', { low: 1 }],
    ['missing low word', { high: 0 }],
    ['string high word', { high: '0', low: 1 }],
    ['negative high word', { high: -1, low: 1 }],
    ['overflowing high word', { high: 0x1_0000_0000, low: 1 }],
    ['fractional low word', { high: 0, low: 1.5 }],
    ['overflowing low word', { high: 0, low: 0x1_0000_0000 }],
    ['inactive registration', { high: 0, low: 0 }],
  ])('rejects a managed task host with %s', (_name, registration) => {
    const registerCurrentThreadTaskHost = vi.fn();
    const unregisterCurrentThreadTaskHost = vi.fn();
    expect(() =>
      registerWorkerdCurrentThreadTaskHost({
        getCurrentThreadTaskHostContractVersion: () => 4,
        isCurrentThreadHostRegistrationActive: vi.fn(() => true),
        registerCurrentThreadTaskHost,
        reserveCurrentThreadHostRegistration: () => registration,
        unregisterCurrentThreadTaskHost,
      }),
    ).toThrow(/invalid host registration/);
    expect(registerCurrentThreadTaskHost).not.toHaveBeenCalled();
    expect(unregisterCurrentThreadTaskHost).not.toHaveBeenCalled();
  });

  test('unregisters the exact managed task host once and retries a failed unregister', () => {
    const registration = { high: 0x1234_5678, low: 0x9abc_def0 };
    const unregisterError = new Error('task host unregister failed');
    const registerCurrentThreadTaskHost = vi.fn();
    const unregisterCurrentThreadTaskHost = vi
      .fn()
      .mockImplementationOnce(() => {
        throw unregisterError;
      })
      .mockImplementation(() => {});
    const dispose = registerWorkerdCurrentThreadTaskHost({
      getCurrentThreadTaskHostContractVersion: () => 4,
      isCurrentThreadHostRegistrationActive: () => true,
      registerCurrentThreadTaskHost,
      reserveCurrentThreadHostRegistration: () => registration,
      unregisterCurrentThreadTaskHost,
    });

    expect(registerCurrentThreadTaskHost).toHaveBeenCalledWith(registration.high, registration.low);
    expect(() => dispose()).toThrow(unregisterError);
    expect(() => dispose()).not.toThrow();
    expect(() => dispose()).not.toThrow();
    expect(unregisterCurrentThreadTaskHost).toHaveBeenCalledTimes(2);
    expect(unregisterCurrentThreadTaskHost).toHaveBeenNthCalledWith(
      1,
      registration.high,
      registration.low,
    );
    expect(unregisterCurrentThreadTaskHost).toHaveBeenNthCalledWith(
      2,
      registration.high,
      registration.low,
    );
  });

  test('generates equivalent ABI-v4 task hosts for CJS and browser roots', async () => {
    const [cjsBootstrap, browserBootstrap] = await Promise.all([
      readCurrentThreadHostBootstrap(cjsLoaderPath),
      readCurrentThreadHostBootstrap(browserLoaderPath),
    ]);
    const normalizeCapturedRegistrations = (bootstrap: string) =>
      bootstrap
        .replace(/^[ \t]*__browserTaskHostRegistration = __taskHostRegistration$/m, '')
        .replace(/^[ \t]*__browserTimerHostRegistration = __timerHostRegistration$/m, '')
        .replace(/^[ \t]*__nodeTaskHostRegistration = __taskHostRegistration$/m, '')
        .replace(/^[ \t]*__nodeTimerHostRegistration = __timerHostRegistration$/m, '');
    expect(normalizeCapturedRegistrations(browserBootstrap)).toBe(
      normalizeCapturedRegistrations(cjsBootstrap),
    );

    for (const bootstrap of [cjsBootstrap, browserBootstrap]) {
      const taskRegistration = { high: 0x1234_5678, low: 0x9abc_def0 };
      const timerRegistration = { high: 0, low: 2 };
      const registrations = [taskRegistration, timerRegistration];
      const live = new Set<number>();
      const binding = {
        getCurrentThreadTaskHostContractVersion: vi.fn(() => 4),
        isCurrentThreadHostRegistrationActive: vi.fn((_high: number, low: number) => live.has(low)),
        registerCurrentThreadTaskHost: vi.fn((_high: number, low: number) => {
          live.add(low);
        }),
        registerTimerHost: vi.fn((_high: number, low: number) => {
          live.add(low);
        }),
        reserveCurrentThreadHostRegistration: vi.fn(() => registrations.shift()),
        unregisterCurrentThreadTaskHost: vi.fn(),
        unregisterTimerHost: vi.fn(),
      };
      runCurrentThreadHostBootstrap(bootstrap, binding);
      expect(binding.getCurrentThreadTaskHostContractVersion).toHaveBeenCalledWith();
      expect(binding.reserveCurrentThreadHostRegistration).toHaveBeenCalledTimes(2);
      expect(binding.registerCurrentThreadTaskHost).toHaveBeenCalledWith(
        taskRegistration.high,
        taskRegistration.low,
      );
      for (const removedExport of removedTaskHostExports) {
        expect(bootstrap).not.toContain(removedExport);
      }
    }
  });

  test('rejects a mismatched generated root task-host ABI before registration', async () => {
    const bootstrap = await readCurrentThreadHostBootstrap(cjsLoaderPath);
    const registerCurrentThreadTaskHost = vi.fn();
    expect(() =>
      runCurrentThreadHostBootstrap(bootstrap, {
        getCurrentThreadTaskHostContractVersion: () => 1,
        isCurrentThreadHostRegistrationActive: vi.fn(),
        registerCurrentThreadTaskHost,
        registerTimerHost: vi.fn(),
        reserveCurrentThreadHostRegistration: vi.fn(),
        unregisterCurrentThreadTaskHost: vi.fn(),
        unregisterTimerHost: vi.fn(),
      }),
    ).toThrow(/contract version 1.*version 4/);
    expect(registerCurrentThreadTaskHost).not.toHaveBeenCalled();
  });

  test.each([
    ['missing registration', undefined],
    ['missing high word', { low: 1 }],
    ['missing low word', { high: 0 }],
    ['fractional low word', { high: 0, low: 1.5 }],
    ['inactive registration', { high: 0, low: 0 }],
  ])('rejects a generated root task host with %s', async (_name, registration) => {
    const bootstrap = await readCurrentThreadHostBootstrap(cjsLoaderPath);
    const registerCurrentThreadTaskHost = vi.fn();
    const registerTimerHost = vi.fn();
    expect(() =>
      runCurrentThreadHostBootstrap(bootstrap, {
        getCurrentThreadTaskHostContractVersion: () => 4,
        isCurrentThreadHostRegistrationActive: vi.fn(() => true),
        registerCurrentThreadTaskHost,
        registerTimerHost,
        reserveCurrentThreadHostRegistration: () => registration,
        unregisterCurrentThreadTaskHost: vi.fn(),
        unregisterTimerHost: vi.fn(),
      }),
    ).toThrow(/invalid task host registration/);
    expect(registerCurrentThreadTaskHost).not.toHaveBeenCalled();
    expect(registerTimerHost).not.toHaveBeenCalled();
  });

  test('keeps removed task-delivery capabilities out of generated roots', async () => {
    const [cjsSource, browserSource] = await Promise.all([
      readFile(cjsLoaderPath, 'utf8'),
      readFile(browserLoaderPath, 'utf8'),
    ]);

    for (const source of [cjsSource, browserSource]) {
      for (const removedExport of removedTaskHostExports) {
        expect(source).not.toContain(removedExport);
      }
    }
    expect(cjsSource.indexOf(currentThreadBootstrapEnd)).toBeLessThan(
      cjsSource.indexOf('module.exports = __napiModule.exports'),
    );
    expect(browserSource.indexOf(currentThreadBootstrapEnd)).toBeLessThan(
      browserSource.indexOf('export default __napiModule.exports'),
    );
    expect(browserSource.indexOf(browserInitializationGuardStart)).toBeLessThan(
      browserSource.indexOf(currentThreadBootstrapStart),
    );
    expect(browserSource.indexOf(currentThreadBootstrapEnd)).toBeLessThan(
      browserSource.indexOf(browserInitializationGuardEnd),
    );
    expect(cjsSource.indexOf(currentThreadBootstrapEnd)).toBeLessThan(
      cjsSource.indexOf(nodeInitializationCleanupStart),
    );
    expect(cjsSource).toContain(nodeInitializationCleanupEnd);
  });

  test('rolls back the exit listener when beforeExit removal throws', async () => {
    const lifecycle = await readGeneratedNodeLifecycle();
    const removalError = new Error('beforeExit removal failed');
    const operations: string[] = [];
    const listeners = {
      beforeExit: [] as Array<() => void>,
      exit: [] as Array<() => void>,
    };
    let failedBeforeExitRemoval = false;
    const processStub = {
      once(event: keyof typeof listeners, listener: () => void) {
        operations.push(`once:${event}`);
        listeners[event].push(listener);
      },
      removeListener(event: keyof typeof listeners, listener: () => void) {
        operations.push(`remove:${event}`);
        if (event === 'beforeExit' && !failedBeforeExitRemoval) {
          failedBeforeExitRemoval = true;
          throw removalError;
        }
        const index = listeners[event].lastIndexOf(listener);
        if (index >= 0) listeners[event].splice(index, 1);
      },
    };
    const state: Record<string, unknown> = {};

    expect(() =>
      runInNewContext(
        `let __emnapiContext = { destroy() {} }
let __emnapiContextDestroyed = false
let __emnapiContextDestroying = false
let __emnapiContextDestroyPromise
let __emnapiContextRegisteredForBeforeExit = false
let __emnapiContextRegisteredForExit = false
let __emnapiContextBeforeExitRegistrationRetryCount = 0
let __emnapiContextBeforeExitRegistrationRetryScheduled = false
let __napiInstance
let __emnapiWasmEnvCleanupPrepared = false
${lifecycle}
__registerEmnapiContextBeforeExit()
try {
  __handoffEmnapiContextCleanupToExit()
} finally {
  __state.beforeExit = __emnapiContextRegisteredForBeforeExit
  __state.exit = __emnapiContextRegisteredForExit
}`,
        {
          __state: state,
          process: processStub,
        },
      ),
    ).toThrow(removalError);
    expect(operations).toEqual([
      'once:beforeExit',
      'once:exit',
      'remove:beforeExit',
      'remove:exit',
    ]);
    expect(listeners.beforeExit).toHaveLength(1);
    expect(listeners.exit).toHaveLength(0);
    expect(state).toEqual({ beforeExit: true, exit: false });
  });

  test('preserves both listeners when handoff and rollback removal fail', async () => {
    const lifecycle = await readGeneratedNodeLifecycle();
    const beforeExitError = new Error('beforeExit removal failed');
    const rollbackError = new Error('exit rollback removal failed');
    const operations: string[] = [];
    const listeners = {
      beforeExit: [] as Array<() => void>,
      exit: [] as Array<() => void>,
    };
    const processStub = {
      once(event: keyof typeof listeners, listener: () => void) {
        operations.push(`once:${event}`);
        listeners[event].push(listener);
      },
      removeListener(event: keyof typeof listeners) {
        operations.push(`remove:${event}`);
        throw event === 'beforeExit' ? beforeExitError : rollbackError;
      },
    };
    const state: Record<string, unknown> = {};
    let failure: unknown;

    try {
      runInNewContext(
        `let __emnapiContext = { destroy() {} }
let __emnapiContextDestroyed = false
let __emnapiContextDestroying = false
let __emnapiContextDestroyPromise
let __emnapiContextRegisteredForBeforeExit = false
let __emnapiContextRegisteredForExit = false
let __emnapiContextBeforeExitRegistrationRetryCount = 0
let __emnapiContextBeforeExitRegistrationRetryScheduled = false
let __napiInstance
let __emnapiWasmEnvCleanupPrepared = false
${lifecycle}
__registerEmnapiContextBeforeExit()
try {
  __handoffEmnapiContextCleanupToExit()
} finally {
  __state.beforeExit = __emnapiContextRegisteredForBeforeExit
  __state.exit = __emnapiContextRegisteredForExit
}`,
        {
          __state: state,
          process: processStub,
        },
      );
    } catch (error) {
      failure = error;
    }

    expect(operations).toEqual([
      'once:beforeExit',
      'once:exit',
      'remove:beforeExit',
      'remove:exit',
    ]);
    expect(listeners.beforeExit).toHaveLength(1);
    expect(listeners.exit).toHaveLength(1);
    expect(state).toEqual({ beforeExit: true, exit: true });
    expect(failure).toMatchObject({
      cause: beforeExitError,
      errors: [beforeExitError, rollbackError],
      message: 'emnapi context cleanup listener handoff failed',
    });
  });

  test('keeps beforeExit ownership when exit listener registration throws', async () => {
    const lifecycle = await readGeneratedNodeLifecycle();
    const registrationError = new Error('exit registration failed');
    const operations: string[] = [];
    const beforeExitListeners: Array<() => void> = [];
    const processStub = {
      once(event: string, listener: () => void) {
        operations.push(`once:${event}`);
        if (event === 'exit') throw registrationError;
        beforeExitListeners.push(listener);
      },
      removeListener(event: string) {
        operations.push(`remove:${event}`);
      },
    };
    const state: Record<string, unknown> = {};

    expect(() =>
      runInNewContext(
        `let __emnapiContext = { destroy() {} }
let __emnapiContextDestroyed = false
let __emnapiContextDestroying = false
let __emnapiContextDestroyPromise
let __emnapiContextRegisteredForBeforeExit = false
let __emnapiContextRegisteredForExit = false
let __emnapiContextBeforeExitRegistrationRetryCount = 0
let __emnapiContextBeforeExitRegistrationRetryScheduled = false
let __napiInstance
let __emnapiWasmEnvCleanupPrepared = false
${lifecycle}
__registerEmnapiContextBeforeExit()
try {
  __handoffEmnapiContextCleanupToExit()
} finally {
  __state.beforeExit = __emnapiContextRegisteredForBeforeExit
  __state.exit = __emnapiContextRegisteredForExit
}`,
        {
          __state: state,
          process: processStub,
        },
      ),
    ).toThrow(registrationError);
    expect(operations).toEqual(['once:beforeExit', 'once:exit']);
    expect(beforeExitListeners).toHaveLength(1);
    expect(state).toEqual({ beforeExit: true, exit: false });
  });

  test('aggregates both listener removal failures and keeps them retryable', async () => {
    const lifecycle = await readGeneratedNodeLifecycle();
    const beforeExitError = new Error('beforeExit removal failed');
    const exitError = new Error('exit removal failed');
    const operations: string[] = [];
    const listeners = {
      beforeExit: [] as Array<() => void>,
      exit: [] as Array<() => void>,
    };
    let failRemovals = true;
    const processStub = {
      once(event: keyof typeof listeners, listener: () => void) {
        operations.push(`once:${event}`);
        listeners[event].push(listener);
      },
      removeListener(event: keyof typeof listeners, listener: () => void) {
        operations.push(`remove:${event}`);
        if (failRemovals) {
          throw event === 'beforeExit' ? beforeExitError : exitError;
        }
        const index = listeners[event].lastIndexOf(listener);
        if (index >= 0) listeners[event].splice(index, 1);
      },
    };
    const state: Record<string, unknown> = {};

    runInNewContext(
      `let __emnapiContext = { destroy() {} }
let __emnapiContextDestroyed = false
let __emnapiContextDestroying = false
let __emnapiContextDestroyPromise
let __emnapiContextRegisteredForBeforeExit = false
let __emnapiContextRegisteredForExit = false
let __emnapiContextBeforeExitRegistrationRetryCount = 0
let __emnapiContextBeforeExitRegistrationRetryScheduled = false
let __napiInstance
let __emnapiWasmEnvCleanupPrepared = false
${lifecycle}
__registerEmnapiContextBeforeExit()
__registerEmnapiContextAtExit()
try {
  __removeEmnapiContextCleanupListeners()
} catch (__error) {
  __state.failure = __error
}
__state.failedBeforeExit = __emnapiContextRegisteredForBeforeExit
__state.failedExit = __emnapiContextRegisteredForExit
__state.failedBeforeExitListeners = __listenerCount('beforeExit')
__state.failedExitListeners = __listenerCount('exit')
__allowRemovals()
__removeEmnapiContextCleanupListeners()
__state.retriedBeforeExit = __emnapiContextRegisteredForBeforeExit
__state.retriedExit = __emnapiContextRegisteredForExit`,
      {
        __allowRemovals: () => {
          failRemovals = false;
        },
        __listenerCount: (event: keyof typeof listeners) => listeners[event].length,
        __state: state,
        process: processStub,
      },
    );

    expect(operations).toEqual([
      'once:beforeExit',
      'once:exit',
      'remove:beforeExit',
      'remove:exit',
      'remove:beforeExit',
      'remove:exit',
    ]);
    expect(state).toMatchObject({
      failedBeforeExit: true,
      failedBeforeExitListeners: 1,
      failedExit: true,
      failedExitListeners: 1,
      retriedBeforeExit: false,
      retriedExit: false,
    });
    expect(state.failure).toMatchObject({
      errors: [beforeExitError, exitError],
      message: 'emnapi context cleanup listener removal failed',
    });
    expect(listeners.beforeExit).toHaveLength(0);
    expect(listeners.exit).toHaveLength(0);
  });

  test('rearms cleanup ownership when bootstrap and both destroy attempts fail', async () => {
    const source = await readFile(cjsLoaderPath, 'utf8');
    const lifecycle = await readGeneratedNodeLifecycle();
    const start = source.indexOf(nodeInitializationCleanupStart);
    const end = source.indexOf(nodeInitializationCleanupEnd, start);
    const cleanup = source.slice(start + nodeInitializationCleanupStart.length, end);
    const primaryError = new Error('Node bootstrap failed');
    const destroyErrors = [
      new Error('Node context cleanup failed once'),
      new Error('Node context cleanup failed twice'),
    ];
    const beforeExitListeners: Array<() => void> = [];
    const processStub = {
      once(event: string, listener: () => void) {
        if (event === 'beforeExit') beforeExitListeners.push(listener);
      },
      removeListener() {},
    };
    let failure: unknown;

    try {
      runInNewContext(
        `let __emnapiContext = {
  destroy() {
    throw __destroyErrors.shift()
  },
}
let __emnapiContextDestroyed = false
let __emnapiContextDestroying = false
let __emnapiContextDestroyPromise
let __emnapiContextRegisteredForBeforeExit = false
let __emnapiContextRegisteredForExit = false
let __emnapiContextBeforeExitRegistrationRetryCount = 0
let __emnapiContextBeforeExitRegistrationRetryScheduled = false
let __napiInstance
let __emnapiWasmEnvCleanupPrepared = false
let __nodeTaskHostRegistration
let __nodeTimerHostRegistration
${lifecycle}
try {
  throw __primaryError
${cleanup}`,
        {
          __attachCleanupError: vi.fn(),
          __destroyErrors: [...destroyErrors],
          __primaryError: primaryError,
          process: processStub,
        },
      );
    } catch (error) {
      failure = error;
    }

    expect(beforeExitListeners).toHaveLength(1);
    expect(failure).toMatchObject({
      cause: primaryError,
      errors: [
        primaryError,
        expect.objectContaining({
          errors: [
            expect.objectContaining({
              errors: destroyErrors,
              message: 'Threadless Node initialization context cleanup failed',
            }),
          ],
          message: 'Threadless Node initialization cleanup failed',
        }),
      ],
    });
  });

  test('retries transient generated Node context cleanup failures', async () => {
    const source = await readFile(cjsLoaderPath, 'utf8');
    const start = source.indexOf(nodeInitializationCleanupStart);
    const end = source.indexOf(nodeInitializationCleanupEnd, start);
    expect(start).toBeGreaterThanOrEqual(0);
    expect(end).toBeGreaterThan(start);
    const cleanup = source.slice(start + nodeInitializationCleanupStart.length, end);
    const primaryError = new Error('Node initialization failed');
    const firstCleanupError = new Error('transient Node context cleanup failure');
    const destroyEmnapiContext = vi
      .fn()
      .mockImplementationOnce(() => {
        throw firstCleanupError;
      })
      .mockImplementationOnce(() => {});
    const removeEmnapiContextCleanupListeners = vi.fn();
    const preserveCleanupError = vi.fn();

    expect(() =>
      runInNewContext(
        `try {
  throw __primaryError
${cleanup}`,
        {
          __preserveCleanupError: preserveCleanupError,
          __destroyEmnapiContext: destroyEmnapiContext,
          __emnapiContextRegisteredForBeforeExit: true,
          __removeEmnapiContextCleanupListeners: removeEmnapiContextCleanupListeners,
          __primaryError: primaryError,
        },
      ),
    ).toThrow(primaryError);
    expect(destroyEmnapiContext).toHaveBeenCalledTimes(2);
    expect(removeEmnapiContextCleanupListeners).toHaveBeenCalledOnce();
    expect(preserveCleanupError).not.toHaveBeenCalled();
  });

  test('retries transient generated Node host cleanup before destroying the context', async () => {
    const source = await readFile(cjsLoaderPath, 'utf8');
    const start = source.indexOf(nodeInitializationCleanupStart);
    const end = source.indexOf(nodeInitializationCleanupEnd, start);
    const cleanup = source.slice(start + nodeInitializationCleanupStart.length, end);
    const primaryError = new Error('Node initialization failed');
    const transientTimerError = new Error('transient timer-host cleanup failure');
    const taskRegistration = { high: 0x1234_5678, low: 0x9abc_def0 };
    const timerRegistration = { high: 0x0fed_cba9, low: 0x8765_4321 };
    const operations: string[] = [];
    const unregisterTimerHost = vi
      .fn()
      .mockImplementationOnce(() => {
        operations.push('timer');
        throw transientTimerError;
      })
      .mockImplementationOnce(() => {
        operations.push('timer');
      });
    const unregisterCurrentThreadTaskHost = vi.fn(() => {
      operations.push('task');
    });
    const destroyEmnapiContext = vi.fn(() => {
      operations.push('context');
    });
    const removeEmnapiContextCleanupListeners = vi.fn();

    expect(() =>
      runInNewContext(
        `try {
  throw __primaryError
${cleanup}`,
        {
          __attachCleanupError: vi.fn(),
          __destroyEmnapiContext: destroyEmnapiContext,
          __emnapiContextRegisteredForBeforeExit: true,
          __napiModule: {
            exports: {
              unregisterCurrentThreadTaskHost,
              unregisterTimerHost,
            },
          },
          __nodeTaskHostRegistration: taskRegistration,
          __nodeTimerHostRegistration: timerRegistration,
          __removeEmnapiContextCleanupListeners: removeEmnapiContextCleanupListeners,
          __retainEmnapiContextCleanupListener: vi.fn(),
          __primaryError: primaryError,
        },
      ),
    ).toThrow(primaryError);
    expect(operations).toEqual(['timer', 'timer', 'task', 'context']);
    expect(unregisterTimerHost).toHaveBeenNthCalledWith(
      1,
      timerRegistration.high,
      timerRegistration.low,
    );
    expect(unregisterTimerHost).toHaveBeenNthCalledWith(
      2,
      timerRegistration.high,
      timerRegistration.low,
    );
    expect(unregisterCurrentThreadTaskHost).toHaveBeenCalledWith(
      taskRegistration.high,
      taskRegistration.low,
    );
    expect(removeEmnapiContextCleanupListeners).toHaveBeenCalledOnce();
  });

  test('removes generated Node cleanup listeners after asynchronous context cleanup', async () => {
    const source = await readFile(cjsLoaderPath, 'utf8');
    const start = source.indexOf(nodeInitializationCleanupStart);
    const end = source.indexOf(nodeInitializationCleanupEnd, start);
    const cleanup = source.slice(start + nodeInitializationCleanupStart.length, end);
    const primaryError = new Error('Node initialization failed');
    const destroyEmnapiContext = vi.fn(() => Promise.resolve());
    const removeEmnapiContextCleanupListeners = vi.fn();
    const preserveCleanupError = vi.fn();

    expect(() =>
      runInNewContext(
        `try {
  throw __primaryError
${cleanup}`,
        {
          __preserveCleanupError: preserveCleanupError,
          __destroyEmnapiContext: destroyEmnapiContext,
          __emnapiContextRegisteredForBeforeExit: true,
          __removeEmnapiContextCleanupListeners: removeEmnapiContextCleanupListeners,
          __retainEmnapiContextCleanupListener: vi.fn(),
          __primaryError: primaryError,
        },
      ),
    ).toThrow(primaryError);
    await vi.waitFor(() => {
      expect(removeEmnapiContextCleanupListeners).toHaveBeenCalledOnce();
    });
    expect(destroyEmnapiContext).toHaveBeenCalledOnce();
    expect(preserveCleanupError).not.toHaveBeenCalled();
  });

  test('retains generated Node cleanup listeners after asynchronous context cleanup rejects', async () => {
    const source = await readFile(cjsLoaderPath, 'utf8');
    const start = source.indexOf(nodeInitializationCleanupStart);
    const end = source.indexOf(nodeInitializationCleanupEnd, start);
    const cleanup = source.slice(start + nodeInitializationCleanupStart.length, end);
    const primaryError = new Error('Node initialization failed');
    const cleanupError = new Error('asynchronous Node context cleanup failed');
    const destroyEmnapiContext = vi.fn(() => Promise.reject(cleanupError));
    const removeEmnapiContextCleanupListeners = vi.fn();
    const retainEmnapiContextCleanupListener = vi.fn();
    const preserveCleanupError = vi.fn();

    expect(() =>
      runInNewContext(
        `try {
  throw __primaryError
${cleanup}`,
        {
          __preserveCleanupError: preserveCleanupError,
          __destroyEmnapiContext: destroyEmnapiContext,
          __emnapiContextRegisteredForBeforeExit: true,
          __removeEmnapiContextCleanupListeners: removeEmnapiContextCleanupListeners,
          __retainEmnapiContextCleanupListener: retainEmnapiContextCleanupListener,
          __primaryError: primaryError,
        },
      ),
    ).toThrow(primaryError);
    await vi.waitFor(() => {
      expect(preserveCleanupError).toHaveBeenCalledWith(primaryError, cleanupError);
    });
    expect(destroyEmnapiContext).toHaveBeenCalledOnce();
    expect(removeEmnapiContextCleanupListeners).not.toHaveBeenCalled();
    expect(retainEmnapiContextCleanupListener).toHaveBeenCalledOnce();
  });

  test('surfaces asynchronous cleanup rejection after a primitive initialization failure', async () => {
    const source = await readFile(cjsLoaderPath, 'utf8');
    const lifecycle = await readGeneratedNodeLifecycle();
    const start = source.indexOf(nodeInitializationCleanupStart);
    const end = source.indexOf(nodeInitializationCleanupEnd, start);
    const cleanup = source.slice(start + nodeInitializationCleanupStart.length, end);
    const primaryError = 'primitive Node initialization failure';
    const cleanupError = new Error('asynchronous Node context cleanup failed');
    const queuedMicrotasks: Array<() => void> = [];
    const beforeExitListeners: Array<() => void> = [];
    const removeListener = vi.fn();
    let rejectCleanup!: (error: unknown) => void;
    const cleanupResult = new Promise<never>((_resolve, reject) => {
      rejectCleanup = reject;
    });
    let failure: unknown;

    try {
      runInNewContext(
        `let __emnapiContext = {
  destroy() {
    return __cleanupResult
  },
}
let __emnapiContextDestroyed = false
let __emnapiContextDestroying = false
let __emnapiContextDestroyPromise
let __emnapiContextRegisteredForBeforeExit = false
let __emnapiContextRegisteredForExit = false
let __emnapiContextBeforeExitRegistrationRetryCount = 0
let __emnapiContextBeforeExitRegistrationRetryScheduled = false
let __napiInstance
let __emnapiWasmEnvCleanupPrepared = false
${lifecycle}
__registerEmnapiContextBeforeExit()
try {
  throw __primaryError
${cleanup}`,
        {
          __cleanupResult: cleanupResult,
          __primaryError: primaryError,
          process: {
            once(event: string, listener: () => void) {
              if (event === 'beforeExit') beforeExitListeners.push(listener);
            },
            removeListener,
          },
          queueMicrotask(callback: () => void) {
            queuedMicrotasks.push(callback);
          },
        },
      );
    } catch (error) {
      failure = error;
    }

    expect(failure).toBe(primaryError);
    rejectCleanup(cleanupError);
    await vi.waitFor(() => {
      expect(queuedMicrotasks).toHaveLength(1);
    });
    expect(beforeExitListeners).toHaveLength(1);
    expect(removeListener).not.toHaveBeenCalled();
    expect(() => queuedMicrotasks[0]()).toThrow(cleanupError);
  });

  test('surfaces asynchronous listener-removal failure when the primary cause is occupied', async () => {
    const source = await readFile(cjsLoaderPath, 'utf8');
    const lifecycle = await readGeneratedNodeLifecycle();
    const start = source.indexOf(nodeInitializationCleanupStart);
    const end = source.indexOf(nodeInitializationCleanupEnd, start);
    const cleanup = source.slice(start + nodeInitializationCleanupStart.length, end);
    const existingCause = new Error('existing primary cause');
    const primaryError = new Error('Node initialization failed', {
      cause: existingCause,
    });
    const removalError = new Error('beforeExit listener removal failed');
    const queuedMicrotasks: Array<() => void> = [];
    const beforeExitListeners: Array<() => void> = [];
    let resolveCleanup!: () => void;
    const cleanupResult = new Promise<void>((resolve) => {
      resolveCleanup = resolve;
    });

    expect(() =>
      runInNewContext(
        `let __emnapiContext = {
  destroy() {
    return __cleanupResult
  },
}
let __emnapiContextDestroyed = false
let __emnapiContextDestroying = false
let __emnapiContextDestroyPromise
let __emnapiContextRegisteredForBeforeExit = false
let __emnapiContextRegisteredForExit = false
let __emnapiContextBeforeExitRegistrationRetryCount = 0
let __emnapiContextBeforeExitRegistrationRetryScheduled = false
let __napiInstance
let __emnapiWasmEnvCleanupPrepared = false
${lifecycle}
__registerEmnapiContextBeforeExit()
try {
  throw __primaryError
${cleanup}`,
        {
          __cleanupResult: cleanupResult,
          __primaryError: primaryError,
          process: {
            once(event: string, listener: () => void) {
              if (event === 'beforeExit') beforeExitListeners.push(listener);
            },
            removeListener() {
              throw removalError;
            },
          },
          queueMicrotask(callback: () => void) {
            queuedMicrotasks.push(callback);
          },
        },
      ),
    ).toThrow(primaryError);

    resolveCleanup();
    await vi.waitFor(() => {
      expect(queuedMicrotasks).toHaveLength(1);
    });
    expect(primaryError.cause).toBe(existingCause);
    expect(beforeExitListeners).toHaveLength(1);
    expect(() => queuedMicrotasks[0]()).toThrow(removalError);
  });

  test('aggregates persistent generated Node host and context cleanup failures', async () => {
    const source = await readFile(cjsLoaderPath, 'utf8');
    const start = source.indexOf(nodeInitializationCleanupStart);
    const end = source.indexOf(nodeInitializationCleanupEnd, start);
    const cleanup = source.slice(start + nodeInitializationCleanupStart.length, end);
    const primaryError = new Error('Node initialization failed');
    const contextCleanupErrors = [
      new Error('Node context cleanup failed once'),
      new Error('Node context cleanup failed twice'),
    ];
    const timerCleanupErrors = [
      new Error('Node timer-host cleanup failed once'),
      new Error('Node timer-host cleanup failed twice'),
    ];
    const taskRegistration = { high: 0x1234_5678, low: 0x9abc_def0 };
    const timerRegistration = { high: 0x0fed_cba9, low: 0x8765_4321 };
    const operations: string[] = [];
    const unregisterTimerHost = vi.fn(() => {
      operations.push('timer');
      throw timerCleanupErrors[unregisterTimerHost.mock.calls.length - 1];
    });
    const unregisterCurrentThreadTaskHost = vi.fn(() => {
      operations.push('task');
    });
    const destroyEmnapiContext = vi.fn(() => {
      operations.push('context');
      throw contextCleanupErrors[destroyEmnapiContext.mock.calls.length - 1];
    });
    const removeEmnapiContextCleanupListeners = vi.fn();
    const retainEmnapiContextCleanupListener = vi.fn();
    let failure: unknown;

    try {
      runInNewContext(
        `try {
  throw __primaryError
${cleanup}`,
        {
          __attachCleanupError: vi.fn(),
          __destroyEmnapiContext: destroyEmnapiContext,
          __emnapiContextRegisteredForBeforeExit: true,
          __napiModule: {
            exports: {
              unregisterCurrentThreadTaskHost,
              unregisterTimerHost,
            },
          },
          __nodeTaskHostRegistration: taskRegistration,
          __nodeTimerHostRegistration: timerRegistration,
          __removeEmnapiContextCleanupListeners: removeEmnapiContextCleanupListeners,
          __retainEmnapiContextCleanupListener: retainEmnapiContextCleanupListener,
          __primaryError: primaryError,
        },
      );
    } catch (error) {
      failure = error;
    }
    expect(operations).toEqual(['timer', 'timer', 'task', 'context', 'context']);
    expect(unregisterTimerHost).toHaveBeenCalledTimes(2);
    expect(unregisterCurrentThreadTaskHost).toHaveBeenCalledWith(
      taskRegistration.high,
      taskRegistration.low,
    );
    expect(destroyEmnapiContext).toHaveBeenCalledTimes(2);
    expect(removeEmnapiContextCleanupListeners).not.toHaveBeenCalled();
    expect(retainEmnapiContextCleanupListener).toHaveBeenCalledOnce();
    expect(failure).toMatchObject({
      cause: primaryError,
      errors: [
        primaryError,
        expect.objectContaining({
          message: 'Threadless Node initialization cleanup failed',
          errors: [
            expect.objectContaining({
              message: 'Threadless Node timer-host cleanup failed',
              errors: timerCleanupErrors,
            }),
            expect.objectContaining({
              message: 'Threadless Node initialization context cleanup failed',
              errors: contextCleanupErrors,
            }),
          ],
        }),
      ],
      message: 'Threadless Node initialization failed and cleanup did not complete',
    });
  });

  test.each([
    {
      name: 'a primitive primary failure',
      createPrimaryError: () => 'primitive Node initialization failure',
    },
    {
      name: 'an occupied primary cause',
      createPrimaryError: () =>
        new Error('Node initialization failed', {
          cause: new Error('existing primary cause'),
        }),
    },
  ])('retains generated Node cleanup diagnostics for $name', async ({ createPrimaryError }) => {
    const source = await readFile(cjsLoaderPath, 'utf8');
    const start = source.indexOf(nodeInitializationCleanupStart);
    const end = source.indexOf(nodeInitializationCleanupEnd, start);
    const cleanup = source.slice(start + nodeInitializationCleanupStart.length, end);
    const primaryError: unknown = createPrimaryError();
    const cleanupErrors = [
      new Error('Node context cleanup failed once'),
      new Error('Node context cleanup failed twice'),
    ];
    const destroyEmnapiContext = vi.fn(() => {
      throw cleanupErrors[destroyEmnapiContext.mock.calls.length - 1];
    });
    const removeEmnapiContextCleanupListeners = vi.fn();
    const retainEmnapiContextCleanupListener = vi.fn();
    let failure: unknown;

    try {
      runInNewContext(
        `try {
  throw __primaryError
${cleanup}`,
        {
          __attachCleanupError: vi.fn(),
          __destroyEmnapiContext: destroyEmnapiContext,
          __emnapiContextRegisteredForBeforeExit: true,
          __removeEmnapiContextCleanupListeners: removeEmnapiContextCleanupListeners,
          __retainEmnapiContextCleanupListener: retainEmnapiContextCleanupListener,
          __primaryError: primaryError,
        },
      );
    } catch (error) {
      failure = error;
    }

    expect(removeEmnapiContextCleanupListeners).not.toHaveBeenCalled();
    expect(retainEmnapiContextCleanupListener).toHaveBeenCalledOnce();
    expect(failure).toMatchObject({
      cause: primaryError,
      errors: [
        primaryError,
        expect.objectContaining({
          message: 'Threadless Node initialization cleanup failed',
          errors: [
            expect.objectContaining({
              message: 'Threadless Node initialization context cleanup failed',
              errors: cleanupErrors,
            }),
          ],
        }),
      ],
    });
    if (primaryError instanceof Error) {
      expect(primaryError.cause).toMatchObject({ message: 'existing primary cause' });
    }
  });

  test('generated root timer hosts preserve relay semantics across long delays and failures', async () => {
    const bootstrap = await readCurrentThreadHostBootstrap(cjsLoaderPath);
    const schedulerError = new Error('timer scheduling failed');
    const callbacks = new Map<number, () => void>();
    const clearedHandles: number[] = [];
    const scheduledDelays: number[] = [];
    let nextHandle = 0;
    let failScheduling = false;
    let schedule: ((id: number, ms: number) => Promise<void>) | undefined;
    let cancel: ((id: number) => void) | undefined;
    const reservations = [
      { high: 0, low: 1 },
      { high: 0, low: 2 },
    ];
    const live = new Set<number>();
    const binding = {
      getCurrentThreadTaskHostContractVersion: () => 4,
      isCurrentThreadHostRegistrationActive: (_high: number, low: number) => live.has(low),
      reserveCurrentThreadHostRegistration: () => reservations.shift(),
      registerCurrentThreadTaskHost: vi.fn((_high: number, low: number) => {
        live.add(low);
      }),
      registerTimerHost(
        _high: number,
        low: number,
        scheduleCallback: (id: number, ms: number) => Promise<void>,
        cancelCallback: (id: number) => void,
      ) {
        schedule = scheduleCallback;
        cancel = cancelCallback;
        live.add(low);
      },
      unregisterCurrentThreadTaskHost: vi.fn(),
      unregisterTimerHost: vi.fn(),
    };
    runCurrentThreadHostBootstrap(bootstrap, binding, {
      setTimeout(callback: () => void, ms: number) {
        if (failScheduling) {
          failScheduling = false;
          throw schedulerError;
        }
        scheduledDelays.push(ms);
        nextHandle += 1;
        const handle = nextHandle;
        callbacks.set(handle, () => {
          callbacks.delete(handle);
          callback();
        });
        return handle;
      },
      clearTimeout(handle: number) {
        clearedHandles.push(handle);
        callbacks.delete(handle);
      },
    });

    expect(schedule).toBeTypeOf('function');
    expect(cancel).toBeTypeOf('function');
    let firstResolved = false;
    const first = schedule!(7, 10_000).then(() => {
      firstResolved = true;
    });
    const replacement = schedule!(7, 20_000);
    await first;
    expect(firstResolved).toBe(true);
    expect(clearedHandles).toEqual([1]);
    expect(callbacks.has(1)).toBe(false);

    callbacks.get(2)?.();
    await replacement;
    expect(callbacks.has(2)).toBe(false);

    failScheduling = true;
    await expect(schedule!(8, 30_000)).rejects.toBe(schedulerError);
    const recovered = schedule!(8, 40_000);
    cancel!(8);
    await recovered;
    expect(clearedHandles).toEqual([1, 3]);

    const maxHostTimeoutMs = 2_147_483_647;
    const longRelay = schedule!(9, maxHostTimeoutMs + 25);
    const firstLongHandle = nextHandle;
    expect(scheduledDelays.at(-1)).toBe(maxHostTimeoutMs);
    callbacks.get(firstLongHandle)?.();
    const finalLongHandle = nextHandle;
    expect(finalLongHandle).toBe(firstLongHandle + 1);
    expect(scheduledDelays.at(-1)).toBe(25);
    callbacks.get(finalLongHandle)?.();
    await longRelay;

    const chainedFailureRelay = schedule!(10, maxHostTimeoutMs + 1);
    const chainedFailureHandle = nextHandle;
    failScheduling = true;
    const chainedFailure = expect(chainedFailureRelay).rejects.toBe(schedulerError);
    callbacks.get(chainedFailureHandle)?.();
    await chainedFailure;
    expect(callbacks.size).toBe(0);
  });

  test('clears and resolves cancelled host timer relays', async () => {
    vi.useFakeTimers();
    try {
      const registerTimerHost = vi.fn();
      const dispose = registerWorkerdTimerHost(createWorkerdTimerHostBinding(registerTimerHost));
      expect(registerTimerHost).toHaveBeenCalledOnce();

      const [schedule, cancel] = registerTimerHost.mock.calls[0] as [
        (idOrMs: number, ms?: number) => Promise<void>,
        (id: number) => void,
      ];
      let replacedResolved = false;
      const replacedRelay = schedule(7, 60_000).then(() => {
        replacedResolved = true;
      });
      const relay = schedule(7, 120_000);
      await replacedRelay;
      expect(replacedResolved).toBe(true);
      expect(vi.getTimerCount()).toBe(1);

      let resolved = false;
      void relay.then(() => {
        resolved = true;
      });

      await vi.advanceTimersByTimeAsync(1);
      expect(resolved).toBe(false);
      cancel(7);
      await relay;
      expect(resolved).toBe(true);
      expect(vi.getTimerCount()).toBe(0);

      const legacyRelay = schedule(25);
      await vi.advanceTimersByTimeAsync(25);
      await legacyRelay;
      dispose();
    } finally {
      vi.useRealTimers();
    }
  });

  test('splits long managed and legacy host delays into bounded chunks', async () => {
    vi.useFakeTimers();
    try {
      const registerTimerHost = vi.fn();
      const dispose = registerWorkerdTimerHost(createWorkerdTimerHostBinding(registerTimerHost));
      const [schedule] = registerTimerHost.mock.calls[0] as [
        (idOrMs: number, ms?: number) => Promise<void>,
      ];
      const maxHostTimeoutMs = 2_147_483_647;
      let managedSettled = false;
      let legacySettled = false;
      const managedRelay = schedule(7, maxHostTimeoutMs + 25).then(() => {
        managedSettled = true;
      });
      const legacyRelay = schedule(maxHostTimeoutMs + 10).then(() => {
        legacySettled = true;
      });

      await vi.advanceTimersByTimeAsync(maxHostTimeoutMs);
      expect(managedSettled).toBe(false);
      expect(legacySettled).toBe(false);
      expect(vi.getTimerCount()).toBe(2);

      await vi.advanceTimersByTimeAsync(10);
      expect(legacySettled).toBe(true);
      expect(managedSettled).toBe(false);

      await vi.advanceTimersByTimeAsync(15);
      await Promise.all([managedRelay, legacyRelay]);
      expect(managedSettled).toBe(true);
      expect(vi.getTimerCount()).toBe(0);
      dispose();
    } finally {
      vi.useRealTimers();
    }
  });

  test('rejects managed relays when initial or chained timers cannot be armed', async () => {
    vi.useFakeTimers();
    const setTimeoutHost = globalThis.setTimeout.bind(globalThis);
    const schedulerError = new Error('managed setTimeout failed');
    let failNextArm = true;
    vi.stubGlobal('setTimeout', ((callback: TimerHandler, ms?: number, ...args: unknown[]) => {
      if (failNextArm) {
        failNextArm = false;
        throw schedulerError;
      }
      return setTimeoutHost(callback, ms, ...args);
    }) as typeof setTimeout);
    try {
      const registerTimerHost = vi.fn();
      const dispose = registerWorkerdTimerHost(createWorkerdTimerHostBinding(registerTimerHost));
      const [schedule] = registerTimerHost.mock.calls[0] as unknown as [
        (id: number, ms: number) => Promise<void>,
      ];
      const maxHostTimeoutMs = 2_147_483_647;
      await expect(schedule(9, 1)).rejects.toBe(schedulerError);

      const relay = schedule(9, maxHostTimeoutMs + 1);
      const rejection = expect(relay).rejects.toBe(schedulerError);

      failNextArm = true;
      await vi.advanceTimersByTimeAsync(maxHostTimeoutMs);
      await rejection;
      expect(vi.getTimerCount()).toBe(0);
      dispose();
    } finally {
      vi.unstubAllGlobals();
      vi.useRealTimers();
    }
  });

  test('disposes pending host timer relays', async () => {
    vi.useFakeTimers();
    try {
      const registerTimerHost = vi.fn();
      const dispose = registerWorkerdTimerHost(createWorkerdTimerHostBinding(registerTimerHost));
      const [schedule] = registerTimerHost.mock.calls[0] as [
        (idOrMs: number, ms?: number) => Promise<void>,
      ];
      let resolved = 0;
      const relays = [
        schedule(7, 60_000).then(() => {
          resolved += 1;
        }),
        schedule(8, 120_000).then(() => {
          resolved += 1;
        }),
        schedule(180_000).then(() => {
          resolved += 1;
        }),
      ];

      expect(vi.getTimerCount()).toBe(3);
      dispose();
      dispose();
      await Promise.all(relays);
      expect(resolved).toBe(3);
      expect(vi.getTimerCount()).toBe(0);

      await schedule(9, 60_000);
      expect(vi.getTimerCount()).toBe(0);
    } finally {
      vi.useRealTimers();
    }
  });

  test('rejects the v1 managed timer-host ABI before registration', () => {
    const registerTimerHost = vi.fn();
    expect(() =>
      registerWorkerdTimerHost({
        getCurrentThreadTaskHostContractVersion: () => 1,
        isCurrentThreadHostRegistrationActive: vi.fn(),
        registerTimerHost,
        reserveCurrentThreadHostRegistration: vi.fn(),
        unregisterTimerHost: vi.fn(),
      }),
    ).toThrow(/contract version 1.*version 4/);
    expect(registerTimerHost).not.toHaveBeenCalled();
  });

  test('rejects and rolls back an inactive v4 managed timer-host registration', () => {
    const registration = { high: 0, low: 1 };
    const registerTimerHost = vi.fn();
    const unregisterTimerHost = vi.fn();
    expect(() =>
      registerWorkerdTimerHost({
        getCurrentThreadTaskHostContractVersion: () => 4,
        isCurrentThreadHostRegistrationActive: () => false,
        registerTimerHost,
        reserveCurrentThreadHostRegistration: () => registration,
        unregisterTimerHost,
      }),
    ).toThrow(/inactive timer host registration/);
    expect(registerTimerHost).toHaveBeenCalledWith(
      registration.high,
      registration.low,
      expect.any(Function),
      expect.any(Function),
    );
    expect(unregisterTimerHost).toHaveBeenCalledWith(registration.high, registration.low);
  });

  test('retries exact timer-host unregistration before disposing pending relays', async () => {
    vi.useFakeTimers();
    try {
      const registration = { high: 0x1234_5678, low: 0x9abc_def0 };
      const unregisterError = new Error('timer host unregister failed');
      const unregisterTimerHost = vi
        .fn()
        .mockImplementationOnce(() => {
          throw unregisterError;
        })
        .mockImplementationOnce(() => {});
      const registerTimerHost = vi.fn();
      const dispose = registerWorkerdTimerHost(
        createWorkerdTimerHostBinding(registerTimerHost, {
          registration,
          unregisterTimerHost,
        }),
      );
      expect(registerTimerHost).toHaveBeenCalledWith(expect.any(Function), expect.any(Function));

      const [schedule] = registerTimerHost.mock.calls[0] as unknown as [
        (id: number, ms: number) => Promise<void>,
      ];
      const relay = schedule(7, 60_000);
      expect(vi.getTimerCount()).toBe(1);

      expect(dispose).toThrow(unregisterError);
      expect(vi.getTimerCount()).toBe(1);

      expect(dispose).not.toThrow();
      await relay;
      expect(unregisterTimerHost).toHaveBeenNthCalledWith(2, registration.high, registration.low);
      expect(vi.getTimerCount()).toBe(0);

      dispose();
      expect(unregisterTimerHost).toHaveBeenCalledTimes(2);
    } finally {
      vi.useRealTimers();
    }
  });

  test('resolves a cancelled timer relay when host cancellation throws', async () => {
    vi.useFakeTimers();
    const clearTimeoutHost = globalThis.clearTimeout;
    vi.stubGlobal('clearTimeout', (handle: ReturnType<typeof setTimeout>) => {
      clearTimeoutHost(handle);
      throw new Error('host cancellation failed');
    });
    try {
      const registerTimerHost = vi.fn();
      const dispose = registerWorkerdTimerHost(createWorkerdTimerHostBinding(registerTimerHost));
      const [schedule, cancel] = registerTimerHost.mock.calls[0] as [
        (id: number, ms: number) => Promise<void>,
        (id: number) => void,
      ];
      const relay = schedule(7, 60_000);

      expect(() => cancel(7)).not.toThrow();
      await relay;
      expect(vi.getTimerCount()).toBe(0);
      dispose();
    } finally {
      vi.unstubAllGlobals();
      vi.useRealTimers();
    }
  });

  test('generated root timer hosts contain cancellation failures and retire relays', async () => {
    const bootstrap = await readCurrentThreadHostBootstrap(cjsLoaderPath);
    const callbacks = new Map<number, () => void>();
    let schedule: ((id: number, ms: number) => Promise<void>) | undefined;
    let cancel: ((id: number) => void) | undefined;
    const registrations = [
      { high: 0, low: 1 },
      { high: 0, low: 2 },
    ];
    const live = new Set<number>();
    const binding = {
      getCurrentThreadTaskHostContractVersion: () => 4,
      isCurrentThreadHostRegistrationActive: (_high: number, low: number) => live.has(low),
      registerCurrentThreadTaskHost: vi.fn((_high: number, low: number) => {
        live.add(low);
      }),
      registerTimerHost(
        _high: number,
        low: number,
        scheduleCallback: (id: number, ms: number) => Promise<void>,
        cancelCallback: (id: number) => void,
      ) {
        live.add(low);
        schedule = scheduleCallback;
        cancel = cancelCallback;
      },
      reserveCurrentThreadHostRegistration: () => registrations.shift(),
      unregisterCurrentThreadTaskHost: vi.fn(),
      unregisterTimerHost: vi.fn(),
    };
    runCurrentThreadHostBootstrap(bootstrap, binding, {
      setTimeout(callback: () => void) {
        const handle = callbacks.size + 1;
        callbacks.set(handle, callback);
        return handle;
      },
      clearTimeout(handle: number) {
        callbacks.delete(handle);
        throw new Error('root timer cancellation failed');
      },
    });

    const relay = schedule!(7, 60_000);
    expect(() => cancel!(7)).not.toThrow();
    await relay;
    expect(callbacks.size).toBe(0);
  });
});
