import assert from 'node:assert/strict';
import { AsyncLocalStorage } from 'node:async_hooks';
import { readdir, readFile } from 'node:fs/promises';
import { parse } from 'acorn';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const repoRoot = path.resolve(import.meta.dirname, '../..');
const distDir = path.resolve(repoRoot, process.argv[2] ?? 'packages/browser/dist');
const entries = await readdir(distDir);
const browserLoader = 'rolldown-binding.wasip1-browser.js';
const browserLoaderCode = await readFile(path.join(distDir, browserLoader), 'utf8');
// @napi-rs/cli >= 3.10.0 installs both CurrentThread hosts from the shared
// `@napi-rs/async-runtime` protocol package instead of emitting a
// rolldown-authored bootstrap, so the loader's own `__setTimeoutHost` /
// `__clearTimeoutHost` locals are gone. The browser build bundles that
// installer into the loader, so the contract is asserted against the bundled
// upstream code: string literals survive bundling verbatim, identifiers may be
// renamed, hence the renaming-tolerant patterns below.
// See internal-docs/async-runtime/implementation.md.
const hostContractFailures = [];
const requireLoaderMarker = (label, pattern) => {
  if (!pattern.test(browserLoaderCode)) {
    hostContractFailures.push(label);
  }
};
// A quoted name — the form the installer reads its binding exports in — rather
// than a bare substring, which any mention of the host anywhere would satisfy.
const quotedName = (name) => new RegExp('([\'"`])' + name + '\\1');

// The upstream installer is really in the bundle, not merely something that
// happens to name the hosts.
requireLoaderMarker(
  'the @napi-rs/async-runtime installer (its realm-global registration registry key)',
  /@napi-rs\/async-runtime\/current-thread-hosts\/v4/,
);
requireLoaderMarker(
  'the @napi-rs/async-runtime binding-mismatch guard',
  /ERR_NAPI_ASYNC_RUNTIME_BINDING_MISMATCH/,
);

// Both hosts are wired: the installer reads every binding export it needs by
// name, so each quoted name proves that half of the contract survived.
for (const [host, exportNames] of [
  ['CurrentThread task host', ['registerCurrentThreadTaskHost', 'unregisterCurrentThreadTaskHost']],
  ['timer host', ['registerTimerHost', 'unregisterTimerHost']],
  [
    'host registration handshake',
    [
      'getCurrentThreadTaskHostContractVersion',
      'isCurrentThreadHostRegistrationActive',
      'reserveCurrentThreadHostRegistration',
    ],
  ],
]) {
  for (const exportName of exportNames) {
    requireLoaderMarker(`${host} binding export ${exportName}`, quotedName(exportName));
  }
}

// The loader bootstrap runs the installer against the binding exports and
// leaves the timer host enabled — a second argument would be the
// `{ installTimerHost: false }` opt-out.
requireLoaderMarker(
  'the installer call against the binding exports with the timer host enabled',
  /__currentThreadHostsDisposer\w*\s*=\s*[A-Za-z_$][\w$]*\(\s*__napiModule\w*\.exports\s*,?\s*\)/,
);
// The disposer must be captured AND evicted on teardown: a declared but never
// called helper would leave both hosts registered against a destroyed context.
requireLoaderMarker(
  'the CurrentThread host disposal helper',
  /function __disposeCurrentThreadHosts\w*\(\)/,
);
if ((browserLoaderCode.match(/__disposeCurrentThreadHosts\w*\(\)/g) ?? []).length < 2) {
  hostContractFailures.push('a call to the CurrentThread host disposal helper (declaration only)');
}

if (hostContractFailures.length > 0) {
  throw new Error(
    `Browser WASI loader does not install its CurrentThread task and timer hosts through ` +
      `@napi-rs/async-runtime — missing: ${hostContractFailures.join('; ')}`,
  );
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

// The workerd entries may probe for `node:async_hooks` at runtime: workerd
// serves it behind the `nodejs_als`/`nodejs_compat` compatibility flag, and the
// probe is a guarded dynamic import whose rejection is swallowed elsewhere. A
// STATIC dependency on it is still forbidden everywhere, including there.
const runtimeAsyncHooksProbeEntries = new Set(['workerd.mjs', 'workerd.browser.mjs']);

function findAsyncHooksReferences(code) {
  const program = parse(code, { ecmaVersion: 'latest', sourceType: 'module' });
  const references = [];
  const pending = [program];
  while (pending.length > 0) {
    const node = pending.pop();
    if (!node || typeof node !== 'object') continue;
    const isStaticSource =
      (node.type === 'ImportDeclaration' ||
        node.type === 'ExportNamedDeclaration' ||
        node.type === 'ExportAllDeclaration') &&
      node.source?.value === 'node:async_hooks';
    const isDynamicSource =
      node.type === 'ImportExpression' && node.source?.value === 'node:async_hooks';
    const isRequire =
      node.type === 'CallExpression' &&
      node.callee?.type === 'Identifier' &&
      /^(?:__)?require\d*$/.test(node.callee.name) &&
      node.arguments?.[0]?.value === 'node:async_hooks';
    if (isStaticSource || isRequire) references.push('static');
    if (isDynamicSource) references.push('dynamic');
    for (const value of Object.values(node)) {
      if (Array.isArray(value)) {
        pending.push(...value);
      } else if (value && typeof value === 'object') {
        pending.push(value);
      }
    }
  }
  return references;
}

assert.deepEqual(
  findAsyncHooksReferences(
    "import 'node:async_hooks'; import('node:async_hooks'); __require('node:async_hooks');",
  ).sort(),
  ['dynamic', 'static', 'static'],
  'async-hooks scan must tell static dependencies from runtime probes',
);

for (const entry of entries.filter((entry) => /\.(?:js|mjs)$/.test(entry))) {
  const code = await readFile(path.join(distDir, entry), 'utf8');
  const references = findAsyncHooksReferences(code);
  if (references.includes('static')) {
    throw new Error(`${entry} must not statically import node:async_hooks in the browser artifact`);
  }
  if (references.length > 0 && !runtimeAsyncHooksProbeEntries.has(entry)) {
    throw new Error(`${entry} must not reference node:async_hooks in the browser artifact`);
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

    // An output-option callback is not wrapped by the plugin hook normalizer, so
    // a thrown `undefined` reaches the summary formatter verbatim here too.
    const nullishBundle = await createVirtualBundle(browserApi, {});
    try {
      const failure = await nullishBundle
        .generate({
          entryFileNames: () => {
            throw undefined;
          },
        })
        .catch((error) => error);
      assert.ok(failure instanceof Error, 'A nullish rejection must still produce an Error');
      assert.doesNotMatch(failure.message, /Cannot convert undefined or null to object/);
      assert.match(failure.message, /Error: undefined/);
      assert.equal(failure.errors?.length, 1);
      assert.equal(failure.errors[0], undefined);
    } finally {
      await nullishBundle.close();
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

console.log(
  `OK: browser entries install the CurrentThread task and timer hosts through ` +
    `the @napi-rs/async-runtime installer bundled into ${browserLoader}`,
);

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
