import { AsyncLocalStorage } from 'node:async_hooks';
import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';
import path from 'node:path';
import { Buffer } from 'node:buffer';
import { rolldown } from 'rolldown';
import { viteDynamicImportVarsPlugin } from 'rolldown/experimental';
import { describe, expect, test } from 'vitest';

// @ts-ignore These focused tests intentionally reach package source outside the test rootDir.
import { bindingifyBuiltInPlugin } from '../src/builtin-plugin/utils';
// @ts-ignore These focused tests intentionally reach package source outside the test rootDir.
import type { BuildCallbackRunner } from '../src/plugin/bindingify-plugin';
import type { BindingViteDynamicImportVarsPluginConfig } from '../src/binding.cjs';
import type {
  configureAsyncContext,
  createAsyncContext,
  createRequiredAsyncContext,
  getAsyncContextSupport,
} from '../src/utils/async-context';
// @ts-ignore This focused test intentionally reaches package source outside the test rootDir.
import { bindingOptionsRequireAsyncContext } from '../src/utils/create-bundler-option';

type AsyncContextModule = {
  configureAsyncContext: typeof configureAsyncContext;
  createAsyncContext: typeof createAsyncContext;
  createRequiredAsyncContext: typeof createRequiredAsyncContext;
  getAsyncContextSupport: typeof getAsyncContextSupport;
};

let moduleIndex = 0;

describe.sequential('browser async-context contract', () => {
  test('support probing validates storage shape without locking or using it', async () => {
    await withNativeAsyncContext(undefined, async () => {
      const asyncContext = await importBrowserAsyncContext();
      let invalidStorageCreations = 0;
      asyncContext.configureAsyncContext({
        createStorage() {
          invalidStorageCreations += 1;
          return {} as never;
        },
      });

      expect(asyncContext.getAsyncContextSupport()).toEqual({
        source: 'custom',
        supported: false,
      });
      expect(invalidStorageCreations).toBe(1);

      let getStoreCalls = 0;
      let runCalls = 0;
      expect(() =>
        asyncContext.configureAsyncContext({
          createStorage() {
            return {
              getStore() {
                getStoreCalls += 1;
              },
              run(_store, callback) {
                runCalls += 1;
                return callback();
              },
            };
          },
        }),
      ).not.toThrow();
      expect(asyncContext.getAsyncContextSupport()).toEqual({
        source: 'custom',
        supported: true,
      });
      expect(getStoreCalls).toBe(0);
      expect(runCalls).toBe(0);
    });
  });

  test('malformed native AsyncContext.Variable support is reported as unsupported', async () => {
    await withNativeAsyncContext(
      {
        Variable: class {},
      },
      async () => {
        const asyncContext = await importBrowserAsyncContext();
        expect(asyncContext.getAsyncContextSupport()).toEqual({
          source: 'native',
          supported: false,
        });
      },
    );
  });

  test('optional context selection locks the configured provider for required contexts', async () => {
    await withNativeAsyncContext(undefined, async () => {
      const asyncContext = await importBrowserAsyncContext();
      let storageCreations = 0;
      asyncContext.configureAsyncContext({
        createStorage() {
          storageCreations += 1;
          return new AsyncLocalStorage<unknown>();
        },
      });

      const optionalContext = asyncContext.createAsyncContext<string>();
      expect(optionalContext).toBeDefined();
      expect(storageCreations).toBe(1);
      expect(() =>
        asyncContext.configureAsyncContext({
          createStorage: () => new AsyncLocalStorage<unknown>(),
        }),
      ).toThrow(/already in use/);

      const requiredContext = asyncContext.createRequiredAsyncContext<string>();
      await requiredContext.run('selected-provider', async () => {
        await Promise.resolve();
        expect(requiredContext.getStore()).toBe('selected-provider');
      });
      expect(storageCreations).toBe(2);
    });
  });

  test('reentrant global AsyncContext accessor recovers and adopts the nested provider', async () => {
    let asyncContext: AsyncContextModule;
    let nestedContext: ReturnType<typeof createAsyncContext>;
    let getterCalls = 0;
    const providerCreations: string[] = [];
    const accessorError = new Error('nested global AsyncContext accessor failed');
    const ProviderAVariable = createNativeVariableClass(() => providerCreations.push('A'));
    const ProviderBVariable = createNativeVariableClass(() => providerCreations.push('B'));

    await withNativeAsyncContextDescriptor(
      {
        configurable: true,
        get() {
          getterCalls += 1;
          if (getterCalls === 1) {
            asyncContext.createAsyncContext();
            throw new Error('Expected nested global AsyncContext accessor to fail');
          }
          if (getterCalls === 2) throw accessorError;
          Object.defineProperty(globalThis, 'AsyncContext', {
            configurable: true,
            value: { Variable: ProviderBVariable },
            writable: true,
          });
          nestedContext = asyncContext.createAsyncContext();
          return { Variable: ProviderAVariable };
        },
      },
      async () => {
        asyncContext = await importBrowserAsyncContext();

        let acquisitionError: unknown;
        try {
          asyncContext.createAsyncContext();
        } catch (error) {
          acquisitionError = error;
        }
        expect(acquisitionError).toBe(accessorError);
        expect(providerCreations).toEqual([]);
        expect(() => asyncContext.configureAsyncContext(undefined as never)).toThrow(
          'Async context provider must define createStorage()',
        );

        const outerContext = asyncContext.createAsyncContext();
        const laterContext = asyncContext.createAsyncContext();

        expect(outerContext).toBeDefined();
        expect(nestedContext).toBeDefined();
        expect(laterContext).toBeDefined();
        expect(getterCalls).toBe(3);
        expect(providerCreations).toEqual(['B', 'B', 'B']);
        expect(asyncContext.getAsyncContextSupport()).toEqual({
          source: 'native',
          supported: true,
        });
        expect(providerCreations).toEqual(['B', 'B', 'B', 'B']);
      },
    );
  });

  test('reentrant AsyncContext.Variable accessor recovers and adopts the nested provider', async () => {
    let asyncContext: AsyncContextModule;
    let nestedContext: ReturnType<typeof createAsyncContext>;
    let getterCalls = 0;
    const providerCreations: string[] = [];
    const accessorError = new Error('nested AsyncContext.Variable accessor failed');
    const ProviderAVariable = createNativeVariableClass(() => providerCreations.push('A'));
    const ProviderBVariable = createNativeVariableClass(() => providerCreations.push('B'));
    const nativeAsyncContext = Object.defineProperty({}, 'Variable', {
      configurable: true,
      get() {
        getterCalls += 1;
        if (getterCalls === 1) {
          asyncContext.createAsyncContext();
          throw new Error('Expected nested AsyncContext.Variable accessor to fail');
        }
        if (getterCalls === 2) throw accessorError;
        Object.defineProperty(nativeAsyncContext, 'Variable', {
          configurable: true,
          value: ProviderBVariable,
          writable: true,
        });
        nestedContext = asyncContext.createAsyncContext();
        return ProviderAVariable;
      },
    });

    await withNativeAsyncContext(nativeAsyncContext, async () => {
      asyncContext = await importBrowserAsyncContext();

      let acquisitionError: unknown;
      try {
        asyncContext.createAsyncContext();
      } catch (error) {
        acquisitionError = error;
      }
      expect(acquisitionError).toBe(accessorError);
      expect(providerCreations).toEqual([]);
      expect(() => asyncContext.configureAsyncContext(undefined as never)).toThrow(
        'Async context provider must define createStorage()',
      );

      const outerContext = asyncContext.createAsyncContext();
      const laterContext = asyncContext.createAsyncContext();

      expect(outerContext).toBeDefined();
      expect(nestedContext).toBeDefined();
      expect(laterContext).toBeDefined();
      expect(getterCalls).toBe(3);
      expect(providerCreations).toEqual(['B', 'B', 'B']);
      expect(asyncContext.getAsyncContextSupport()).toEqual({
        source: 'native',
        supported: true,
      });
      expect(providerCreations).toEqual(['B', 'B', 'B', 'B']);
    });
  });

  test('reentrant native acquisition keeps the first provider candidate', async () => {
    let asyncContext: AsyncContextModule;
    let nestedContext: ReturnType<typeof createAsyncContext>;
    let providerACreations = 0;
    let providerBCreations = 0;

    class ProviderBVariable {
      readonly storage = new AsyncLocalStorage<unknown>();

      constructor() {
        providerBCreations += 1;
      }

      get(): unknown {
        return this.storage.getStore();
      }

      run<R>(store: unknown, callback: () => R): R {
        return this.storage.run(store, callback);
      }
    }

    class ProviderAVariable {
      readonly storage = new AsyncLocalStorage<unknown>();

      constructor() {
        providerACreations += 1;
        if (providerACreations === 1) {
          Object.defineProperty(globalThis, 'AsyncContext', {
            configurable: true,
            value: { Variable: ProviderBVariable },
            writable: true,
          });
          nestedContext = asyncContext.createAsyncContext();
        }
      }

      get(): unknown {
        return this.storage.getStore();
      }

      run<R>(store: unknown, callback: () => R): R {
        return this.storage.run(store, callback);
      }
    }

    await withNativeAsyncContext({ Variable: ProviderAVariable }, async () => {
      asyncContext = await importBrowserAsyncContext();

      const outerContext = asyncContext.createAsyncContext();
      const laterContext = asyncContext.createAsyncContext();

      expect(outerContext).toBeDefined();
      expect(nestedContext).toBeDefined();
      expect(laterContext).toBeDefined();
      expect(providerACreations).toBe(3);
      expect(providerBCreations).toBe(0);
      expect(asyncContext.getAsyncContextSupport()).toEqual({
        source: 'native',
        supported: true,
      });
      expect(providerACreations).toBe(4);
      expect(providerBCreations).toBe(0);
    });
  });

  test('reentrant acquisition failure cannot reopen a successful provider selection', async () => {
    await withNativeAsyncContext(undefined, async () => {
      const asyncContext = await importBrowserAsyncContext();
      let storageCreations = 0;
      let replacementDuringSelection = false;
      const replacementProvider = {
        createStorage: () => new AsyncLocalStorage<unknown>(),
      };
      asyncContext.configureAsyncContext({
        createStorage() {
          storageCreations += 1;
          if (storageCreations === 1) {
            expect(() => asyncContext.createAsyncContext()).toThrow('nested acquisition failed');
            try {
              asyncContext.configureAsyncContext(replacementProvider);
              replacementDuringSelection = true;
            } catch {}
            return new AsyncLocalStorage<unknown>();
          }
          if (storageCreations === 2) throw new Error('nested acquisition failed');
          return new AsyncLocalStorage<unknown>();
        },
      });

      expect(asyncContext.createAsyncContext()).toBeDefined();
      expect(replacementDuringSelection).toBe(false);
      expect(() => asyncContext.configureAsyncContext(replacementProvider)).toThrow(
        /already in use/,
      );
      expect(asyncContext.getAsyncContextSupport()).toEqual({
        source: 'custom',
        supported: true,
      });
    });
  });

  test('provider validation cannot reenter configuration and snapshots createStorage', async () => {
    await withNativeAsyncContext(undefined, async () => {
      const asyncContext = await importBrowserAsyncContext();
      let getterCalls = 0;
      let selectedStorageCreations = 0;
      let replacementStorageCreations = 0;
      const replacementProvider = {
        createStorage() {
          replacementStorageCreations += 1;
          return new AsyncLocalStorage<unknown>();
        },
      };
      const provider = Object.defineProperty({}, 'createStorage', {
        get() {
          getterCalls += 1;
          expect(() => asyncContext.configureAsyncContext(replacementProvider)).toThrow(
            /already in use/,
          );
          return () => {
            selectedStorageCreations += 1;
            return new AsyncLocalStorage<unknown>();
          };
        },
      });

      asyncContext.configureAsyncContext(provider as never);
      expect(asyncContext.createAsyncContext()).toBeDefined();
      expect(asyncContext.getAsyncContextSupport()).toEqual({
        source: 'custom',
        supported: true,
      });
      expect(getterCalls).toBe(1);
      expect(selectedStorageCreations).toBe(2);
      expect(replacementStorageCreations).toBe(0);
    });
  });

  test('provider validation cannot finish after reentrant native selection', async () => {
    await withNativeAsyncContext(
      {
        Variable: class {
          get(): undefined {
            return undefined;
          }

          run<T>(_store: unknown, callback: () => T): T {
            return callback();
          }
        },
      },
      async () => {
        const asyncContext = await importBrowserAsyncContext();
        const provider = Object.defineProperty({}, 'createStorage', {
          get() {
            expect(asyncContext.createAsyncContext()).toBeDefined();
            return () => new AsyncLocalStorage<unknown>();
          },
        });

        expect(() => asyncContext.configureAsyncContext(provider as never)).toThrow(
          /already in use/,
        );
        expect(asyncContext.getAsyncContextSupport()).toEqual({
          source: 'native',
          supported: true,
        });
      },
    );
  });

  test('unavailable optional selection remains configurable', async () => {
    await withNativeAsyncContext(undefined, async () => {
      const asyncContext = await importBrowserAsyncContext();
      expect(asyncContext.createAsyncContext()).toBeUndefined();

      asyncContext.configureAsyncContext({
        createStorage: () => new AsyncLocalStorage<unknown>(),
      });

      expect(asyncContext.getAsyncContextSupport()).toEqual({
        source: 'custom',
        supported: true,
      });
      const context = asyncContext.createAsyncContext<string>();
      expect(context).toBeDefined();
      await context!.run('configured', async () => {
        await Promise.resolve();
        expect(context!.getStore()).toBe('configured');
      });
    });
  });

  test('failed required selection can recover after configuring a provider', async () => {
    await withNativeAsyncContext(undefined, async () => {
      const asyncContext = await importBrowserAsyncContext();
      const context = asyncContext.createRequiredAsyncContext<string>();

      expect(() => context.run('unavailable', () => {})).toThrowError(
        expect.objectContaining({
          code: 'ERR_ROLLDOWN_ASYNC_CONTEXT_UNAVAILABLE',
          name: 'AsyncContextUnavailableError',
        }),
      );

      asyncContext.configureAsyncContext({
        createStorage: () => new AsyncLocalStorage<unknown>(),
      });
      await context.run('configured', async () => {
        await Promise.resolve();
        expect(context.getStore()).toBe('configured');
      });
      expect(asyncContext.getAsyncContextSupport()).toEqual({
        source: 'custom',
        supported: true,
      });
    });
  });

  test('built-in callback wrappers fail before user code without a provider', async () => {
    await withNativeAsyncContext(undefined, async () => {
      const asyncContext = await importBrowserAsyncContext();
      const context = asyncContext.createRequiredAsyncContext<unknown>();
      const runBuildCallback: BuildCallbackRunner = (callback) => context.run({}, callback);
      let callbackCalls = 0;
      const bindingPlugin = bindingifyBuiltInPlugin(
        viteDynamicImportVarsPlugin({
          resolver() {
            callbackCalls += 1;
            return undefined;
          },
        }),
        runBuildCallback,
      );
      const resolver = (bindingPlugin.options as BindingViteDynamicImportVarsPluginConfig)
        .resolver!;

      expect(() => resolver('entry.js', 'importer.js')).toThrowError(
        expect.objectContaining({
          code: 'ERR_ROLLDOWN_ASYNC_CONTEXT_UNAVAILABLE',
        }),
      );
      expect(callbackCalls).toBe(0);
    });
  });

  test('built-in callback accessors fail before the getter without a provider', async () => {
    await withNativeAsyncContext(undefined, async () => {
      const asyncContext = await importBrowserAsyncContext();
      const context = asyncContext.createRequiredAsyncContext<unknown>();
      const runBuildCallback: BuildCallbackRunner = (callback) => context.run({}, callback);
      let getterCalls = 0;
      const config = {};
      Object.defineProperty(config, 'resolver', {
        configurable: true,
        enumerable: true,
        get() {
          getterCalls += 1;
          return undefined;
        },
      });

      expect(() =>
        bindingifyBuiltInPlugin(viteDynamicImportVarsPlugin(config), runBuildCallback),
      ).toThrowError(
        expect.objectContaining({
          code: 'ERR_ROLLDOWN_ASYNC_CONTEXT_UNAVAILABLE',
        }),
      );
      expect(getterCalls).toBe(0);
    });
  });

  test.each([
    [
      'same-identity cycle',
      () => {
        let config: object;
        config = new Proxy(
          {},
          {
            getPrototypeOf() {
              return config;
            },
          },
        );
        return config;
      },
      /Prototype cycle detected while inspecting callback options/,
    ],
    [
      'fresh-proxy chain',
      () => {
        const createConfig = (): object =>
          new Proxy(
            {},
            {
              getPrototypeOf() {
                return createConfig();
              },
            },
          );
        return createConfig();
      },
      /Prototype chain exceeded 256 objects while inspecting callback options/,
    ],
  ])('built-in callback option access rejects a %s', (_, createConfig, expected) => {
    const runBuildCallback: BuildCallbackRunner = (callback) => callback();
    expect(() =>
      bindingifyBuiltInPlugin(viteDynamicImportVarsPlugin(createConfig()), runBuildCallback),
    ).toThrow(expected);
  });
});

test.each([
  ['without a build callback runner', undefined],
  ['with a build callback runner', ((callback) => callback()) satisfies BuildCallbackRunner],
])('built-in callbacks preserve their options receiver %s', (_, runBuildCallback) => {
  const marker = 'options receiver';
  const options = {
    marker,
    resolver(this: { marker: string }) {
      return this.marker;
    },
  };
  const bindingPlugin = bindingifyBuiltInPlugin(
    viteDynamicImportVarsPlugin(options),
    runBuildCallback,
  );
  const resolver = (bindingPlugin.options as BindingViteDynamicImportVarsPluginConfig).resolver!;

  expect(resolver('entry.js', 'importer.js')).toBe(marker);
});

test('browser preflight detects direct data-property plugin callbacks', () => {
  const plugin = {
    name: 'direct-data-callback',
    buildStart: () => {},
  };
  expect(Object.getOwnPropertyDescriptor(plugin, 'buildStart')).toMatchObject({
    value: plugin.buildStart,
  });

  expect(
    bindingOptionsRequireAsyncContext(
      {
        plugins: [{ name: plugin.name, buildStart: plugin.buildStart }],
      } as never,
      {} as never,
      false,
    ),
  ).toBe(true);
});

test(
  'callback settlement rejects path-local cycles and preserves direct terminal identity',
  { timeout: 10_000 },
  () => {
    const tsxLoader = createRequire(import.meta.url).resolve('tsx');
    const asyncContextUrl = new URL('../src/utils/async-context.ts', import.meta.url).href;
    const child = spawnSync(
      process.execPath,
      [
        '--import',
        tsxLoader,
        '--input-type=module',
        '--eval',
        `
import assert from 'node:assert/strict'
import { trackAsyncCallbackSettlement } from ${JSON.stringify(asyncContextUrl)}

const settle = (value) => trackAsyncCallbackSettlement(value, () => {})
const cyclePattern = /Thenable cycle detected while settling a callback result/

const self = {}
self.then = (resolve) => resolve(self)
await assert.rejects(settle(self), cyclePattern)

const first = {}
const second = {}
first.then = (resolve) => resolve(second)
second.then = (resolve) => resolve(first)
await assert.rejects(settle(first), cyclePattern)

let publicPromise
const publicPromiseCycle = {
  then(resolve) {
    resolve(publicPromise)
  },
}
publicPromise = settle(publicPromiseCycle)
await assert.rejects(publicPromise, cyclePattern)

let alternatingReads = 0
const alternatingFirst = {}
const alternatingSecond = {
  then(resolve) {
    resolve(alternatingFirst)
  },
}
Object.defineProperty(alternatingFirst, 'then', {
  get() {
    alternatingReads += 1
    return alternatingReads % 2 === 1
      ? (resolve) => resolve(alternatingSecond)
      : (resolve) => resolve(alternatingFirst)
  },
})
await assert.rejects(settle(alternatingFirst), cyclePattern)
assert.equal(alternatingReads, 1)

class Terminal {
  #marker = 'terminal'
  thenReads = 0

  get then() {
    this.thenReads += 1
    return undefined
  }

  marker() {
    return this.#marker
  }
}

const directTerminal = new Terminal()
assert.strictEqual(settle(directTerminal), directTerminal)
assert.equal(directTerminal.thenReads, 1)
assert.equal(directTerminal.marker(), 'terminal')

const nestedTerminal = new Terminal()
assert.strictEqual(
  await settle({ then(resolve) { resolve(nestedTerminal) } }),
  nestedTerminal,
)
assert.ok(nestedTerminal.thenReads > 0)
assert.equal(nestedTerminal.marker(), 'terminal')

const nestedSelf = {}
let nestedSelfReads = 0
Object.defineProperty(nestedSelf, 'then', {
  get() {
    nestedSelfReads += 1
    return (resolve) => resolve(nestedSelf)
  },
})
await assert.rejects(settle({ then(resolve) { resolve(nestedSelf) } }), cyclePattern)
assert.equal(nestedSelfReads, 1)

let callableGetterReads = 0
let callableThenCalls = 0
const nestedCallable = Object.defineProperty({}, 'then', {
  get() {
    callableGetterReads += 1
    return (resolve) => {
      callableThenCalls += 1
      resolve('accessor-settled')
    }
  },
})
assert.equal(
  await settle({ then(resolve) { resolve(nestedCallable) } }),
  'accessor-settled',
)
assert.equal(callableGetterReads, 1)
assert.equal(callableThenCalls, 1)

const getterError = new Error('nested then getter failed')
const nestedThrowing = Object.defineProperty({}, 'then', {
  get() {
    throw getterError
  },
})
await assert.rejects(
  settle({ then(resolve) { resolve(nestedThrowing) } }),
  (error) => error === getterError,
)

const mutableSelf = {
  then(resolve) {
    delete mutableSelf.then
    resolve(mutableSelf)
  },
}
assert.strictEqual(await settle(mutableSelf), mutableSelf)

let mutableAccessorReads = 0
class MutableAccessorSelf {
  #marker = 'mutable-accessor'

  constructor() {
    Object.defineProperty(this, 'then', {
      configurable: true,
      get: () => {
        mutableAccessorReads += 1
        Reflect.deleteProperty(this, 'then')
        return (resolve) => resolve(this)
      },
    })
  }

  marker() {
    return this.#marker
  }
}
const mutableAccessorSelf = new MutableAccessorSelf()
assert.strictEqual(await settle(mutableAccessorSelf), mutableAccessorSelf)
assert.equal(mutableAccessorReads, 1)
assert.equal(mutableAccessorSelf.marker(), 'mutable-accessor')

const nestedEvents = []
const nestedMutable = {
  then(resolve) {
    nestedEvents.push('nested:then')
    resolve('original')
  },
}
const outerMutable = {
  then(resolve) {
    resolve(nestedMutable)
    queueMicrotask(() => {
      nestedEvents.push('outer:microtask')
      nestedMutable.then = (resolve) => resolve('mutated')
    })
  },
}
assert.equal(await settle(outerMutable), 'original')
assert.deepEqual(nestedEvents, ['nested:then', 'outer:microtask'])

console.log('async callback settlement completed')
`,
      ],
      {
        encoding: 'utf8',
        timeout: 5_000,
      },
    );

    expect(child.error).toBeUndefined();
    expect(child.signal).toBeNull();
    expect(child.status, child.stderr || child.stdout).toBe(0);
    expect(child.stdout).toContain('async callback settlement completed');
  },
);

async function importBrowserAsyncContext(): Promise<AsyncContextModule> {
  const bundle = await rolldown({
    input: path.resolve(import.meta.dirname, '../src/utils/async-context.ts'),
    platform: 'node',
    transform: {
      define: {
        'import.meta.browserBuild': 'true',
      },
    },
  });
  try {
    const output = await bundle.generate({
      codeSplitting: false,
      format: 'esm',
    });
    const chunk = output.output.find((item) => item.type === 'chunk');
    if (!chunk) throw new Error('Expected async-context browser test chunk');
    const encoded = Buffer.from(chunk.code).toString('base64');
    return (await import(
      `data:text/javascript;base64,${encoded}#async-context-${moduleIndex++}`
    )) as AsyncContextModule;
  } finally {
    await bundle.close();
  }
}

async function withNativeAsyncContext(
  value: unknown,
  callback: () => Promise<void>,
): Promise<void> {
  await withNativeAsyncContextDescriptor(
    {
      configurable: true,
      value,
      writable: true,
    },
    callback,
  );
}

async function withNativeAsyncContextDescriptor(
  nativeDescriptor: PropertyDescriptor,
  callback: () => Promise<void>,
): Promise<void> {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'AsyncContext');
  Object.defineProperty(globalThis, 'AsyncContext', nativeDescriptor);
  try {
    await callback();
  } finally {
    if (descriptor) {
      Object.defineProperty(globalThis, 'AsyncContext', descriptor);
    } else {
      Reflect.deleteProperty(globalThis, 'AsyncContext');
    }
  }
}

function createNativeVariableClass(onCreate: () => void) {
  return class {
    readonly storage = new AsyncLocalStorage<unknown>();

    constructor() {
      onCreate();
    }

    get(): unknown {
      return this.storage.getStore();
    }

    run<R>(store: unknown, callback: () => R): R {
      return this.storage.run(store, callback);
    }
  };
}
