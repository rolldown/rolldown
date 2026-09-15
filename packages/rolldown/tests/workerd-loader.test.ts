import { spawnSync } from 'node:child_process';
import { Buffer as NodeBuffer } from 'node:buffer';
import { existsSync } from 'node:fs';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { runInNewContext } from 'node:vm';
// @ts-ignore This focused build-codegen test intentionally reaches package tooling outside the test rootDir.
import { preserveGeneratedBindingSources } from '../generate-workerd-loader';
// @ts-ignore This focused build-codegen test intentionally reaches package tooling outside the test rootDir.
import { preserveInactiveWasiDeclaration } from '../generate-workerd-loader';
// @ts-ignore This focused unit test intentionally reaches generated package source outside the test rootDir.
import type { DeferredRolldownInstance } from '../src/rolldown-binding.wasip1-deferred.js';
// @ts-ignore This focused integration test intentionally reaches the package source outside the test rootDir.
import * as workerd from '../src/workerd';
import { describe, expect, test, vi } from 'vitest';

const { createInstance, getWorkerdRuntimeStats, WORKERD_WASM_MEMORY } = workerd;

// Resolved the way the generated loaders resolve it, from the rolldown package.
const { installCurrentThreadHosts } = createRequire(
  fileURLToPath(new URL('../src/rolldown-binding.wasip1-browser.js', import.meta.url)),
)('@napi-rs/async-runtime') as {
  installCurrentThreadHosts: (
    binding: object,
    options?: { installTimerHost?: boolean },
  ) => () => void;
};

const wasmPath = new URL('../src/rolldown-binding.wasm32-wasip1.wasm', import.meta.url);
const wasiTest = test.runIf(existsSync(wasmPath));
const deferredLoaderPath = new URL('../src/rolldown-binding.wasip1-deferred.js', import.meta.url);
const browserLoaderPath = new URL('../src/rolldown-binding.wasip1-browser.js', import.meta.url);
const privateManagedHostExports = [
  'getCurrentThreadTaskHostContractVersion',
  'isCurrentThreadHostRegistrationActive',
  'registerCurrentThreadTaskHost',
  'registerTimerHost',
  'reserveCurrentThreadHostRegistration',
  'unregisterCurrentThreadTaskHost',
  'unregisterTimerHost',
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

async function loadDeferredLoaderWithDependencies(dependencies: object) {
  const source = await readFile(deferredLoaderPath, 'utf8');
  const dependencyKey = `__rolldownWorkerdLoaderTest${Date.now()}${Math.random()}`;
  const testDependencies = {
    Buffer: NodeBuffer,
    emnapiAsyncWorkPlugin: undefined,
    emnapiTSFNPlugin: undefined,
    ...dependencies,
  } as Record<PropertyKey, unknown>;
  const createContext = Reflect.get(testDependencies, 'createContext');
  if (typeof createContext === 'function') {
    Reflect.set(testDependencies, 'createContext', (...args: unknown[]) => {
      const context = Reflect.apply(createContext, dependencies, args);
      if (
        context &&
        (typeof context === 'object' || typeof context === 'function') &&
        !Reflect.has(context, 'features')
      ) {
        Reflect.set(context, 'features', {});
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
  emnapiAsyncWorkPlugin: __emnapiAsyncWorkPlugin,
  emnapiTSFNPlugin: __emnapiTSFNPlugin,
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
    /import \{[\s\S]*?\} from '@napi-rs\/wasm-runtime'\nimport \{ createContext as __emnapiCreateContext \} from '@emnapi\/runtime'\nimport \{ installCurrentThreadHosts as __installCurrentThreadHosts \} from '@napi-rs\/async-runtime'\nimport \{ memfs, Buffer \} from '@napi-rs\/wasm-runtime\/fs'\n/;
  if (!runtimeImports.test(source)) {
    throw new Error('Unable to inject generated browser loader test dependencies');
  }
  const dependencyKey = `__rolldownBrowserLoaderTest${Date.now()}${Math.random()}`;
  const testDependencies = {
    Buffer: NodeBuffer,
    // The real host protocol package the generated loader installs with.
    installCurrentThreadHosts,
    ...dependencies,
  } as Record<PropertyKey, unknown>;
  const createContext = Reflect.get(testDependencies, 'createContext');
  if (typeof createContext === 'function') {
    Reflect.set(testDependencies, 'createContext', (...args: unknown[]) => {
      const context = Reflect.apply(createContext, dependencies, args);
      if (context && (typeof context === 'object' || typeof context === 'function')) {
        if (!Reflect.has(context, 'features')) {
          Reflect.set(context, 'features', {});
        }
        if (!Reflect.has(context, 'suppressDestroy')) {
          Reflect.set(context, 'suppressDestroy', () => {});
        }
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
  emnapiAsyncWorkPlugin: __emnapiAsyncWorkPlugin,
  emnapiTSFNPlugin: __emnapiTSFNPlugin,
  fetch: __browserFetch,
  installCurrentThreadHosts: __installCurrentThreadHosts,
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
  test.each([
    {
      name: 'threadless target',
      options: { target: 'wasm32-wasip1' },
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

  test('does not create a managed context before the module promise settles', async () => {
    const createContext = vi.fn();
    const loader = await loadDeferredLoaderWithDependencies({
      createContext,
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
      features: {} as { Buffer?: typeof NodeBuffer },
      suppressDestroy() {},
      destroy() {},
    };
    const loader = await loadDeferredLoaderWithDependencies({
      createContext: () => context,
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
      expect(context.features.Buffer).toBe(NodeBuffer);
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

  test('aggregates browser host and context cleanup failures onto the primary error', async () => {
    const initializationError = new Error('browser timer host registration failed');
    const registration = { high: 0x1234_5678, low: 0x9abc_def0 };
    const prepareWasmEnvCleanup = vi.fn();
    const taskCleanupError = new Error('task host cleanup failure');
    const unregisterCurrentThreadTaskHost = vi.fn(() => {
      throw taskCleanupError;
    });
    const contextCleanupError = new Error('context cleanup failure');
    const destroy = vi.fn().mockRejectedValueOnce(contextCleanupError);

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
    ).rejects.toMatchObject({
      // `installCurrentThreadHosts` reports a rollback that did not complete as
      // an aggregate whose cause is the primary failure; the generated loader
      // then attaches its own cleanup failures to it.
      errors: [initializationError, taskCleanupError],
      cause: initializationError,
      cleanupErrors: [contextCleanupError],
    });

    expect(unregisterCurrentThreadTaskHost).toHaveBeenCalledExactlyOnceWith(
      registration.high,
      registration.low,
    );
    expect(prepareWasmEnvCleanup).toHaveBeenCalledOnce();
    // The generated rollback destroys the context exactly once; a failed
    // destroy is reported through the attached cleanup errors instead of
    // being retried.
    expect(destroy).toHaveBeenCalledOnce();
  });

  test('rolls back the exact browser task host before context destruction', async () => {
    const registrationError = new Error('browser timer host registration failed');
    const unregisterErrors = [
      new Error('browser task host cleanup failed once'),
      new Error('browser task host cleanup failed twice'),
    ];
    const destroyError = new Error('browser context cleanup failed');
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
        throw destroyError;
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
      'destroy context',
    ]);
    // The reserved timer token is rolled back exactly once even though its
    // registration threw: v4 reserves the capability before side effects, so
    // cleanup can always target the exact token.
    expect(unregisterTimerHost).toHaveBeenCalledExactlyOnceWith(
      timerRegistration.high,
      timerRegistration.low,
    );
    // The primary error stays the aggregate's cause, and the single context
    // destroy failure rides along on `cleanupErrors`.
    expect(failure).toMatchObject({
      errors: [registrationError, unregisterErrors[0]],
      cause: registrationError,
      cleanupErrors: [destroyError],
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
    // The reserved timer token is rolled back even though its registration threw.
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

  wasiTest(
    'owns independent concurrent instances with idempotent disposal',
    { timeout: 60_000 },
    async () => {
      const module = await WebAssembly.compile(await readFile(wasmPath));
      const before = getWorkerdRuntimeStats();
      const [first, second] = await Promise.all([
        createInstance(module),
        createInstance(Promise.resolve(module)),
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
const context = { features: {}, suppressDestroy() {}, destroy() {} }
globalThis[dependencyKey] = {
  Buffer,
  createContext: () => context,
  instantiateNapiModule: async () => ({
    napiModule: { exports: createRawBinding() },
  }),
  WASI: class {},
}
const transformed = source
  .replace(
    /import \\{[\\s\\S]*?\\} from '@napi-rs\\/wasm-runtime'\\nimport \\{ createContext as __emnapiCreateContext \\} from '@emnapi\\/runtime'\\n/,
    \`const {
  emnapiAsyncWorkPlugin: __emnapiAsyncWorkPlugin,
  emnapiTSFNPlugin: __emnapiTSFNPlugin,
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
      // `--import` parses its value as a URL first, so a bare Windows absolute path
      // is read as the `d:` scheme and rejected. Always hand Node a file:// URL.
      const tsxLoaderUrl = pathToFileURL(createRequire(import.meta.url).resolve('tsx')).href;
      // Import the deferred loader directly: the public workerd entry also pulls
      // the high-level build() pipeline, whose legacy decorators the child's tsx
      // transform mishandles.
      const workerdUrl = new URL('../src/rolldown-binding.wasip1-deferred.js', import.meta.url)
        .href;
      const child = spawnSync(
        process.execPath,
        [
          '--import',
          tsxLoaderUrl,
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
      // `--import` parses its value as a URL first, so a bare Windows absolute path
      // is read as the `d:` scheme and rejected. Always hand Node a file:// URL.
      const tsxLoaderUrl = pathToFileURL(createRequire(import.meta.url).resolve('tsx')).href;
      // Import the deferred loader directly: the public workerd entry also pulls
      // the high-level build() pipeline, whose legacy decorators the child's tsx
      // transform mishandles.
      const workerdUrl = new URL('../src/rolldown-binding.wasip1-deferred.js', import.meta.url)
        .href;
      const child = spawnSync(
        process.execPath,
        [
          '--import',
          tsxLoaderUrl,
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
    'keeps the measured ~64 MiB initial floor through repeated representative builds',
    { timeout: 60_000 },
    async () => {
      // 1027 pages: the wasm module's env.memory minimum as of oxc 0.146.0;
      // must stay in lockstep with napi.wasm.threadlessInitialMemory and the
      // ceiling in scripts/wasi/check-wasi-threadless.mjs.
      expect(WORKERD_WASM_MEMORY).toMatchObject({
        initialPages: 1027,
        initialBytes: 1027 * 64 * 1024,
      });

      const module = await WebAssembly.compile(await readFile(wasmPath));
      const moduleCount = 256;
      for (let round = 0; round < 3; round += 1) {
        const instance = await createInstance(module);
        expect(instance.memoryBytes).toBeGreaterThanOrEqual(1027 * 64 * 1024);
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

  // A short import chain on the raw binding surface; every module renders to a
  // non-empty source, so each `getModules()` box has data behind it.
  async function generateRawChain(
    bundler: InstanceType<DeferredRolldownInstance['exports']['BindingBundler']>,
    moduleCount: number,
  ) {
    const result = await bundler.generate({
      inputOptions: {
        input: [{ import: 'virtual:0' }],
        plugins: [
          {
            name: 'workerd-raw-chain',
            hookUsage: 11,
            resolveId(_ctx, id) {
              if (id.startsWith('virtual:')) return { id };
            },
            load(_ctx, id) {
              if (!id.startsWith('virtual:')) return;
              const index = Number(id.slice('virtual:'.length));
              const next =
                index + 1 < moduleCount
                  ? `import next from 'virtual:${index + 1}';`
                  : 'const next = 1;';
              return {
                code: `${next}\nexport function fn_${index}(a) { return a + next + ${index}; }\nexport default next + ${index};`,
              };
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
    return result;
  }

  wasiTest(
    'raw getModules() boxes outlive freeOutputs(): each is its own last native holder',
    { timeout: 60_000 },
    async () => {
      const module = await WebAssembly.compile(await readFile(wasmPath));
      const instance = await createInstance(module);
      const bundler = new instance.exports.BindingBundler();
      try {
        const result = await generateRawChain(bundler, 4);
        const chunk = result.chunks[0];
        const modules = chunk.getModules();
        expect(modules.values).toHaveLength(4);

        const report = workerd.freeOutputs(result);
        // The chunk box held the last reference to the chunk itself...
        expect(report.chunks[0]).toEqual({ freed: true });
        expect(() => chunk.getModules()).toThrowError(/Memory has been freed/);
        // ...while every module box still reads through its own reference to
        // that module's rendered source: freeOutputs() cannot see these boxes,
        // and on workerd no finalizer ever releases them.
        for (const box of modules.values) {
          expect(typeof box.code).toBe('string');
          // `freed: true` = this box was the LAST holder, so nothing but this
          // explicit call would have freed the rendered source.
          expect(box.dropInner()).toEqual({ freed: true });
          expect(box.dropInner()).toEqual({
            freed: false,
            reason: 'Memory has already been freed',
          });
        }
      } finally {
        try {
          await bundler.close();
        } finally {
          instance.dispose();
        }
      }
    },
  );

  wasiTest(
    'snapshotModules() copies raw getModules() boxes to plain data and releases every box',
    { timeout: 60_000 },
    async () => {
      const module = await WebAssembly.compile(await readFile(wasmPath));
      const instance = await createInstance(module);
      const bundler = new instance.exports.BindingBundler();
      let keys: string[];
      let snapshot: ReturnType<typeof workerd.snapshotModules>;
      try {
        const result = await generateRawChain(bundler, 4);
        const chunk = result.chunks[0];
        const modules = chunk.getModules();
        keys = [...modules.keys];
        // The documented raw-path recipe: snapshot the modules, then free the
        // output.
        snapshot = workerd.snapshotModules(modules);
        for (const box of modules.values) {
          expect(() => box.code).toThrowError(/Memory has been freed/);
          expect(box.dropInner()).toEqual({
            freed: false,
            reason: 'Memory has already been freed',
          });
        }
        expect(workerd.freeOutputs(result).chunks[0]).toEqual({ freed: true });
      } finally {
        try {
          await bundler.close();
        } finally {
          instance.dispose();
        }
      }
      // Plain JavaScript data: fully readable after the instance is gone.
      expect(instance.disposed).toBe(true);
      expect(Object.keys(snapshot).sort()).toEqual([...keys].sort());
      for (const key of keys) {
        const rendered = snapshot[key];
        expect(typeof rendered.code).toBe('string');
        expect(rendered.renderedLength).toBe(rendered.code!.length);
        expect(Array.isArray(rendered.renderedExports)).toBe(true);
      }
      expect(snapshot['virtual:0'].renderedExports).toEqual(
        expect.arrayContaining(['fn_0', 'default']),
      );
    },
  );

  test('rejects inputs that would require dynamic Wasm compilation', async () => {
    const beforeStats = getWorkerdRuntimeStats();
    const beforeListeners = process.rawListeners('beforeExit').length;

    await expect(
      createInstance(new Uint8Array([0, 97, 115, 109]) as unknown as WebAssembly.Module),
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
    await expect(createInstance(module, { memory: sharedMemory })).rejects.toThrow(
      /requires an unshared WebAssembly\.Memory/,
    );
    await expect(createInstance(module, { memory: crossRealmSharedMemory })).rejects.toThrow(
      /requires an unshared WebAssembly\.Memory/,
    );
  });

  wasiTest('rejects concurrent and sequential reuse of caller-provided memory', async () => {
    const module = await WebAssembly.compile(await readFile(wasmPath));
    const memory = new WebAssembly.Memory({
      initial: WORKERD_WASM_MEMORY.initialPages,
      maximum: WORKERD_WASM_MEMORY.maximumPages,
    });

    const concurrent = await Promise.allSettled([
      createInstance(module, { memory }),
      createInstance(module, { memory }),
    ]);
    const fulfilled = concurrent.filter(
      (result): result is PromiseFulfilledResult<Awaited<ReturnType<typeof createInstance>>> =>
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
    await expect(createInstance(module, { memory })).rejects.toThrow(/initialization attempt/);
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
      createInstance(new Uint8Array([0, 97, 115, 109]) as unknown as WebAssembly.Module, {
        memory,
      }),
    ).rejects.toThrow(/precompiled WebAssembly\.Module/);

    const instance = await createInstance(module, { memory });
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

    await expect(createInstance(incompatibleModule, { memory })).rejects.toThrow();
    expect(getWorkerdRuntimeStats()).toEqual(beforeStats);
    expect(process.rawListeners('beforeExit')).toHaveLength(beforeListeners);

    await expect(createInstance(module, { memory })).rejects.toThrow(/initialization attempt/);
    expect(getWorkerdRuntimeStats()).toEqual(beforeStats);
    expect(process.rawListeners('beforeExit')).toHaveLength(beforeListeners);
  });

  wasiTest('accepts Buffer asset inputs without a Buffer global', async () => {
    const module = await WebAssembly.compile(await readFile(wasmPath));
    vi.stubGlobal('Buffer', undefined);

    let instance: Awaited<ReturnType<typeof createInstance>> | undefined;
    try {
      instance = await createInstance(module);
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
      const instance = await createInstance(module);
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
  const [
    {
      instantiateNapiModule,
      WASI,
      emnapiAsyncWorkPlugin,
      emnapiTSFNPlugin,
    },
    { createContext },
  ] =
    await Promise.all([
      import(${JSON.stringify(wasmRuntimeUrl)}),
      import(${JSON.stringify(emnapiRuntimeUrl)}),
    ])
  const source = await readFile(${JSON.stringify(fileURLToPath(deferredLoaderPath))}, 'utf8')
  const dependencyKey = '__rolldownManagedTsfnDisposalTest'
  let rawBinding
  // Replacing the import block leaves the loader's plugin bindings undeclared, so
  // the async-work/TSFN plugins must be injected alongside the runtime; without
  // them this wasm's basic emnapi archive fails to instantiate with a LinkError
  // on napi_create_threadsafe_function.
  globalThis[dependencyKey] = {
    Buffer,
    createContext,
    emnapiAsyncWorkPlugin,
    emnapiTSFNPlugin,
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
  instantiateNapiModule: __emnapiInstantiateNapiModule,
  WASI: __WASI,
  createContext: __emnapiCreateContext,
  emnapiAsyncWorkPlugin: __emnapiAsyncWorkPlugin,
  emnapiTSFNPlugin: __emnapiTSFNPlugin,
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
});
