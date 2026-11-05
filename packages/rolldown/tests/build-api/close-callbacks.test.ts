import { execa } from 'execa';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { rolldown, type NormalizedInputOptions, type NormalizedOutputOptions } from 'rolldown';
import { expect, test } from 'vitest';

// The close of a build releases the JS functions of its options.
// See internal-docs/bundler-data-lifecycle/implementation.md ("JS callback lifetime").

const input = path.join(import.meta.dirname, 'main.js');

// https://github.com/rolldown/rolldown/issues/11444
// The script runs one build for each case, so it gets more time than the default 20 s. The execa
// limit is lower, so a hung script fails with its own output.
test('a closed build does not keep its plugins alive', { timeout: 60_000 }, async () => {
  await execa(
    process.execPath,
    ['--expose-gc', fileURLToPath(new URL('./close-bundle-gc.mjs', import.meta.url))],
    { timeout: 50_000 },
  );
});

test('option getters read for the first time after close', async () => {
  let inputOptions: NormalizedInputOptions | undefined;
  let outputOptions: NormalizedOutputOptions | undefined;
  const entryFileNames = () => 'entry.js';
  const bundle = await rolldown({
    input,
    external: (id) => id === 'never',
    plugins: [
      {
        name: 'keep-options',
        buildStart(options) {
          inputOptions = options;
        },
        renderStart(options) {
          outputOptions = options;
        },
      },
    ],
  });
  await bundle.generate({ format: 'cjs', entryFileNames });
  await bundle.close();

  expect(inputOptions!.input).toEqual([input]);
  expect(inputOptions!.cwd).toBe(process.cwd());
  expect(outputOptions!.format).toBe('cjs');
  expect(outputOptions!.entryFileNames).toBe(entryFileNames);
});

test('closeBundle can still log through onLog', async () => {
  const logs: string[] = [];
  const bundle = await rolldown({
    input,
    onLog(level, log) {
      logs.push(`${level}:${log.message}`);
    },
    plugins: [
      {
        name: 'log-at-close',
        closeBundle() {
          this.warn('from closeBundle');
        },
      },
    ],
  });
  await bundle.generate({});
  await bundle.close();

  expect(logs).toContainEqual(expect.stringContaining('from closeBundle'));
});

test('a second close does not run closeBundle again', async () => {
  let closeBundleCalls = 0;
  const bundle = await rolldown({
    input,
    plugins: [
      {
        name: 'count-close',
        closeBundle() {
          closeBundleCalls++;
        },
      },
    ],
  });
  await bundle.generate({});
  await Promise.all([bundle.close(), bundle.close()]);
  await bundle.close();

  expect(closeBundleCalls).toBe(1);
});

// `bundle.close()` does not wait for a build that started before it. That build must still be
// able to call the option functions, so the release waits until the build ends.
test('a build that runs during close still calls the option functions', async () => {
  // With the WASI binding, the close of the last open bundle stops the shared async runtime, also
  // while a build is pending (https://github.com/rolldown/rolldown/issues/10639). This bundle stays
  // open, so that the pending build can end.
  const runtimeKeeper = await rolldown({ input });
  let buildStarted!: () => void;
  const started = new Promise<void>((resolve) => (buildStarted = resolve));
  let releaseBuild!: () => void;
  const paused = new Promise<void>((resolve) => (releaseBuild = resolve));
  let closeBundleCalls = 0;
  const bundle = await rolldown({
    input: 'entry',
    external: (id) => id === 'ext',
    plugins: [
      {
        name: 'close-during-build',
        async buildStart() {
          buildStarted();
          await paused;
        },
        resolveId(id) {
          return id === 'entry' ? id : null;
        },
        load(id) {
          return id === 'entry' ? "import ext from 'ext'; console.log(ext);" : null;
        },
        closeBundle() {
          closeBundleCalls++;
        },
      },
    ],
  });
  const pending = bundle.generate({});
  await started;
  await bundle.close();
  releaseBuild();

  const { output } = await pending;
  await runtimeKeeper.close();
  expect(output[0].code).toContain('from "ext"');
  expect(closeBundleCalls).toBe(1);
});
