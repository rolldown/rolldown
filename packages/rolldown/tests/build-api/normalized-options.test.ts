import path from 'node:path';
import { pathToFileURL } from 'node:url';
import v8 from 'node:v8';
import { rolldown, type Plugin } from 'rolldown';
import { isWasiTest } from 'rolldown-tests/utils';
import { expect, test } from 'vitest';
import { getBindingPath, loadBinding } from '../src/load-binding';

// See internal-docs/bundler-data-lifecycle/implementation.md ("Normalized options objects").

const input = path.join(import.meta.dirname, 'main.js');

// The class must come from the binding instance that `rolldown` uses. The native glue loads the
// `.node` file with `require`, so `loadBinding()` gets that instance. The WASI glue imports its
// `.cjs` file and vitest runs that import itself, so `require` would load a second instance there.
const { BindingNormalizedOptions }: ReturnType<typeof loadBinding> = isWasiTest
  ? await import(pathToFileURL(getBindingPath()).href)
  : loadBinding();

function reachableNativeOptions(): number {
  return v8.queryObjects(BindingNormalizedOptions, { format: 'count' });
}

// https://github.com/rolldown/rolldown/issues/11444
test('normalized options objects do not keep the native options', async () => {
  // Everything a user can keep stays reachable until the end of the test.
  const kept: unknown[] = [];
  let inHook = 0;
  const plugin = (): Plugin => ({
    name: 'keep-options',
    buildStart(options) {
      kept.push(options);
      // The hook argument is reachable here, so 0 would mean that the class is not the one that
      // the builds use.
      inHook = reachableNativeOptions();
    },
    renderStart(outputOptions, inputOptions) {
      kept.push(outputOptions, inputOptions);
    },
    generateBundle(outputOptions) {
      kept.push(outputOptions);
    },
  });
  async function build(close: boolean) {
    const bundle = await rolldown({ input, logLevel: 'silent', plugins: [plugin()] });
    kept.push(bundle, await bundle.generate({ format: 'es' }));
    if (close) await bundle.close();
  }

  await build(true);
  expect(inHook).toBeGreaterThan(0);
  // Compare with the count after the first build, not with 0: from the first native instance on,
  // V8 keeps one empty instance of the class in its template instantiation cache. The WASI binding
  // has no V8 templates, so the count is 0 there.
  const afterFirst = reachableNativeOptions();
  for (const close of [true, false]) await build(close);
  // Options objects that keep their native options add two per build: input and output.
  expect(reachableNativeOptions(), 'native options reachable after 3 builds').toBe(afterFirst);
  expect(kept).toHaveLength(3 * 6);
});
