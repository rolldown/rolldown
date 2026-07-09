import assert from 'node:assert/strict';
import { AsyncLocalStorage } from 'node:async_hooks';
import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const repoRoot = path.resolve(import.meta.dirname, '../..');
const distDir = path.resolve(repoRoot, process.argv[2] ?? 'packages/browser/dist');
const entries = await readdir(distDir);
const browserLoader = 'rolldown-binding.wasip1-browser.js';
const browserLoaderCode = await readFile(path.join(distDir, browserLoader), 'utf8');
if (
  !browserLoaderCode.includes('registerCurrentThreadTaskHost') ||
  !browserLoaderCode.includes('registerTimerHost') ||
  !browserLoaderCode.includes('__setTimeoutHost') ||
  !browserLoaderCode.includes('__clearTimeoutHost')
) {
  throw new Error('Browser WASI loader does not register its CurrentThread task and timer hosts');
}
if (browserLoaderCode.includes('rolldown-binding.wasip1.cjs')) {
  throw new Error('Browser timer host unexpectedly imports the Node WASI loader');
}

for (const entry of ['index.browser.mjs', 'experimental-index.browser.mjs']) {
  const code = await readFile(path.join(distDir, entry), 'utf8');
  if (!code.includes(`"./${browserLoader}"`)) {
    throw new Error(`${entry} does not import the browser WASI loader`);
  }
}

for (const entry of entries.filter((entry) => /\.(?:js|mjs)$/.test(entry))) {
  const code = await readFile(path.join(distDir, entry), 'utf8');
  if (code.includes('node:async_hooks')) {
    throw new Error(`${entry} must not import node:async_hooks in the browser artifact`);
  }
}

const originalFetch = globalThis.fetch;
globalThis.fetch = async (input, init) => {
  const url = input instanceof Request ? input.url : String(input);
  if (url.startsWith('file:')) {
    return new Response(await readFile(fileURLToPath(url)));
  }
  return originalFetch(input, init);
};
try {
  const browserApi = await import(
    `${pathToFileURL(path.join(distDir, 'index.browser.mjs')).href}?runtime-contract`
  );
  const experimentalApi = await import(
    `${pathToFileURL(path.join(distDir, 'experimental-index.browser.mjs')).href}?runtime-contract`
  );
  assert.deepEqual(experimentalApi.getRuntimeSupport(), {
    dev: false,
    watch: false,
    dynamicImportVarsResolver: true,
    importGlobResolver: true,
    parallelPlugins: false,
    pluginErrorMetadata: true,
    symlinks: false,
    threadlessWasi: true,
    workerd: true,
  });
  const originalProcess = globalThis.process;
  globalThis.process = undefined;
  try {
    experimentalApi.memfs.volume.fromJSON({
      '/entry.js': 'export default 1',
    });
    const noPluginBundle = await browserApi.rolldown({ cwd: '/', input: '/entry.js' });
    await noPluginBundle.generate();
    await noPluginBundle.close();
    assert.deepEqual(experimentalApi.getAsyncContextSupport(), {
      source: 'unavailable',
      supported: false,
    });

    let unavailableHookCalls = 0;
    const unavailableHook = {
      async buildStart() {
        unavailableHookCalls += 1;
      },
    };
    assert.equal(
      typeof Object.getOwnPropertyDescriptor(unavailableHook, 'buildStart')?.value,
      'function',
      'Async-context preflight must exercise a direct callback data property',
    );
    const unavailableBundle = await createVirtualBundle(browserApi, unavailableHook);
    const unavailableError = await unavailableBundle.generate().catch((error) => error);
    assert.equal(unavailableError?.name, 'AsyncContextUnavailableError');
    assert.equal(unavailableError?.code, 'ERR_ROLLDOWN_ASYNC_CONTEXT_UNAVAILABLE');
    assert.match(
      unavailableError?.message ?? '',
      /browser require async-context propagation|configureAsyncContext/,
    );
    assert.equal(unavailableHookCalls, 0, 'Unavailable async context must fail before the hook');
    await unavailableBundle.close();
    assert.deepEqual(experimentalApi.getAsyncContextSupport(), {
      source: 'unavailable',
      supported: false,
    });

    experimentalApi.configureAsyncContext({
      createStorage: () => new AsyncLocalStorage(),
    });
    assert.deepEqual(experimentalApi.getAsyncContextSupport(), {
      source: 'custom',
      supported: true,
    });

    const metadataCause = Object.assign(new RangeError('browser nested cause'), {
      nestedMarker: 31,
    });
    const originalMetadataError = Object.assign(new TypeError('browser plugin metadata failure'), {
      cause: metadataCause,
      code: 'BROWSER_USER_CODE',
      customMarker: 'browser-retained',
    });
    const metadataBundle = await createVirtualBundle(browserApi, {
      transform(_code, id) {
        if (id === 'virtual:entry') throw originalMetadataError;
      },
    });
    try {
      const failure = await metadataBundle.generate().catch((error) => error);
      const [pluginError] = failure?.errors ?? [];
      assert.equal(pluginError, originalMetadataError);
      assert.equal(pluginError.code, 'PLUGIN_ERROR');
      assert.equal(pluginError.pluginCode, 'BROWSER_USER_CODE');
      assert.equal(pluginError.plugin, 'browser-async-context-contract');
      assert.equal(pluginError.hook, 'transform');
      assert.equal(pluginError.id, 'virtual:entry');
      assert.equal(pluginError.customMarker, 'browser-retained');
      assert.match(pluginError.stack, /browser plugin metadata failure/);
      assert.equal(pluginError.cause, metadataCause);
      assert.equal(pluginError.cause.nestedMarker, 31);
    } finally {
      await metadataBundle.close();
    }

    for (const operation of ['generate', 'write', 'close']) {
      let bundle;
      let reentrantError;
      let attempted = false;
      bundle = await createVirtualBundle(browserApi, {
        async buildStart() {
          if (attempted) return;
          attempted = true;
          await Promise.resolve();
          try {
            if (operation === 'close') {
              await bundle.close();
            } else {
              await bundle[operation]();
            }
          } catch (error) {
            reentrantError = error;
          }
        },
      });
      await bundle.generate();
      assert.match(reentrantError?.message ?? '', /active JavaScript callbacks/);
      await bundle.close();
    }

    let outputCallbackBundle;
    let outputCallbackError;
    outputCallbackBundle = await browserApi.rolldown({
      cwd: '/',
      input: '/entry.js',
    });
    await outputCallbackBundle.generate({
      async banner() {
        await Promise.resolve();
        try {
          await outputCallbackBundle.generate();
        } catch (error) {
          outputCallbackError = error;
        }
        return '';
      },
    });
    assert.match(outputCallbackError?.message ?? '', /active JavaScript callbacks/);
    await outputCallbackBundle.close();

    assert.throws(
      () =>
        experimentalApi.configureAsyncContext({
          createStorage: () => new AsyncLocalStorage(),
        }),
      /already in use/,
    );

    let buildStarts = 0;
    let markHookStarted;
    const hookStarted = new Promise((resolve) => {
      markHookStarted = resolve;
    });
    let releaseHook;
    const hookRelease = new Promise((resolve) => {
      releaseHook = resolve;
    });
    const concurrentBundle = await createVirtualBundle(browserApi, {
      async buildStart() {
        buildStarts += 1;
        if (buildStarts === 1) {
          markHookStarted();
          await hookRelease;
        }
      },
    });
    const firstBuild = concurrentBundle.generate();
    await hookStarted;
    const externalBuild = concurrentBundle.generate();
    releaseHook();
    await Promise.all([firstBuild, externalBuild]);
    assert.equal(buildStarts, 2, 'External concurrent browser builds must remain supported');
    await concurrentBundle.close();
  } finally {
    globalThis.process = originalProcess;
  }
} finally {
  globalThis.fetch = originalFetch;
}

console.log(`OK: browser entries register CurrentThread hosts through ${browserLoader}`);

function createVirtualBundle(browserApi, hook) {
  return browserApi.rolldown({
    cwd: '/',
    input: 'virtual:entry',
    plugins: [
      {
        name: 'browser-async-context-contract',
        ...hook,
        resolveId(id) {
          if (id === 'virtual:entry') return id;
        },
        load(id) {
          if (id === 'virtual:entry') return 'export default 1';
        },
      },
    ],
  });
}
