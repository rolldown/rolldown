import { getDevWatchOptionsForCi } from '@rolldown/test-dev-server';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import type { InputOptions, OutputOptions } from 'rolldown';
import type { DevEngine, DevOptions } from 'rolldown/experimental';
import { dev as _dev } from 'rolldown/experimental';
import { isWasiTest } from 'rolldown-tests/utils';
import { expect, test } from 'vitest';

const TEST_TIMEOUT = 60_000;

function dev(
  inputOptions: InputOptions,
  outputOptions: OutputOptions,
  devOptions: DevOptions,
): Promise<DevEngine> {
  return _dev(inputOptions, outputOptions, {
    ...devOptions,
    watch: {
      ...devOptions.watch,
      watcher: {
        ...getDevWatchOptionsForCi().watcher,
        ...devOptions.watch?.watcher,
      },
    },
  });
}

// Regression test for https://github.com/rolldown/rolldown/issues/9365
//
// When the initial build fails, Vite's `waitForInitialBuildFinish` polls
// `ensureCurrentBuildFinish()` in a loop. If the user edits `vite.config.ts`
// during that loop, Vite restarts the server and closes the dev engine, but
// the polling loop is left running in the old environment. The old loop's
// next `ensureCurrentBuildFinish()` call then ran on a closed engine and
// rejected with `Dev engine is closed`. Vite's loop has no `.catch()`, so the
// rejection became an unhandled rejection and crashed the process.
//
// Fix: after `close()`, `ensureCurrentBuildFinish()` is a no-op rather than
// an error — there is no ongoing bundle to wait for.
//
// Under the wasm binding `close()` tears down the shared tokio runtime, so the no-op call panics with "Access tokio runtime failed in spawn". See https://github.com/rolldown/rolldown/issues/10639.
test.skipIf(isWasiTest)(
  'ensureCurrentBuildFinish after close resolves instead of rejecting',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-close-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    const input = path.join(dir, 'main.js');
    fs.writeFileSync(input, 'console.log(1)');

    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    const engine = await dev(
      {
        input,
        experimental: { devMode: true },
        plugins: [
          {
            name: 'force-failure',
            load() {
              // Reproduce the failing-load-hook from the original issue's
              // vite.config.ts so the initial build is in the "failed" state
              // at the moment of close.
              throw new Error('test error');
            },
          },
        ],
      },
      { dir: path.join(dir, 'dist') },
      {},
    );

    // Mirror Vite: kick off run() without awaiting, so the initial build is
    // in flight (and failing) when close() races with the next call.
    engine.run().catch(() => {});

    // Let the build fail at least once before closing.
    await engine.ensureCurrentBuildFinish();

    await engine.close();

    // After close, this call must resolve rather than reject. Vite calls it
    // from a polling loop without a `.catch()` handler — a rejection here
    // surfaces as an unhandled promise rejection that crashes the host.
    await expect(engine.ensureCurrentBuildFinish()).resolves.toBeUndefined();
  },
);

// The close of the dev engine releases the JS functions of the options. A repeated `close()`
// returns at once, but the release waits until the first close ends.
// See internal-docs/bundler-data-lifecycle/implementation.md ("JS callback lifetime").

function createInput(name: string, onTestFinished: (fn: () => void) => void) {
  const uniqueId = crypto.randomUUID().slice(0, 8);
  const dir = path.join(import.meta.dirname, 'temp', `${name}-${uniqueId}`);
  fs.mkdirSync(dir, { recursive: true });
  const input = path.join(dir, 'main.js');
  fs.writeFileSync(input, 'console.log(1)');
  onTestFinished(() => {
    if (!process.env.CI) {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });
  return { input, outDir: path.join(dir, 'dist') };
}

// This function fails the test after 5 s, so that the test does not hang until its timeout.
function withTimeout<T>(promise: Promise<T>, what: string): Promise<T> {
  return Promise.race([
    promise,
    new Promise<never>((_, reject) =>
      setTimeout(() => reject(new Error(`${what} did not end in 5 s`)), 5_000),
    ),
  ]);
}

const resolveExternal = (context: {
  resolve(id: string): Promise<{ external?: unknown } | null>;
}) =>
  context.resolve('ext').then(
    (result) => result?.external,
    (error: Error) => (error.cause as Error | undefined)?.message ?? error.message,
  );

test.skipIf(isWasiTest)(
  'a closeBundle hook that awaits engine.close() does not hang',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const { input, outDir } = createInput('dev-close-nested', onTestFinished);
    let engine!: DevEngine;
    let resolved: unknown;
    engine = await dev(
      {
        input,
        external: (id) => id === 'ext',
        experimental: { devMode: true },
        plugins: [
          {
            name: 'close-from-close-bundle',
            async closeBundle() {
              await engine.close();
              // The nested close returned, but the first close still runs this hook.
              resolved = await resolveExternal(this);
            },
          },
        ],
      },
      { dir: outDir },
      {},
    );
    engine.run().catch(() => {});
    await engine.ensureCurrentBuildFinish();

    await withTimeout(engine.close(), 'The close');
    expect(resolved).toBe(true);
  },
);

test.skipIf(isWasiTest)(
  'a second close does not release the option functions while closeBundle runs',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const { input, outDir } = createInput('dev-close-twice', onTestFinished);
    let hookStarted!: () => void;
    const started = new Promise<void>((resolve) => (hookStarted = resolve));
    let releaseHook!: () => void;
    const paused = new Promise<void>((resolve) => (releaseHook = resolve));
    let resolved: unknown;

    const engine = await dev(
      {
        input,
        external: (id) => id === 'ext',
        experimental: { devMode: true },
        plugins: [
          {
            name: 'paused-close-bundle',
            async closeBundle() {
              hookStarted();
              await paused;
              resolved = await resolveExternal(this);
            },
          },
        ],
      },
      { dir: outDir },
      {},
    );
    engine.run().catch(() => {});
    await engine.ensureCurrentBuildFinish();

    const first = engine.close();
    await started;
    await withTimeout(engine.close(), 'The second close');
    releaseHook();
    await withTimeout(first, 'The first close');
    expect(resolved).toBe(true);
  },
);

// The dev engine keeps its plugin driver after close, so a kept plugin context can still reach an
// option function. After `bundle.close()` the same call fails earlier, because that close drops the
// plugin driver.
test.skipIf(isWasiTest)(
  'a kept plugin context gets a clear error when an option function runs after close',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const { input, outDir } = createInput('dev-close-kept-context', onTestFinished);
    let context: { resolve(id: string): Promise<unknown> } | undefined;
    const keepContext = (value: typeof context) => {
      context = value;
    };
    const engine = await dev(
      {
        input,
        external: (id) => id === 'ext',
        experimental: { devMode: true },
        plugins: [
          {
            name: 'keep-context',
            buildStart() {
              keepContext(this);
            },
          },
        ],
      },
      { dir: outDir },
      {},
    );
    engine.run().catch(() => {});
    await engine.ensureCurrentBuildFinish();
    await expect(context!.resolve('ext')).resolves.toMatchObject({ id: 'ext', external: true });
    await withTimeout(engine.close(), 'The close');

    const error = await context!.resolve('ext').catch((error: unknown) => error);
    expect(error).toBeInstanceOf(Error);
    expect((error as Error).message).toBe('Errored while resolving "ext" in `this.resolve`.');
    expect(((error as Error).cause as Error).message).toContain(
      'external option: A JS function of the bundler options was called after the bundle closed.',
    );
  },
);
