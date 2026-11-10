import { execa } from 'execa';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  rolldown,
  type NormalizedInputOptions,
  type NormalizedOutputOptions,
  type OutputOptions,
  type Plugin,
} from 'rolldown';
import { expect, test } from 'vitest';

// The normalized options objects hold plain values, not the native options.
// See internal-docs/bundler-data-lifecycle/implementation.md ("Normalized options objects").

const input = path.join(import.meta.dirname, 'main.js');

// https://github.com/rolldown/rolldown/issues/11444
// The script runs one build for each case, so it gets more time than the default. The execa limit
// is lower, so a hung script fails with its own output.
test(
  'normalized options objects do not keep the plugins of a build alive',
  { timeout: 60_000 },
  async () => {
    await execa(
      process.execPath,
      ['--expose-gc', fileURLToPath(new URL('./normalized-options-gc.mjs', import.meta.url))],
      { timeout: 50_000 },
    );
  },
);

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
