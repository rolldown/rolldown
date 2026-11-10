import path from 'node:path';
import { pathToFileURL } from 'node:url';
import v8 from 'node:v8';
import {
  rolldown,
  type NormalizedInputOptions,
  type NormalizedOutputOptions,
  type OutputOptions,
  type Plugin,
} from 'rolldown';
import { expect, test } from 'vitest';
import { getBindingPath, loadBinding } from '../src/load-binding';

// The normalized options objects hold plain values, not the native options.
// See internal-docs/bundler-data-lifecycle/implementation.md ("Normalized options objects").

const input = path.join(import.meta.dirname, 'main.js');

// The class must come from the binding instance that `rolldown` uses. The native glue loads the
// `.node` file with `require`, so `loadBinding()` gets that instance. The WASI glue imports its
// `.cjs` file and vitest runs that import itself, so `require` would load a second instance there.
const { BindingNormalizedOptions }: ReturnType<typeof loadBinding> = process.env.ROLLDOWN_WASI_TEST
  ? await import(pathToFileURL(getBindingPath()).href)
  : loadBinding();

// The number of native options objects that are reachable from the GC roots. This is a property of
// the object graph, not of what a GC happened to free.
function reachableNativeOptions(): number {
  return v8.queryObjects(BindingNormalizedOptions, { format: 'count' });
}

// https://github.com/rolldown/rolldown/issues/11444
// The native options own the JS option functions, which are GC roots that reach the options objects
// again. So an options object that keeps the native options keeps the plugins of its build alive.
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
  for (const close of [true, false, true, false, false]) await build(close);
  // Options objects that keep their native options add two per build: input and output.
  expect(reachableNativeOptions(), 'native options reachable after 6 builds').toBe(afterFirst);
  expect(kept).toHaveLength(6 * 6);
});

type Options = NormalizedInputOptions | NormalizedOutputOptions;

// Each build makes new functions (for example the default addons), so compare only their kind.
function plain(value: unknown): unknown {
  if (typeof value === 'function') return 'function';
  if (Array.isArray(value)) return value.map(plain);
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, plain(item)]));
  }
  return value;
}

function readAll(options: Options): Record<string, unknown> {
  const values: Record<string, unknown> = {};
  for (const key of Object.keys(options)) {
    const value: unknown = Reflect.get(options, key);
    // `inner` is internal state, not an option.
    if (key !== 'inner') values[key] = plain(value);
  }
  return values;
}

test('normalized options read for the first time after close', async () => {
  let readInHooks = false;
  let inputOptions: NormalizedInputOptions | undefined;
  let outputOptions: NormalizedOutputOptions | undefined;
  const inHooks: { keys: string[]; values: Record<string, unknown> }[] = [];
  const keep = (options: Options) => {
    if (readInHooks) inHooks.push({ keys: Object.keys(options), values: readAll(options) });
    return options;
  };
  // One plugin object for both builds, so that the `plugins` values are equal.
  const plugin: Plugin = {
    name: 'keep-options',
    buildStart(options) {
      inputOptions = keep(options) as NormalizedInputOptions;
    },
    renderStart(options) {
      outputOptions = keep(options) as NormalizedOutputOptions;
    },
  };
  const output: OutputOptions = {
    format: 'cjs',
    sourcemap: true,
    minify: 'dce-only',
    banner: () => '/* banner */',
    entryFileNames: () => 'entry.js',
  };
  async function build() {
    const bundle = await rolldown({ input, platform: 'browser', plugins: [plugin] });
    try {
      await bundle.generate(output);
    } finally {
      await bundle.close();
    }
  }

  await build();
  const afterClose = [inputOptions!, outputOptions!].map((options) => ({
    keys: Object.keys(options),
    values: readAll(options),
  }));

  expect(outputOptions!.banner).toBe(output.banner);
  expect(outputOptions!.entryFileNames).toBe(output.entryFileNames);

  readInHooks = true;
  await build();

  expect(afterClose).toEqual(inHooks);
  expect(afterClose[0].values).toMatchObject({ platform: 'browser', input: [input] });
  expect(afterClose[1].values).toMatchObject({
    format: 'cjs',
    sourcemap: true,
    minify: 'dce-only',
  });
});
