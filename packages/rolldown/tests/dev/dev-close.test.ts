import { getDevWatchOptionsForCi } from '@rolldown/test-dev-server';
import { isSingleThread } from '@tests/runtime-flavor';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import type { InputOptions, OutputOptions } from 'rolldown';
import type { DevEngine, DevOptions } from 'rolldown/experimental';
import { defineParallelPlugin, dev as _dev } from 'rolldown/experimental';
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
      ...getDevWatchOptionsForCi(),
      ...devOptions.watch,
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
// Dev mode spawns the BindingDevEngine, which is out of scope for the
// single-thread (CurrentThread) runtime flavor.
test.skipIf(isSingleThread)(
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

test.skipIf(isSingleThread)(
  'post-close methods settle without re-entering the runtime',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-post-close-${uniqueId}`);
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
      },
      { dir: path.join(dir, 'dist') },
      {},
    );

    await engine.run();
    await engine.close();

    const closedError = 'Dev engine is closed';
    await expect(engine.run()).rejects.toThrow(closedError);
    await expect(engine.ensureCurrentBuildFinish()).resolves.toBeUndefined();
    await expect(engine.getBundleState()).rejects.toThrow(closedError);
    await expect(engine.ensureLatestBuildOutput()).rejects.toThrow(closedError);
    expect(() => engine.triggerFullBuild()).toThrow(closedError);
    await expect(engine.invalidate(input)).rejects.toThrow(closedError);
    await expect(engine.registerModules('client', [input])).rejects.toThrow(closedError);
    await expect(engine.removeClient('client')).resolves.toBeUndefined();
    await expect(engine.compileEntry(input, 'client')).rejects.toThrow(closedError);
    await expect(engine.close()).resolves.toBeUndefined();
  },
);

test.skipIf(isSingleThread)(
  'removeClient becomes a no-op while close is in progress',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-remove-client-close-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    const input = path.join(dir, 'main.js');
    fs.writeFileSync(input, 'console.log(1)');

    const state = new Int32Array(new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT * 3));
    const parallelPlugin = defineParallelPlugin<{
      state: Int32Array;
    }>(path.join(import.meta.dirname, 'parallel-close-plugin.mjs'));
    const engine = await dev(
      {
        input,
        experimental: { devMode: true },
        plugins: [parallelPlugin({ state })],
      },
      { dir: path.join(dir, 'dist') },
      {},
    );

    onTestFinished(async () => {
      Atomics.store(state, 1, 1);
      Atomics.notify(state, 1);
      await engine.close().catch(() => {});
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    const runPromise = engine.run().catch(() => {});
    while (Atomics.load(state, 0) === 0) {
      await new Promise<void>((resolve) => setImmediate(resolve));
    }

    const closePromise = engine.close();
    await expect(engine.removeClient('late-client')).resolves.toBeUndefined();

    Atomics.store(state, 1, 1);
    Atomics.notify(state, 1);
    await Promise.all([runPromise, closePromise]);
  },
);

test.skipIf(isSingleThread)(
  'close preserves the terminal closeBundle failure',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-close-error-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    const input = path.join(dir, 'main.js');
    fs.writeFileSync(input, 'console.log(1)');
    const closeError = Object.assign(new TypeError('dev close terminal failure'), {
      closeCode: 'DEV_CLOSE_TERMINAL',
    });

    const engine = await dev(
      {
        input,
        experimental: { devMode: true },
        plugins: [
          {
            name: 'close-failure',
            closeBundle() {
              throw closeError;
            },
          },
        ],
      },
      { dir: path.join(dir, 'dist') },
      {},
    );

    onTestFinished(async () => {
      await engine.close().catch(() => {});
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await engine.run();
    const firstError = await engine.close().catch((error: unknown) => error);
    expect(firstError).toBe(closeError);
    expect(firstError).toBeInstanceOf(TypeError);
    expect((firstError as typeof closeError).closeCode).toBe('DEV_CLOSE_TERMINAL');

    const replayedError = await engine.close().catch((error: unknown) => error);
    expect(replayedError).toBe(closeError);
  },
);

test.skipIf(isSingleThread)(
  'close preserves callback and closeBundle failures',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-close-errors-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    const input = path.join(dir, 'main.js');
    fs.writeFileSync(input, 'console.log(1)');
    const callbackError = new TypeError('dev output callback failure');
    const closeError = new RangeError('dev closeBundle failure');
    let closeBundleCalls = 0;

    const engine = await dev(
      {
        input,
        experimental: { devMode: true },
        plugins: [
          {
            name: 'multiple-dev-close-failures',
            closeBundle() {
              closeBundleCalls += 1;
              throw closeError;
            },
          },
        ],
      },
      { dir: path.join(dir, 'dist') },
      {
        onOutput() {
          throw callbackError;
        },
      },
    );

    onTestFinished(async () => {
      await engine.close().catch(() => {});
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    const runError = await engine.run().catch((error: unknown) => error);
    expect(runError).toBe(callbackError);

    const [firstCloseError, concurrentCloseError] = await Promise.all([
      engine.close().catch((error: unknown) => error),
      engine.close().catch((error: unknown) => error),
    ]);
    expect(firstCloseError).toBe(concurrentCloseError);
    expect(firstCloseError).toBeInstanceOf(AggregateError);
    const closeErrors = (firstCloseError as AggregateError).errors;
    expect(closeErrors).toHaveLength(2);
    expect(closeErrors.filter((error) => error === callbackError)).toHaveLength(1);
    expect(closeErrors.filter((error) => error === closeError)).toHaveLength(1);
    expect(closeBundleCalls).toBe(1);

    const replayedCloseError = await engine.close().catch((error: unknown) => error);
    expect(replayedCloseError).toBe(firstCloseError);
    expect(closeBundleCalls).toBe(1);
  },
);

test.skipIf(isSingleThread)(
  'close waits for an active parallel-plugin build before terminating workers',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-parallel-close-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    const input = path.join(dir, 'main.js');
    fs.writeFileSync(input, 'console.log(1)');

    const state = new Int32Array(new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT * 3));
    const parallelPlugin = defineParallelPlugin<{
      state: Int32Array;
    }>(path.join(import.meta.dirname, 'parallel-close-plugin.mjs'));
    const engine = await dev(
      {
        input,
        experimental: { devMode: true },
        plugins: [parallelPlugin({ state })],
      },
      { dir: path.join(dir, 'dist') },
      {},
    );

    onTestFinished(async () => {
      Atomics.store(state, 1, 1);
      Atomics.notify(state, 1);
      await engine.close().catch(() => {});
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    let runRejected = false;
    let runSettled = false;
    let runError: unknown;
    // Closing an active initial build may cause `run()` to report the
    // cancellation. Before the hook starts, however, a rejection means the
    // production worker path failed to become usable and should fail fast.
    const runPromise = engine
      .run()
      .catch((error) => {
        runRejected = true;
        runError = error;
      })
      .finally(() => {
        runSettled = true;
      });
    while (Atomics.load(state, 0) === 0) {
      if (runRejected) throw runError;
      await new Promise<void>((resolve) => setImmediate(resolve));
    }

    let closeSettled = false;
    const closePromise = engine.close().finally(() => {
      closeSettled = true;
    });
    await new Promise<void>((resolve) => setImmediate(resolve));
    expect(closeSettled).toBe(false);
    expect(Atomics.load(state, 2)).toBe(0);
    await expect(engine.ensureCurrentBuildFinish()).resolves.toBeUndefined();
    await expect(engine.getBundleState()).rejects.toThrow('Dev engine is closed');
    expect(() => engine.triggerFullBuild()).toThrow('Dev engine is closed');

    Atomics.store(state, 1, 1);
    Atomics.notify(state, 1);

    await Promise.all([runPromise, closePromise]);
    expect(runSettled).toBe(true);
    expect(Atomics.load(state, 0)).toBeGreaterThan(0);
    expect(Atomics.load(state, 2)).toBe(Atomics.load(state, 0));
  },
);
