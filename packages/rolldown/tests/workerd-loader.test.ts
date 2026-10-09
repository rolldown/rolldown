import { spawnSync } from 'node:child_process';
import { Buffer as NodeBuffer } from 'node:buffer';
import { existsSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { WASM_MEMORY } from '../src/rolldown-binding.wasip1-deferred.js';
import * as workerd from '../src/workerd';
import { enterInstance, exitInstance, instanceExports } from '../src/workerd-managed-instance';
import { describe, expect, test, vi } from 'vitest';

const { createInstance, getWorkerdRuntimeStats } = workerd;

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
const managedInstancePath = new URL('../src/workerd-managed-instance.ts', import.meta.url);
const browserLoaderPath = new URL('../src/rolldown-binding.wasip1-browser.js', import.meta.url);

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
  test('keeps test-only runtime probes out of generated public sources', async () => {
    const [bindingSource, declarationSource] = await Promise.all([
      readFile(new URL('../src/binding.cjs', import.meta.url), 'utf8'),
      readFile(new URL('../src/binding.d.cts', import.meta.url), 'utf8'),
    ]);

    expect(bindingSource).not.toContain('__rolldownTest');
    expect(declarationSource).not.toContain('__rolldownTest');
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
    // registration threw: host contract v4 reserves it before side effects, so
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
      const callerMemory = new WebAssembly.Memory({ initial: 1, maximum: 1 });
      const [first, second] = await Promise.all([
        createInstance(module),
        // Not an option of this entry: a caller Memory never reaches the loader.
        createInstance(Promise.resolve(module), { memory: callerMemory } as never),
      ]);

      expect(first.memory).not.toBe(second.memory);
      expect(second.memory).not.toBe(callerMemory);
      expect(first.memoryBytes).toBeGreaterThanOrEqual(WASM_MEMORY.initialBytes);
      expect(instanceExports(first).getRuntimeCapabilities()).toMatchObject({
        target: 'wasi',
        flavor: 'CurrentThread',
        watchSupported: false,
      });
      expect(getWorkerdRuntimeStats()).toMatchObject({
        createdInstances: before.createdInstances + 2,
        liveInstances: before.liveInstances + 2,
      });

      await first.dispose();
      await first.dispose();
      expect(first.disposed).toBe(true);
      expect(first.memoryBytes).toBe(0);
      expect(() => instanceExports(first)).toThrow(
        'This workerd Rolldown instance has been disposed',
      );
      expect(() => first.memory).toThrow('This workerd Rolldown instance has been disposed');
      expect(getWorkerdRuntimeStats().liveInstances).toBe(before.liveInstances + 1);

      await second.dispose();
      expect(getWorkerdRuntimeStats().liveInstances).toBe(before.liveInstances);
    },
  );

  wasiTest(
    'counts each entered build and refuses dispose until every one exits',
    { timeout: 60_000 },
    async () => {
      const module = await WebAssembly.compile(await readFile(wasmPath));
      const [first, second] = await Promise.all([createInstance(module), createInstance(module)]);
      try {
        enterInstance(first);
        enterInstance(first);
        await expect(first.dispose()).rejects.toThrow(
          'Cannot dispose this workerd Rolldown instance with 2 active binding operations; ' +
            'await active operations and close every binding object first',
        );
        expect(first.disposed).toBe(false);
        // One active instance per module copy.
        expect(() => enterInstance(second)).toThrow(
          /Another workerd Rolldown instance is currently active/,
        );

        exitInstance(first);
        await expect(first.dispose()).rejects.toThrow(/with 1 active binding operation;/);
        exitInstance(first);
        // An exit without a matching enter changes nothing.
        exitInstance(first);

        enterInstance(second);
        exitInstance(second);
        await first.dispose();
        expect(() => enterInstance(first)).toThrow(
          'This workerd Rolldown instance has been disposed',
        );
        expect(() => enterInstance({} as never)).toThrow(TypeError);
      } finally {
        await first.dispose();
        await second.dispose();
      }
    },
  );

  test('marks disposal started before the loader dispose and keeps a failed one retryable', async () => {
    const cleanupError = new Error('cleanup failed');
    let disposeCalls = 0;
    let exportsDuringDispose: unknown;
    let instance!: any;
    let managed!: any;
    vi.resetModules();
    vi.doMock('../src/rolldown-binding.wasip1-deferred.js', () => ({
      createInstance: async () => ({
        exports: {},
        memory: new WebAssembly.Memory({ initial: 1, maximum: 1 }),
        memoryBytes: 65_536,
        disposed: false,
        async dispose() {
          disposeCalls += 1;
          exportsDuringDispose = (() => {
            try {
              return managed.instanceExports(instance);
            } catch (error) {
              return error;
            }
          })();
          if (disposeCalls === 1) throw cleanupError;
        },
      }),
    }));
    try {
      managed = await import('../src/workerd-managed-instance');
      instance = await managed.createInstance({} as WebAssembly.Module);

      await expect(instance.dispose()).rejects.toBe(cleanupError);
      expect(exportsDuringDispose).toBeInstanceOf(Error);
      expect((exportsDuringDispose as Error).message).toMatch(/disposal has started/);
      expect(instance.disposed).toBe(false);
      expect(instance.memoryBytes).toBeGreaterThan(0);
      expect(() => managed.instanceExports(instance)).toThrow(/disposal has started/);
      expect(() => managed.enterInstance(instance)).toThrow(/disposal has started/);

      await expect(instance.dispose()).resolves.toBeUndefined();
      expect(disposeCalls).toBe(2);
      expect(instance.disposed).toBe(true);
      expect(instance.memoryBytes).toBe(0);
      expect(() => managed.instanceExports(instance)).toThrow(
        'This workerd Rolldown instance has been disposed',
      );
      await instance.dispose();
      expect(disposeCalls).toBe(2);
    } finally {
      vi.doUnmock('../src/rolldown-binding.wasip1-deferred.js');
      vi.resetModules();
    }
  });

  test('keeps the raw binding exports off the public handle', async () => {
    const rawExports = { getRuntimeCapabilities: () => ({ target: 'wasi' }) };
    vi.resetModules();
    vi.doMock('../src/rolldown-binding.wasip1-deferred.js', () => ({
      createInstance: async () => ({
        exports: rawExports,
        memory: new WebAssembly.Memory({ initial: 1, maximum: 1 }),
        memoryBytes: 65_536,
        disposed: false,
        async dispose() {},
      }),
    }));
    try {
      const managed = await import('../src/workerd-managed-instance');
      const instance = await managed.createInstance({} as WebAssembly.Module);
      expect('exports' in instance).toBe(false);
      expect(Object.keys(instance).sort()).toEqual([
        'dispose',
        'disposed',
        'memory',
        'memoryBytes',
      ]);
      // Only the internal accessor reaches them, and only until disposal starts.
      expect(managed.instanceExports(instance)).toBe(rawExports);
      await instance.dispose();
      expect(() => managed.instanceExports(instance)).toThrow(
        'This workerd Rolldown instance has been disposed',
      );
    } finally {
      vi.doUnmock('../src/rolldown-binding.wasip1-deferred.js');
      vi.resetModules();
    }
  });

  wasiTest(
    'keeps the measured ~64 MiB initial floor through repeated representative builds',
    { timeout: 60_000 },
    async () => {
      const module = await WebAssembly.compile(await readFile(wasmPath));
      const moduleCount = 256;
      for (let round = 0; round < 3; round += 1) {
        const instance = await createInstance(module);
        expect(instance.memoryBytes).toBeGreaterThanOrEqual(1027 * 64 * 1024);
        expect(instance.memoryBytes).toBeLessThanOrEqual(65 * 1024 * 1024);
        const bundler = new (instanceExports(instance).BindingBundler)();
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

  test('rejects inputs that would require dynamic Wasm compilation', async () => {
    const beforeStats = getWorkerdRuntimeStats();
    const beforeListeners = process.rawListeners('beforeExit').length;

    await expect(
      createInstance(new Uint8Array([0, 97, 115, 109]) as unknown as WebAssembly.Module),
    ).rejects.toThrow(/precompiled WebAssembly\.Module/);

    expect(getWorkerdRuntimeStats()).toEqual(beforeStats);
    expect(process.rawListeners('beforeExit')).toHaveLength(beforeListeners);
  });

  wasiTest('accepts Buffer asset inputs without a Buffer global', async () => {
    const module = await WebAssembly.compile(await readFile(wasmPath));
    vi.stubGlobal('Buffer', undefined);

    let instance: Awaited<ReturnType<typeof createInstance>> | undefined;
    try {
      instance = await createInstance(module);
      const bundler = new (instanceExports(instance).BindingBundler)();
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

// Loaded before setImmediate is stubbed: tsx schedules its own cache write
// with setImmediate(...).unref() when it transforms the module.
const { createInstance, instanceExports } = await import(${JSON.stringify(managedInstancePath.href)})

const realSetImmediate = globalThis.setImmediate
const immediateQueue = []
globalThis.setImmediate = (callback, ...args) => {
  immediateQueue.push(() => callback(...args))
  return immediateQueue.length
}

try {
  const module = await WebAssembly.compile(
    await readFile(${JSON.stringify(fileURLToPath(wasmPath))}),
  )
  const instance = await createInstance(module)
  assert.equal(immediateQueue.length, 0)

  // A raw generate() outside build() is not counted, so dispose() goes ahead
  // while this one native future stays queued.
  const bundler = new (instanceExports(instance).BindingBundler)()
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
