// Runs the packed @rolldown/browser inside a real browser page, the way a REPL or playground uses
// it: a Vite-served app imports the package through its `browser` export condition and bundles.
//
// This directory is the app under test. It installs the tarball from tests/fixtures/browser, so the
// bare `@rolldown/browser` specifier below resolves through the published package.json `exports`,
// not through the workspace source.
//
// Regression coverage for https://github.com/rolldown/rolldown/issues/10535, where a top-level
// `node:fs` / `node:url` import leaked into the browser build. Vite externalizes node builtins for
// the browser, so importing the package threw at load with "Module "node:url" has been externalized
// for browser compatibility". packages/rolldown/tests/exports-consistency.test.ts compares
// package.json keys; only running the package in a browser catches a leak like that.
import { beforeAll, expect, inject, test } from 'vitest';
import type * as RolldownBrowser from '@rolldown/browser';
import type * as RolldownBrowserExperimental from '@rolldown/browser/experimental';

// Vite serves every node builtin it externalizes for the browser from this id, so any of them in
// the loaded graph shows up as a fetched module regardless of how it was imported.
const BROWSER_EXTERNAL = '__vite-browser-external';

let rolldownBrowser: typeof RolldownBrowser;
let experimental: typeof RolldownBrowserExperimental;

// imported here rather than at the top level so a load-time failure is reported against this suite
// rather than as a bare collection error for the whole file
beforeAll(async () => {
  rolldownBrowser = await import('@rolldown/browser');
  experimental = await import('@rolldown/browser/experimental');
});

function loadedModules() {
  return performance.getEntriesByType('resource').map((entry) => entry.name);
}

test('the packed @rolldown/browser bundles in the browser', async () => {
  // CI runs the runner's system Chrome, which drifts with the runner image, so a failing run has to
  // record which browser it ran on
  console.log(navigator.userAgent);

  // the tarball is gitignored build output, so pin the version that actually loaded
  expect(rolldownBrowser.VERSION).toBe(inject('rolldownVersion'));

  // The `browser` condition points at dist/index.browser.mjs; the `default` one pulls in
  // node:worker_threads and the CJS binding. Both entries export the same names, so name the entry
  // that the page actually fetched rather than inferring it from the module's shape.
  const entries = loadedModules().filter((name) => name.includes('/@rolldown/browser/dist/index'));
  expect(entries.join('\n')).toContain('/@rolldown/browser/dist/index.browser.mjs');

  // A named import of an externalized builtin already throws while the package loads, which is how
  // #10535 surfaced. This also catches the shapes that do not throw, such as an unused default
  // import, because the browser still fetches Vite's stand-in module for them.
  const externalized = loadedModules().filter((name) => name.includes(BROWSER_EXTERNAL));
  expect(externalized, externalized.join('\n')).toHaveLength(0);

  // The editor buffers a playground would feed to the bundler: a plain browser page bundles them
  // callback-free through the wasm runtime's in-memory filesystem. JavaScript plugin hooks also
  // need host-backed async-context propagation (docs/guide/wasi.md), which system Chrome does not
  // ship, so the plugin round trip is asserted below as the documented preflight failure instead.
  const files: Record<string, string> = {
    '/entry.js': "import { hyperCube } from './hyper-cube.js';\nconsole.log(hyperCube(5));\n",
    '/hyper-cube.js':
      "import { cube } from './cube.js';\nexport function hyperCube(x) {\n  return cube(x) * x;\n}\n",
    '/cube.js': 'export function cube(x) {\n  return x * x * x;\n}\n',
  };

  const { memfs, getAsyncContextSupport } = experimental;
  if (!memfs) {
    throw new Error('the browser build must expose the wasm in-memory filesystem');
  }
  memfs.volume.fromJSON(files);

  const bundle = await rolldownBrowser.rolldown({
    input: '/entry.js',
    cwd: '/',
  });

  try {
    const { output } = await bundle.generate({ format: 'esm' });

    expect(output).toHaveLength(1);
    const [{ fileName, code }] = output;
    expect(fileName).toBe('entry.js');

    // the whole graph is inlined into one chunk, and the leaf lands before its consumers
    expect(code).toContain('function cube(x)');
    expect(code).toContain('function hyperCube(x)');
    expect(code).toContain('console.log(hyperCube(5))');
    expect(code.indexOf('function cube(x)')).toBeLessThan(code.indexOf('function hyperCube(x)'));
    expect(code).not.toContain('import');
  } finally {
    await bundle.close();
  }

  // A callback-bearing build must fail the async-context preflight BEFORE invoking any user hook.
  // If this section ever flips because the runner's Chrome ships AsyncContext.Variable, upgrade it
  // to run the full JS plugin round trip instead.
  expect(getAsyncContextSupport()).toEqual({ source: 'unavailable', supported: false });
  let hookCalls = 0;
  const callbackError: unknown = await rolldownBrowser
    .rolldown({
      input: '/entry.js',
      cwd: '/',
      plugins: [
        {
          name: 'callback-probe',
          load() {
            hookCalls += 1;
            return null;
          },
        },
      ],
    })
    .then(
      async (callbackBundle) => {
        try {
          await callbackBundle.generate({ format: 'esm' });
          return new Error('callback-bearing build unexpectedly succeeded');
        } catch (error) {
          return error;
        } finally {
          await callbackBundle.close();
        }
      },
      (error) => error,
    );
  expect(hookCalls, 'the async-context preflight must reject before user hooks run').toBe(0);
  expect(callbackError).toMatchObject({
    name: 'AsyncContextUnavailableError',
    code: 'ERR_ROLLDOWN_ASYNC_CONTEXT_UNAVAILABLE',
  });
});
