import { AsyncLocalStorage } from 'node:async_hooks';
import { spawnSync } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { Buffer } from 'node:buffer';
import { rolldown } from 'rolldown';
import { viteDynamicImportVarsPlugin, viteReporterPlugin } from 'rolldown/experimental';
import { describe, expect, test } from 'vitest';

// @ts-ignore These focused tests intentionally reach package source outside the test rootDir.
import { bindingifyBuiltInPlugin, BuiltinPlugin } from '../src/builtin-plugin/utils';
// @ts-ignore These focused tests intentionally reach package source outside the test rootDir.
import type { BuildCallbackRunner } from '../src/plugin/bindingify-plugin';
import type {
  BindingViteDynamicImportVarsPluginConfig,
  BindingViteReporterPluginConfig,
  BindingViteResolvePluginConfig,
} from '../src/binding.cjs';
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
      expect(() => asyncContext.configureAsyncContext(undefined as never)).toThrow(
        'Async context provider must define createStorage()',
      );

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

  test('callback-free built-in options need no provider', async () => {
    await withNativeAsyncContext(undefined, async () => {
      const asyncContext = await importBrowserAsyncContext();
      const context = asyncContext.createRequiredAsyncContext<unknown>();
      let runnerCalls = 0;
      const runBuildCallback: BuildCallbackRunner = (callback) => {
        runnerCalls += 1;
        return context.run({}, callback);
      };

      // Option setup may only reach for a provider once it has a callback to
      // run. A config carrying none must stay inert, so making proxy-served
      // callbacks work must not become an unconditional read of every
      // callback key.
      const bindingPlugin = bindingifyBuiltInPlugin(
        viteDynamicImportVarsPlugin({ include: ['**/*.js'] }),
        runBuildCallback,
      );

      expect(
        (bindingPlugin.options as BindingViteDynamicImportVarsPluginConfig).resolver,
      ).toBeUndefined();
      expect(runnerCalls).toBe(0);
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

test('built-in callbacks served from a Proxy get trap enter the build callback runner', () => {
  let runnerCalls = 0;
  const runBuildCallback: BuildCallbackRunner = (callback) => {
    runnerCalls += 1;
    return callback();
  };
  let resolverCalls = 0;
  const resolver = () => {
    resolverCalls += 1;
    return 'from-get-trap';
  };
  // N-API reads built-in options by named property, so a `get` trap that never
  // shows up as a descriptor still hands the native side a callback.
  const config = new Proxy(
    { include: ['**/*.js'] },
    {
      get(target, key, receiver) {
        return key === 'resolver' ? resolver : Reflect.get(target, key, receiver);
      },
    },
  );

  const bindingPlugin = bindingifyBuiltInPlugin(
    viteDynamicImportVarsPlugin(config),
    runBuildCallback,
  );
  const options = bindingPlugin.options as BindingViteDynamicImportVarsPluginConfig;

  expect(options).not.toBe(config);
  expect(options.resolver).not.toBe(resolver);
  expect(options.resolver!('entry.js', 'importer.js')).toBe('from-get-trap');
  expect(runnerCalls).toBe(1);
  expect(resolverCalls).toBe(1);
});

test('a Proxy-served built-in callback does not displace sibling data callbacks', () => {
  const runBuildCallback: BuildCallbackRunner = (callback) => callback();
  const onDebug = () => {};
  const config = new Proxy(
    { onWarn: () => {} },
    {
      get(target, key, receiver) {
        return key === 'onDebug' ? onDebug : Reflect.get(target, key, receiver);
      },
    },
  ) as unknown as BindingViteResolvePluginConfig;

  const bindingPlugin = bindingifyBuiltInPlugin(
    new BuiltinPlugin('builtin:vite-resolve', config),
    runBuildCallback,
  );
  const options = bindingPlugin.options as BindingViteResolvePluginConfig;

  expect(typeof options.onWarn).toBe('function');
  expect(typeof options.onDebug).toBe('function');
});

function reporterConfigTrap(
  served: Record<string, unknown>,
  own: Record<string, unknown> = {},
): BindingViteReporterPluginConfig {
  // Only what `own` carries is an own property; every other field can be
  // answered by the `get` trap alone. A host that computes its reporter config
  // lazily produces exactly this shape.
  return new Proxy(Object.assign({} as BindingViteReporterPluginConfig, own), {
    get(target, key, receiver) {
      if (Reflect.getOwnPropertyDescriptor(target, key)) return Reflect.get(target, key, receiver);
      return typeof key === 'string' && key in served ? served[key] : undefined;
    },
  });
}

test('a stateful built-in callback trap cannot swap in a raw callback after the snapshot', () => {
  let runnerCalls = 0;
  const runBuildCallback: BuildCallbackRunner = (callback) => {
    runnerCalls += 1;
    return callback();
  };

  // N-API reads each field by name after the wrapping pass, so a trap that
  // answers differently on the second read used to hand it the raw callback.
  const buildConfig = (values: readonly unknown[]) => {
    const state = { reads: 0 };
    const config = new Proxy({} as BindingViteReporterPluginConfig, {
      get(target, key, receiver) {
        if (key !== 'logInfo') return Reflect.get(target, key, receiver);
        const value = values[Math.min(state.reads, values.length - 1)];
        state.reads += 1;
        return value;
      },
    });
    return { config, state };
  };

  let rawCalls = 0;
  const rawLogInfo = () => {
    rawCalls += 1;
  };

  const hidden = buildConfig([undefined, rawLogInfo]);
  const hiddenOptions = bindingifyBuiltInPlugin(viteReporterPlugin(hidden.config), runBuildCallback)
    .options as BindingViteReporterPluginConfig;

  expect(hidden.state.reads).toBe(1);
  expect(hiddenOptions).not.toBe(hidden.config);
  expect(hiddenOptions.logInfo).toBeUndefined();
  expect(hiddenOptions.logInfo).toBeUndefined();
  expect(hidden.state.reads).toBe(1);
  expect(rawCalls).toBe(0);
  expect(runnerCalls).toBe(0);

  const revoked = buildConfig([rawLogInfo, undefined]);
  const revokedOptions = bindingifyBuiltInPlugin(
    viteReporterPlugin(revoked.config),
    runBuildCallback,
  ).options as BindingViteReporterPluginConfig;

  const wrapped = revokedOptions.logInfo;
  expect(typeof wrapped).toBe('function');
  expect(wrapped).not.toBe(rawLogInfo);
  expect(revokedOptions.logInfo).toBe(wrapped);
  expect(revoked.state.reads).toBe(1);

  wrapped!('built');
  expect(runnerCalls).toBe(1);
  expect(rawCalls).toBe(1);
});

test('a trap-served built-in config keeps its required fields and runs its callback', async () => {
  let viewRunnerCalls = 0;
  const messages: string[] = [];
  const reentrancy: Promise<unknown>[] = [];
  let bundle: Awaited<ReturnType<typeof rolldown>> | undefined;
  const served = {
    root: import.meta.dirname,
    isTty: false,
    isLib: false,
    assetsDir: 'assets',
    chunkLimit: 500,
    warnLargeChunks: true,
    reportCompressedSize: false,
    logInfo: (message: string) => {
      messages.push(message);
      // Entering the runner is what makes this reentrancy fail, so a rejection
      // here is the proof that `logInfo` ran inside `runBuildCallback`.
      if (bundle) reentrancy.push(bundle.generate({ format: 'esm' }).catch((error) => error));
    },
  };

  // The wrapping pass may not drop the required fields the trap serves, or the
  // binding rejects the config with ``Missing field `root```.
  const view = bindingifyBuiltInPlugin(
    viteReporterPlugin(reporterConfigTrap(served)),
    (callback) => {
      viewRunnerCalls += 1;
      return callback();
    },
  ).options as BindingViteReporterPluginConfig;

  expect(view.root).toBe(served.root);
  expect(view.chunkLimit).toBe(500);
  expect(view.assetsDir).toBe('assets');
  expect(typeof view.logInfo).toBe('function');
  expect(viewRunnerCalls).toBe(0);

  bundle = await rolldown({
    input: 'entry.js',
    plugins: [
      {
        name: 'virtual-entry',
        resolveId: (id) => (id === 'entry.js' ? id : null),
        load: (id) => (id === 'entry.js' ? 'export const value = 1;' : null),
      },
      viteReporterPlugin(reporterConfigTrap(served)),
    ],
  });
  // `logInfo` is the reporter's `writeBundle` reporting hook, so the build has
  // to reach disk for the callback to run at all.
  const outDir = await mkdtemp(path.join(tmpdir(), 'rolldown-trap-served-reporter-'));
  try {
    await bundle.write({ dir: outDir, format: 'esm' });
  } finally {
    await bundle.close();
    await rm(outDir, { force: true, recursive: true });
  }

  expect(messages.length).toBeGreaterThan(0);
  expect(reentrancy.length).toBeGreaterThan(0);
  for (const settled of await Promise.all(reentrancy)) {
    expect(settled).toBeInstanceOf(Error);
    expect((settled as Error).message).toContain(
      "Cannot call bundle.generate() or bundle.write() from one of the same bundle's active JavaScript callbacks",
    );
  }
});

test('an own built-in callback on a trap-served config keeps the trap-served fields', async () => {
  let viewRunnerCalls = 0;
  const messages: string[] = [];
  const reentrancy: Promise<unknown>[] = [];
  let bundle: Awaited<ReturnType<typeof rolldown>> | undefined;
  const served = {
    root: import.meta.dirname,
    isTty: false,
    isLib: false,
    assetsDir: 'assets',
    chunkLimit: 500,
    warnLargeChunks: true,
    reportCompressedSize: false,
  };
  const logInfo = (message: string) => {
    messages.push(message);
    // Entering the runner is what makes this reentrancy fail, so a rejection
    // here is the proof that `logInfo` ran inside `runBuildCallback`.
    if (bundle) reentrancy.push(bundle.generate({ format: 'esm' }).catch((error) => error));
  };

  // The callback is an own data property, so a descriptor answers for it, but
  // every required field still comes from the `get` trap. Rebuilding the
  // options from the original's own descriptors would drop all of them and the
  // binding would reject the config with ``Missing field `root```.
  const view = bindingifyBuiltInPlugin(
    viteReporterPlugin(reporterConfigTrap(served, { logInfo })),
    (callback) => {
      viewRunnerCalls += 1;
      return callback();
    },
  ).options as BindingViteReporterPluginConfig;

  expect(view.root).toBe(served.root);
  expect(view.chunkLimit).toBe(500);
  expect(view.assetsDir).toBe('assets');
  expect(typeof view.logInfo).toBe('function');
  expect(view.logInfo).not.toBe(logInfo);
  expect(viewRunnerCalls).toBe(0);

  bundle = await rolldown({
    input: 'entry.js',
    plugins: [
      {
        name: 'virtual-entry',
        resolveId: (id) => (id === 'entry.js' ? id : null),
        load: (id) => (id === 'entry.js' ? 'export const value = 1;' : null),
      },
      viteReporterPlugin(reporterConfigTrap(served, { logInfo })),
    ],
  });
  // `logInfo` is the reporter's `writeBundle` reporting hook, so the build has
  // to reach disk for the callback to run at all.
  const outDir = await mkdtemp(path.join(tmpdir(), 'rolldown-own-callback-reporter-'));
  try {
    await bundle.write({ dir: outDir, format: 'esm' });
  } finally {
    await bundle.close();
    await rm(outDir, { force: true, recursive: true });
  }

  expect(messages.length).toBeGreaterThan(0);
  expect(reentrancy.length).toBeGreaterThan(0);
  for (const settled of await Promise.all(reentrancy)) {
    expect(settled).toBeInstanceOf(Error);
    expect((settled as Error).message).toContain(
      "Cannot call bundle.generate() or bundle.write() from one of the same bundle's active JavaScript callbacks",
    );
  }
});

test("a built-in callback served by a get trap wins over the target's own descriptor", () => {
  let runnerCalls = 0;
  const runBuildCallback: BuildCallbackRunner = (callback) => {
    runnerCalls += 1;
    return callback();
  };
  let trapReads = 0;
  const ownResolver = () => 'from-own-descriptor';
  const trapResolver = () => 'from-get-trap';
  // N-API reads every field with `napi_get_named_property`, which is a plain
  // `[[Get]]`, so the trap's answer is the one the native side would see. An
  // own data descriptor on the target must not shadow it.
  const config = new Proxy(
    { include: ['**/*.js'], resolver: ownResolver },
    {
      get(target, key, receiver) {
        if (key !== 'resolver') return Reflect.get(target, key, receiver);
        trapReads += 1;
        return trapResolver;
      },
    },
  );

  const options = bindingifyBuiltInPlugin(viteDynamicImportVarsPlugin(config), runBuildCallback)
    .options as BindingViteDynamicImportVarsPluginConfig;

  // One `[[Get]]` per callback key, and a data read is not user code, so it
  // stays outside the boundary.
  expect(trapReads).toBe(1);
  expect(runnerCalls).toBe(0);
  expect(options.resolver).not.toBe(trapResolver);
  expect(options.resolver).not.toBe(ownResolver);
  expect(options.resolver!('entry.js', 'importer.js')).toBe('from-get-trap');
  expect(runnerCalls).toBe(1);
  // Reading the view never reaches the trap again.
  expect(trapReads).toBe(1);
  // What the binding got is what any other JS reader gets.
  expect((Reflect.get(config, 'resolver') as typeof trapResolver)()).toBe('from-get-trap');
});

test('a trap-served built-in callback over an own non-function placeholder is still wrapped', () => {
  let runnerCalls = 0;
  const runBuildCallback: BuildCallbackRunner = (callback) => {
    runnerCalls += 1;
    return callback();
  };
  let rawCalls = 0;
  const rawLogInfo = () => {
    rawCalls += 1;
  };
  // The target owns `logInfo` as `undefined`, so the descriptor walk reports a
  // data property. Answering from it left the config unwrapped while N-API
  // still asked the trap and ran the real callback raw.
  const config = new Proxy({ logInfo: undefined } as BindingViteReporterPluginConfig, {
    get(target, key, receiver) {
      return key === 'logInfo' ? rawLogInfo : Reflect.get(target, key, receiver);
    },
  });

  const options = bindingifyBuiltInPlugin(viteReporterPlugin(config), runBuildCallback)
    .options as BindingViteReporterPluginConfig;

  expect(options).not.toBe(config);
  expect(typeof options.logInfo).toBe('function');
  expect(options.logInfo).not.toBe(rawLogInfo);
  expect(runnerCalls).toBe(0);

  options.logInfo!('built');

  expect(rawCalls).toBe(1);
  expect(runnerCalls).toBe(1);
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

test('browser preflight detects Proxy-served built-in plugin callbacks', () => {
  const logInfo = () => {};
  const config = new Proxy({} as BindingViteReporterPluginConfig, {
    get(target, key, receiver) {
      return key === 'logInfo' ? logInfo : Reflect.get(target, key, receiver);
    },
  });
  const bindingPlugin = bindingifyBuiltInPlugin(viteReporterPlugin(config), (callback) =>
    callback(),
  );

  expect(
    bindingOptionsRequireAsyncContext({ plugins: [bindingPlugin] } as never, {} as never, false),
  ).toBe(true);
});

test('browser preflight answers from the snapshot the binding reads', () => {
  const logInfo = () => {};
  // Every field, callback included, is trap-served: the preflight only looks at
  // own data properties, so the view has to carry the snapshot as one.
  const trapServed = bindingifyBuiltInPlugin(
    viteReporterPlugin(
      reporterConfigTrap({
        root: import.meta.dirname,
        isTty: false,
        isLib: false,
        assetsDir: 'assets',
        chunkLimit: 500,
        warnLargeChunks: true,
        reportCompressedSize: false,
        logInfo,
      }),
    ),
    (callback) => callback(),
  );

  expect(Object.getOwnPropertyDescriptor(trapServed.options as object, 'logInfo')).toMatchObject({
    writable: true,
  });
  expect(
    bindingOptionsRequireAsyncContext({ plugins: [trapServed] } as never, {} as never, false),
  ).toBe(true);

  // A trap that only turns callable on the second read must not leave the
  // preflight answering `false` while the binding reads a callback.
  let reads = 0;
  const stateful = new Proxy({} as BindingViteReporterPluginConfig, {
    get(target, key, receiver) {
      if (key !== 'logInfo') return Reflect.get(target, key, receiver);
      reads += 1;
      return reads === 1 ? undefined : logInfo;
    },
  });
  const statefulPlugin = bindingifyBuiltInPlugin(viteReporterPlugin(stateful), (callback) =>
    callback(),
  );
  const statefulOptions = statefulPlugin.options as BindingViteReporterPluginConfig;

  expect(typeof statefulOptions.logInfo === 'function').toBe(
    bindingOptionsRequireAsyncContext({ plugins: [statefulPlugin] } as never, {} as never, false),
  );
});

test(
  'callback settlement rejects path-local cycles and preserves direct terminal identity',
  { timeout: 10_000 },
  () => {
    // `--import` parses its value as a URL first, so a bare Windows absolute path
    // is read as the `d:` scheme and rejected. Always hand Node a file:// URL.
    const tsxLoaderUrl = pathToFileURL(createRequire(import.meta.url).resolve('tsx')).href;
    const asyncContextUrl = new URL('../src/utils/async-context.ts', import.meta.url).href;
    const child = spawnSync(
      process.execPath,
      [
        '--import',
        tsxLoaderUrl,
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
const terminalRegistry = new WeakMap([[nestedTerminal, 'weak-key']])
const nestedTerminalResult = await settle({ then(resolve) { resolve(nestedTerminal) } })
assert.strictEqual(nestedTerminalResult, nestedTerminal)
// One classification read plus the single adoption performed by the promise
// handed back to the caller. The settled value is boxed in between, so no
// intermediate promise runs the Promise Resolution Procedure on it again.
assert.equal(nestedTerminal.thenReads, 2)
assert.equal(nestedTerminal.marker(), 'terminal')
assert.equal(terminalRegistry.get(nestedTerminalResult), 'weak-key')

let flipReads = 0
let flipDeactivations = 0
let flipSettled = false
const flipTerminal = Object.defineProperty({}, 'then', {
  get() {
    flipReads += 1
    // Terminal on the classification read, then a never-settling thenable.
    // Deactivation must not depend on the value being adopted afterwards.
    return flipReads >= 2 ? () => {} : undefined
  },
})
const flipPromise = trackAsyncCallbackSettlement(
  { then(resolve) { resolve(flipTerminal) } },
  () => { flipDeactivations += 1 },
)
void flipPromise.then(() => { flipSettled = true }, () => { flipSettled = true })
for (let turn = 0; turn < 20; turn += 1) await Promise.resolve()
assert.ok(flipReads >= 2)
assert.equal(flipDeactivations, 1)
assert.equal(flipSettled, false)

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
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'AsyncContext');
  Object.defineProperty(globalThis, 'AsyncContext', {
    configurable: true,
    value,
    writable: true,
  });
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
