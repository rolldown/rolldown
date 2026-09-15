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
import { preserveGeneratedBindingSources } from '../build-binding-guards';
// @ts-ignore This focused build-codegen test intentionally reaches package tooling outside the test rootDir.
import { preserveInactiveWasiDeclaration } from '../build-binding-guards';
// @ts-ignore This focused unit test intentionally reaches generated package source outside the test rootDir.
import type { WasiInstance } from '../src/rolldown-binding.wasip1-deferred.js';
// @ts-ignore This focused integration test intentionally reaches the package source outside the test rootDir.
import * as workerd from '../src/workerd';
// @ts-ignore This focused unit test intentionally reaches the package source outside the test rootDir.
import {
  claimManagedMemoryForAttempt,
  createManagedInstance,
} from '../src/workerd-managed-instance';
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
const managedInstancePath = new URL('../src/workerd-managed-instance.ts', import.meta.url);
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

// The facade is typed against the real binding surface; these focused tests
// drive it with purpose-built stand-in classes instead.
type ManagedStub = Omit<workerd.WorkerdRolldownInstance, 'exports'> & {
  // oxlint-disable-next-line typescript/no-explicit-any -- stand-in binding shapes
  exports: any;
};

/**
 * A stand-in for one instance of the deferred loader `@napi-rs/cli` generates:
 * the same handle shape, with a caller-supplied teardown. A rejected teardown
 * leaves the stub undisposed, exactly like the real loader's retryable
 * `dispose()`.
 */
function createStubDeferredInstance(
  rawBinding: object,
  dispose: () => void | Promise<void>,
): WasiInstance {
  const memory = new WebAssembly.Memory({ initial: 1, maximum: 1 });
  let disposed = false;
  return {
    exports: rawBinding,
    get memory() {
      return memory;
    },
    get memoryBytes() {
      return disposed ? 0 : memory.buffer.byteLength;
    },
    get disposed() {
      return disposed;
    },
    async dispose() {
      await dispose();
      disposed = true;
    },
  } as unknown as WasiInstance;
}

async function createManagedStub(
  rawBinding: object,
  dispose: () => void | Promise<void> = () => {},
): Promise<ManagedStub> {
  return (await createManagedInstance(
    createStubDeferredInstance(rawBinding, dispose),
  )) as unknown as ManagedStub;
}

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

  test('does not consume caller memory before the module promise settles', async () => {
    const before = getWorkerdRuntimeStats();
    const memory = new WebAssembly.Memory({ initial: 1, maximum: 1 });
    const moduleError = new Error('module resolution failed');
    let rejectModule!: (error: unknown) => void;
    const module = new Promise<WebAssembly.Module>((_resolve, reject) => {
      rejectModule = reject;
    });

    const initialization = createInstance(module, { memory });
    await Promise.resolve();
    expect(getWorkerdRuntimeStats()).toEqual(before);

    rejectModule(moduleError);
    await expect(initialization).rejects.toBe(moduleError);
    expect(getWorkerdRuntimeStats()).toEqual(before);
    // The module was validated before the option bag could be consumed, so the
    // caller's memory is still usable for a corrected call.
    expect(() => claimManagedMemoryForAttempt(memory)).not.toThrow();
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

      await first.dispose();
      await first.dispose();
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

      await second.dispose();
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
    const rawBinding: Record<string, unknown> = {
      BindingBundler,
      getRuntimeCapabilities,
    };
    for (const privateHostExport of privateManagedHostExports) {
      rawBinding[privateHostExport] = vi.fn();
    }
    const destroy = vi.fn();
    const instance = await createManagedStub(rawBinding, destroy);
    const binding = instance.exports;

    for (const privateHostExport of privateManagedHostExports) {
      expect(workerd).not.toHaveProperty(privateHostExport);
      expect(binding).not.toHaveProperty(privateHostExport);
      // Hidden by projection, never deleted: the loader keeps its own host
      // controls on the raw exports object it owns.
      expect(rawBinding).toHaveProperty(privateHostExport);
    }

    const RetainedBundler = binding.BindingBundler;
    const retainedCapabilities = binding.getRuntimeCapabilities;
    const bundler = new RetainedBundler();
    const generation = bundler.generate();

    await expect(instance.dispose()).rejects.toThrow(
      /1 active binding operation and 1 open binding object/,
    );
    expect(destroy).not.toHaveBeenCalled();
    expect(instance.disposed).toBe(false);

    finishGenerate('generated');
    await expect(generation).resolves.toBe('generated');
    await expect(instance.dispose()).rejects.toThrow(/1 open binding object/);
    expect(destroy).not.toHaveBeenCalled();

    await bundler.close();
    await instance.dispose();
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
    const destroy = vi.fn();
    const instance = await createManagedStub(rawBinding, destroy);
    const bundler = new instance.exports.BindingBundler();

    await expect(bundler.close()).rejects.toBe(retryableCloseError);
    await expect(instance.dispose()).rejects.toThrow(/1 open binding object/);

    terminal = true;
    await expect(bundler.close()).rejects.toBe(terminalCloseError);
    await instance.dispose();

    expect(destroy).toHaveBeenCalledOnce();
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
    const destroy = vi.fn();
    const instance = await createManagedStub(rawBinding, destroy);
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

    await expect(instance.dispose()).rejects.toThrow(/1 open binding object/);
    const retainedClose = closable.close;
    await retainedClose();
    expect(closeCalls).toBe(1);

    await instance.dispose();
    expect(destroy).toHaveBeenCalledOnce();
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
    const destroy = vi.fn();
    const instance = await createManagedStub(rawBinding, destroy);
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
    await expect(instance.dispose()).rejects.toThrow(/2 open binding objects/);

    await bundler.close();
    await derived.close();
    expect(close).toHaveBeenCalledTimes(2);
    await instance.dispose();
    expect(destroy).toHaveBeenCalledOnce();
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
    const destroy = vi.fn();
    const instance = await createManagedStub(rawBinding, destroy);
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
    await instance.dispose();
    expect(destroy).toHaveBeenCalledOnce();
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
    const destroy = vi.fn();
    const instance = await createManagedStub(rawBinding, destroy);
    const binding = instance.exports;
    const resources = [
      new binding.BindingBundler(),
      new binding.BindingDevEngine(),
      new binding.BindingWatcher(),
      new binding.BindingWatcherBundler(),
      new binding.TraceSubscriberGuard(),
    ];

    await expect(instance.dispose()).rejects.toThrow(/5 open binding objects/);
    await resources[0].close.call(resources[1]);
    await resources[0].close();
    await Promise.all(resources.slice(2).map((resource) => resource.close()));
    for (const resource of resources.slice(2)) {
      await resource.close();
    }
    await instance.dispose();
    expect(destroy).toHaveBeenCalledOnce();
  });

  test('reads and calls custom thenables once while refusing reentrant disposal', async () => {
    let instance!: ManagedStub;
    let getterCalls = 0;
    let thenCalls = 0;
    let disposalAttempt: Promise<void> | undefined;
    class BindingBundler {
      generate() {
        return {
          // oxlint-disable-next-line unicorn/no-thenable -- exercises one-shot custom thenable assimilation
          get then() {
            getterCalls += 1;
            // dispose() is a promise API: a refusal arrives as a rejection, so
            // capture it here and assert once the call it fired from settles.
            disposalAttempt = instance.dispose();
            return (resolve: (value: string) => void) => {
              thenCalls += 1;
              resolve('generated');
            };
          },
        };
      }

      close(): void {}
    }
    const destroy = vi.fn();
    instance = await createManagedStub({ BindingBundler }, destroy);
    const bundler = new instance.exports.BindingBundler();

    await expect((bundler as { generate(): Promise<string> }).generate()).resolves.toBe(
      'generated',
    );
    expect(getterCalls).toBe(1);
    expect(thenCalls).toBe(1);
    await expect(disposalAttempt).rejects.toThrow(
      /1 active binding operation and 1 open binding object/,
    );
    expect(destroy).not.toHaveBeenCalled();
    expect(instance.disposed).toBe(false);

    bundler.close();
    await instance.dispose();
    expect(destroy).toHaveBeenCalledOnce();
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
    const destroy = vi.fn();
    const instance = await createManagedStub(rawBinding, destroy);
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
    await instance.dispose();
    expect(destroy).toHaveBeenCalledOnce();
  });

  test('releases a resolved close thenable before its later user microtasks', async () => {
    let instance!: ManagedStub;
    let disposal: Promise<void> | undefined;
    class BindingResource {
      close() {
        return {
          // oxlint-disable-next-line unicorn/no-thenable -- verifies resolving-function job order
          then(resolve: () => void) {
            resolve();
            queueMicrotask(() => {
              disposal = instance.dispose();
            });
          },
        };
      }
    }
    const destroy = vi.fn();
    instance = await createManagedStub({ BindingResource }, destroy);
    const resource = new instance.exports.BindingResource();

    await resource.close();
    // The close barrier ran before that later microtask, so the disposal it
    // started was accepted rather than refused for an open binding object.
    expect(disposal).toBeDefined();
    await expect(disposal).resolves.toBeUndefined();
    expect(instance.disposed).toBe(true);
    expect(destroy).toHaveBeenCalledOnce();
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
    const destroy = vi.fn();
    const instance = await createManagedStub(rawBinding, destroy);
    const invoker = new instance.exports.BindingInvoker();

    publicPromise = (invoker as unknown as { invoke(): Promise<unknown> }).invoke();
    await expect(publicPromise).rejects.toThrow(
      /Thenable cycle detected while settling a managed workerd call/,
    );

    invoker.close();
    await instance.dispose();
    expect(destroy).toHaveBeenCalledOnce();
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
    const destroy = vi.fn();
    const instance = await createManagedStub(rawBinding, destroy);

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
      await instance.dispose();
    }
    expect(destroy).toHaveBeenCalledOnce();
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
    const destroy = vi.fn();
    const instance = await createManagedStub(rawBinding, destroy);

    try {
      expect(new instance.exports.BindingInvoker().accept({ buffers })).toEqual([1, 2, 3, 4, 5, 6]);
      expect(receivedBuffers).not.toBe(buffers);
      expect(receivedBuffers).toHaveLength(buffers.length);
      for (const [index, buffer] of buffers.entries()) {
        expect(receivedBuffers?.[index]).toBe(buffer);
        expect(readByteLength(receivedBuffers?.[index])).toBe(index + 1);
      }
    } finally {
      await instance.dispose();
    }
    expect(destroy).toHaveBeenCalledOnce();
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
    const destroy = vi.fn();
    const instance = await createManagedStub(rawBinding, destroy);
    const plugin = new DatePlugin(123);
    const invoker = new instance.exports.BindingInvoker();

    invoker.invoke(plugin);
    expect(plugin.observedTime).toBe(123);
    expect(retainedContext?.getModuleIds()).toEqual(['virtual:entry']);

    await instance.dispose();
    expect(destroy).toHaveBeenCalledOnce();
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
    const destroy = vi.fn();
    const instance = await createManagedStub(rawBinding, destroy);
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
    await expect(instance.dispose()).rejects.toThrow(/3 open binding objects/);

    bundler.close();
    watcher.close();
    retainedWatcherBundler?.close();
    await instance.dispose();
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

      await instance.dispose();
    },
  );

  test(
    'allows dropped binding objects and repeated closed objects to be collected',
    { timeout: 30_000 },
    () => {
      // `--import` parses its value as a URL first, so a bare Windows absolute path
      // is read as the `d:` scheme and rejected. Always hand Node a file:// URL.
      const tsxLoaderUrl = pathToFileURL(createRequire(import.meta.url).resolve('tsx')).href;
      const child = spawnSync(
        process.execPath,
        [
          '--expose-gc',
          '--import',
          tsxLoaderUrl,
          '--input-type=module',
          '--eval',
          `
import assert from 'node:assert/strict'

const { createManagedInstance } = await import(${JSON.stringify(managedInstancePath.href)})
class BindingBundler {
  close() {}
}
const createStubDeferredInstance = () => {
  const memory = new WebAssembly.Memory({ initial: 1, maximum: 1 })
  let disposed = false
  return {
    exports: { BindingBundler },
    get memory() {
      return memory
    },
    get memoryBytes() {
      return disposed ? 0 : memory.buffer.byteLength
    },
    get disposed() {
      return disposed
    },
    async dispose() {
      disposed = true
    },
  }
}
const instance = await createManagedInstance(createStubDeferredInstance())
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
  await instance.dispose().catch((error) => {
    assert.match(error.message, /open binding object/)
  })
}
assert.equal(instance.disposed, true)

const second = await createManagedInstance(createStubDeferredInstance())
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
await second.dispose()
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

  test(
    'releases the deferred instance so a disposed handle retains no memory',
    { timeout: 30_000 },
    () => {
      // Regression: every method on the managed handle shares one closure, so a
      // captured deferred instance is retained for as long as a consumer keeps
      // the handle -- which kept the raw binding exports and the whole
      // WebAssembly.Memory alive while `memoryBytes` already reported 0.
      // A WebAssembly.Memory only becomes unreachable under an async major GC,
      // so the loop below asks for that shape explicitly.
      const tsxLoaderUrl = pathToFileURL(createRequire(import.meta.url).resolve('tsx')).href;
      const child = spawnSync(
        process.execPath,
        [
          '--expose-gc',
          '--import',
          tsxLoaderUrl,
          '--input-type=module',
          '--eval',
          `
import assert from 'node:assert/strict'

const { createManagedInstance } = await import(${JSON.stringify(managedInstancePath.href)})
class BindingBundler {
  close() {}
}
let memoryRef
let rawExportsRef
const createStubDeferredInstance = () => {
  const memory = new WebAssembly.Memory({ initial: 1, maximum: 1 })
  const exports = { BindingBundler }
  memoryRef = new WeakRef(memory)
  rawExportsRef = new WeakRef(exports)
  let disposed = false
  return {
    exports,
    get memory() {
      return memory
    },
    get memoryBytes() {
      return disposed ? 0 : memory.buffer.byteLength
    },
    get disposed() {
      return disposed
    },
    async dispose() {
      disposed = true
    },
  }
}

const instance = await createManagedInstance(createStubDeferredInstance())
let bundler = new instance.exports.BindingBundler()
bundler.close()
bundler = undefined
await instance.dispose()

for (let attempt = 0; attempt < 100; attempt += 1) {
  await globalThis.gc({ type: 'major', execution: 'async' })
  await new Promise(setImmediate)
  if (memoryRef.deref() === undefined && rawExportsRef.deref() === undefined) break
}

// The handle is still strongly referenced here on purpose: that is exactly the
// consumer shape that used to strand a whole instance.
assert.equal(instance.disposed, true)
assert.equal(instance.memoryBytes, 0)
assert.throws(() => instance.memory, /This workerd Rolldown instance has been disposed/)
assert.throws(() => instance.exports, /This workerd Rolldown instance has been disposed/)
await instance.dispose()
assert.equal(
  rawExportsRef.deref(),
  undefined,
  'a disposed managed handle retained the raw binding exports',
)
assert.equal(
  memoryRef.deref(),
  undefined,
  'a disposed managed handle retained the instance memory',
)
console.log('disposed managed handle released its deferred instance')
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
      expect(child.stdout).toContain('disposed managed handle released its deferred instance');
    },
  );

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
      await expect(instance.dispose()).rejects.toThrow(
        /2 active binding operations and 1 open binding object/,
      );
      expect(instance.disposed).toBe(false);

      releaseLoad();
      await expect(generation).resolves.not.toHaveProperty('isBindingErrors', true);
      await expect(instance.dispose()).rejects.toThrow(/1 open binding object/);

      await bundler.close();
      await instance.dispose();
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
      // Import the managed instance module directly: the public workerd entry
      // also pulls the high-level build() pipeline, whose legacy decorators the
      // child's tsx transform mishandles.
      const workerdUrl = managedInstancePath.href;
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
await instance.dispose()
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
      // Import the managed instance module directly: the public workerd entry
      // also pulls the high-level build() pipeline, whose legacy decorators the
      // child's tsx transform mishandles.
      const workerdUrl = managedInstancePath.href;
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
await instance.dispose()
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
            await instance.dispose();
          }
        }
        expect(instance.disposed).toBe(true);
      }
    },
  );

  // A short import chain on the raw binding surface; every module renders to a
  // non-empty source, so each `getModules()` box has data behind it.
  async function generateRawChain(
    bundler: InstanceType<workerd.WorkerdRolldownInstance['exports']['BindingBundler']>,
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
          await instance.dispose();
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
          await instance.dispose();
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

    await fulfilled[0].value.dispose();
    await expect(createInstance(module, { memory })).rejects.toThrow(/initialization attempt/);
  });

  test('coordinates memory claims across evaluated facade copies', async () => {
    // Two evaluations of the facade module, each with its own WeakSet. What
    // makes them agree is the claim pinned on the Memory itself under
    // `Symbol.for('@rolldown/browser/workerd/managed-memory-claims/v1')`.
    const [first, second] = await Promise.all([
      import(/* @vite-ignore */ `${managedInstancePath.href}?managed-memory-claims=1`),
      import(/* @vite-ignore */ `${managedInstancePath.href}?managed-memory-claims=2`),
    ]);
    expect(first.claimManagedMemoryForAttempt).not.toBe(second.claimManagedMemoryForAttempt);
    const memory = new WebAssembly.Memory({ initial: 1, maximum: 1 });

    first.claimManagedMemoryForAttempt(memory);
    expect(() => second.claimManagedMemoryForAttempt(memory)).toThrow(/initialization attempt/);
  });

  test('keeps failed disposal live and retries cleanup', async () => {
    let disposeCalls = 0;
    const cleanupError = new Error('cleanup failed');
    const instance = await createManagedStub(
      { getRuntimeCapabilities: () => ({ target: 'wasi' }) },
      () => {
        disposeCalls += 1;
        if (disposeCalls === 1) throw cleanupError;
      },
    );
    const retainedCapabilities = instance.exports.getRuntimeCapabilities;

    await expect(instance.dispose()).rejects.toBe(cleanupError);
    expect(instance.disposed).toBe(false);
    expect(instance.memoryBytes).toBeGreaterThan(0);
    expect(() => instance.exports).toThrow(/disposal has started/);
    expect(() => retainedCapabilities()).toThrow(/disposal has started/);

    await expect(instance.dispose()).resolves.toBeUndefined();
    expect(disposeCalls).toBe(2);
    expect(instance.disposed).toBe(true);
    expect(instance.memoryBytes).toBe(0);
    expect(() => retainedCapabilities()).toThrow(
      'This workerd Rolldown instance has been disposed',
    );
  });

  test('keeps a sibling instance intact across a failed disposal and its retry', async () => {
    class BindingBundler {
      close(): void {}
    }
    let secondDisposeCalls = 0;
    const firstDispose = vi.fn();
    const first = await createManagedStub({ BindingBundler }, firstDispose);
    const second = await createManagedStub({ BindingBundler }, () => {
      secondDisposeCalls += 1;
      if (secondDisposeCalls === 1) throw new Error('cleanup hook failed');
    });
    const firstBundler = new first.exports.BindingBundler();

    await expect(second.dispose()).rejects.toThrow('cleanup hook failed');
    expect(second.disposed).toBe(false);
    // Only the failing instance is quarantined. Its sibling's facade is
    // untouched, open-object accounting included.
    expect(first.exports.BindingBundler).toBeTypeOf('function');
    await expect(first.dispose()).rejects.toThrow(/1 open binding object/);
    expect(firstDispose).not.toHaveBeenCalled();

    await expect(second.dispose()).resolves.toBeUndefined();
    expect(second.disposed).toBe(true);
    expect(secondDisposeCalls).toBe(2);

    firstBundler.close();
    await first.dispose();
    expect(first.disposed).toBe(true);
    expect(firstDispose).toHaveBeenCalledOnce();
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
    await instance.dispose();
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
      await instance?.dispose();
      vi.unstubAllGlobals();
    }
  });

  wasiTest('does not retain Node beforeExit listeners after managed disposal', async () => {
    const module = await WebAssembly.compile(await readFile(wasmPath));
    const before = process.rawListeners('beforeExit').length;

    for (let index = 0; index < 3; index += 1) {
      const instance = await createInstance(module);
      await instance.dispose();
    }

    expect(process.rawListeners('beforeExit')).toHaveLength(before);
  });

  wasiTest('skips deferred emnapi TSFN drains after managed disposal', { timeout: 30_000 }, () => {
    // `--import` parses its value as a URL first, so a bare Windows absolute path
    // is read as the `d:` scheme and rejected. Always hand Node a file:// URL.
    const tsxLoaderUrl = pathToFileURL(createRequire(import.meta.url).resolve('tsx')).href;
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

const realSetImmediate = globalThis.setImmediate
const immediateQueue = []
globalThis.setImmediate = (callback, ...args) => {
  immediateQueue.push(() => callback(...args))
  return immediateQueue.length
}

try {
  // The deferred loader hands back the raw instance; the managed facade goes on
  // top of it here, so the raw binding stays reachable for the queued future
  // below while disposal still runs the managed handle's real path.
  const { createInstance: createDeferredInstance } = await import(
    ${JSON.stringify(deferredLoaderPath.href)}
  )
  const { createManagedInstance } = await import(${JSON.stringify(managedInstancePath.href)})
  const module = await WebAssembly.compile(
    await readFile(${JSON.stringify(fileURLToPath(wasmPath))}),
  )
  const deferred = await createDeferredInstance(module)
  const rawBinding = deferred.exports
  const instance = await createManagedInstance(deferred)
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

  // Disposal is asynchronous and its settlement drain schedules real
  // event-loop turns onto the stubbed queue, so pump that queue in FIFO order
  // until dispose() settles. The outer TSFN turn stays held back: it was
  // accepted before disposal started and is what queues the nested drain.
  const disposal = instance.dispose()
  let disposalSettled = false
  let disposalError
  disposal.then(
    () => {
      disposalSettled = true
    },
    (error) => {
      disposalSettled = true
      disposalError = error
    },
  )
  for (let pumped = 0; !disposalSettled; pumped += 1) {
    assert.ok(pumped < 1000, 'managed disposal did not settle')
    if (immediateQueue.length > 0) {
      immediateQueue.shift()()
    }
    await new Promise((resolve) => realSetImmediate(resolve))
  }
  if (disposalError) throw disposalError
  assert.equal(instance.disposed, true)
  const cleanupTurnCount = immediateQueue.length

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
