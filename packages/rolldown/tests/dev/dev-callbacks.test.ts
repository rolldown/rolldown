import { getDevWatchOptionsForCi } from '@rolldown/test-dev-server';
import { isSingleThread } from '@tests/runtime-flavor';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import type { InputOptions, OutputOptions } from 'rolldown';
import type { DevEngine, DevOptions } from 'rolldown/experimental';
import { dev as createDevEngine } from 'rolldown/experimental';
import { expect, test } from 'vitest';

const TEST_TIMEOUT = 60_000;

function dev(
  inputOptions: InputOptions,
  outputOptions: OutputOptions,
  devOptions: DevOptions,
): Promise<DevEngine> {
  return createDevEngine(inputOptions, outputOptions, {
    ...devOptions,
    watch: {
      ...getDevWatchOptionsForCi(),
      ...devOptions.watch,
    },
  });
}

function createFixture(label: string, source = 'console.log(1)') {
  const dir = path.join(import.meta.dirname, 'temp', `${label}-${crypto.randomUUID().slice(0, 8)}`);
  fs.mkdirSync(dir, { recursive: true });
  const input = path.join(dir, 'main.js');
  fs.writeFileSync(input, source);
  return { dir, input, outputDir: path.join(dir, 'dist') };
}

async function editFile(filePath: string, source: string) {
  await new Promise((resolve) => setTimeout(resolve, 1_000));
  fs.writeFileSync(filePath, source);
}

async function settleWithin<T>(promise: Promise<T>, label: string): Promise<T> {
  let timeoutId: NodeJS.Timeout | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timeoutId = setTimeout(() => reject(new Error(`${label} did not settle`)), 10_000);
  });
  try {
    return await Promise.race([promise, timeout]);
  } finally {
    if (timeoutId) clearTimeout(timeoutId);
  }
}

test.skipIf(isSingleThread)(
  'run awaits onOutput and propagates an async rejection',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const { dir, input, outputDir } = createFixture('dev-async-output');
    const callbackError = new TypeError('async onOutput failed');
    let callbackStarted!: () => void;
    const started = new Promise<void>((resolve) => {
      callbackStarted = resolve;
    });
    let rejectCallback!: (error: unknown) => void;
    const callbackGate = new Promise<void>((_, reject) => {
      rejectCallback = reject;
    });

    const engine = await dev(
      { input, experimental: { devMode: true } },
      { dir: outputDir },
      {
        onOutput() {
          callbackStarted();
          return callbackGate;
        },
      },
    );
    onTestFinished(async () => {
      await engine.close().catch(() => {});
      if (!process.env.CI) fs.rmSync(dir, { recursive: true, force: true });
    });

    const runPromise = engine.run();
    await started;
    let runSettled = false;
    void runPromise.then(
      () => {
        runSettled = true;
      },
      () => {
        runSettled = true;
      },
    );
    await Promise.resolve();
    expect(runSettled).toBe(false);

    rejectCallback(callbackError);
    await expect(runPromise).rejects.toBe(callbackError);
  },
);

test.skipIf(isSingleThread)(
  'concurrent run calls both await the initial output callback',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const { dir, input, outputDir } = createFixture('dev-concurrent-run');
    let callbackStarted!: () => void;
    const started = new Promise<void>((resolve) => {
      callbackStarted = resolve;
    });
    let releaseCallback!: () => void;
    const callbackGate = new Promise<void>((resolve) => {
      releaseCallback = resolve;
    });

    const engine = await dev(
      { input, experimental: { devMode: true } },
      { dir: outputDir },
      {
        onOutput() {
          callbackStarted();
          return callbackGate;
        },
      },
    );
    onTestFinished(async () => {
      releaseCallback();
      await engine.close().catch(() => {});
      if (!process.env.CI) fs.rmSync(dir, { recursive: true, force: true });
    });

    const firstRun = engine.run();
    await started;
    const secondRun = engine.run();
    let firstSettled = false;
    let secondSettled = false;
    void firstRun.then(
      () => {
        firstSettled = true;
      },
      () => {
        firstSettled = true;
      },
    );
    void secondRun.then(
      () => {
        secondSettled = true;
      },
      () => {
        secondSettled = true;
      },
    );
    await new Promise((resolve) => setTimeout(resolve, 100));
    expect(firstSettled).toBe(false);
    expect(secondSettled).toBe(false);

    releaseCallback();
    await settleWithin(Promise.all([firstRun, secondRun]), 'concurrent run calls');
  },
);

test.skipIf(isSingleThread)(
  'close drains a rejecting run before closeBundle awaits it',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const { dir, input, outputDir } = createFixture('dev-close-drains-run');
    const callbackError = new TypeError('onOutput failed before dev close');
    let callbackStarted!: () => void;
    const started = new Promise<void>((resolve) => {
      callbackStarted = resolve;
    });
    let rejectCallback!: (error: unknown) => void;
    const callbackGate = new Promise<void>((_, reject) => {
      rejectCallback = reject;
    });
    let runPromise!: Promise<void>;
    let closeBundleCalls = 0;

    const engine = await dev(
      {
        input,
        experimental: { devMode: true },
        plugins: [
          {
            name: 'close-awaits-run',
            async closeBundle() {
              closeBundleCalls += 1;
              await runPromise.catch(() => {});
            },
          },
        ],
      },
      { dir: outputDir },
      {
        onOutput() {
          callbackStarted();
          return callbackGate;
        },
      },
    );
    onTestFinished(async () => {
      await engine.close().catch(() => {});
      if (!process.env.CI) fs.rmSync(dir, { recursive: true, force: true });
    });

    runPromise = engine.run();
    await started;
    const closePromise = engine.close();
    await Promise.resolve();
    rejectCallback(callbackError);

    const [runResult, closeResult] = await settleWithin(
      Promise.allSettled([runPromise, closePromise]),
      'run and close',
    );
    expect(runResult).toEqual({ status: 'rejected', reason: callbackError });
    expect(closeResult).toEqual({ status: 'rejected', reason: callbackError });
    expect(closeBundleCalls).toBe(1);
  },
);

test.skipIf(isSingleThread)(
  'close waits for a watcher callback and replays a late closeBundle failure',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const { dir, input, outputDir } = createFixture('dev-close-drains-watcher-callback');
    const closeError = new TypeError('closeBundle failed after watcher callback');
    let callbackStarted!: () => void;
    const started = new Promise<void>((resolve) => {
      callbackStarted = resolve;
    });
    let releaseCallback!: () => void;
    const callbackGate = new Promise<void>((resolve) => {
      releaseCallback = resolve;
    });
    let callbackCompleted = false;
    let closeBundleCalls = 0;

    let engine!: DevEngine;
    engine = await dev(
      {
        input,
        experimental: { devMode: true },
        plugins: [
          {
            name: 'late-watcher-close-failure',
            closeBundle() {
              closeBundleCalls += 1;
              throw closeError;
            },
          },
        ],
      },
      { dir: outputDir },
      {
        async onHmrUpdates() {
          callbackStarted();
          await callbackGate;
          await engine.close();
          callbackCompleted = true;
        },
      },
    );
    onTestFinished(async () => {
      releaseCallback();
      await engine.close().catch(() => {});
      if (!process.env.CI) fs.rmSync(dir, { recursive: true, force: true });
    });

    await engine.run();
    await editFile(input, 'console.log(2)');
    await started;

    const closePromise = engine.close();
    let closeSettled = false;
    void closePromise.then(
      () => {
        closeSettled = true;
      },
      () => {
        closeSettled = true;
      },
    );
    await new Promise((resolve) => setTimeout(resolve, 100));
    expect(closeSettled).toBe(false);
    expect(closeBundleCalls).toBe(0);

    releaseCallback();
    await expect(settleWithin(closePromise, 'watcher callback close')).rejects.toBe(closeError);
    expect(callbackCompleted).toBe(true);
    expect(closeBundleCalls).toBe(1);
    await expect(engine.close()).rejects.toBe(closeError);
  },
);

test.skipIf(isSingleThread)(
  'run propagates a synchronous onOutput throw',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const { dir, input, outputDir } = createFixture('dev-sync-output');
    const callbackError = new RangeError('sync onOutput failed');
    const engine = await dev(
      { input, experimental: { devMode: true } },
      { dir: outputDir },
      {
        onOutput() {
          throw callbackError;
        },
      },
    );
    onTestFinished(async () => {
      await engine.close().catch(() => {});
      if (!process.env.CI) fs.rmSync(dir, { recursive: true, force: true });
    });

    await expect(engine.run()).rejects.toBe(callbackError);
  },
);

test.skipIf(isSingleThread)(
  'close can be awaited inside onOutput',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const { dir, input, outputDir } = createFixture('dev-reentrant-output-close');
    let callbackCompleted = false;
    let engine!: DevEngine;
    engine = await dev(
      { input, experimental: { devMode: true } },
      { dir: outputDir },
      {
        async onOutput(result) {
          if (result instanceof Error) throw result;
          await engine.close();
          callbackCompleted = true;
        },
      },
    );
    onTestFinished(async () => {
      await engine.close().catch(() => {});
      if (!process.env.CI) fs.rmSync(dir, { recursive: true, force: true });
    });

    const runResult = await Promise.allSettled([engine.run()]);
    await engine.close();
    expect(callbackCompleted).toBe(true);
    expect(runResult).toHaveLength(1);
  },
);

test.skipIf(isSingleThread)(
  'two onOutput callbacks can close each opposing dev engine',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const fixtureA = createFixture('dev-cross-engine-close-a');
    const fixtureB = createFixture('dev-cross-engine-close-b');
    let engineA!: DevEngine;
    let engineB!: DevEngine;
    let callbackACompleted = false;
    let callbackBCompleted = false;

    engineA = await dev(
      { input: fixtureA.input, experimental: { devMode: true } },
      { dir: fixtureA.outputDir },
      {
        async onOutput(result) {
          if (result instanceof Error) throw result;
          await Promise.resolve();
          await engineB.close();
          callbackACompleted = true;
        },
      },
    );
    engineB = await dev(
      { input: fixtureB.input, experimental: { devMode: true } },
      { dir: fixtureB.outputDir },
      {
        async onOutput(result) {
          if (result instanceof Error) throw result;
          await Promise.resolve();
          await engineA.close();
          callbackBCompleted = true;
        },
      },
    );
    onTestFinished(async () => {
      await Promise.allSettled([engineA.close(), engineB.close()]);
      if (!process.env.CI) {
        fs.rmSync(fixtureA.dir, { recursive: true, force: true });
        fs.rmSync(fixtureB.dir, { recursive: true, force: true });
      }
    });

    await Promise.allSettled([engineA.run(), engineB.run()]);
    await Promise.all([engineA.close(), engineB.close()]);

    expect(callbackACompleted).toBe(true);
    expect(callbackBCompleted).toBe(true);
  },
);

test.skipIf(isSingleThread)(
  'ensureCurrentBuildFinish observes an async onHmrUpdates rejection',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const { dir, input, outputDir } = createFixture('dev-async-hmr');
    const callbackError = new Error('async onHmrUpdates failed');
    let callbackStarted!: () => void;
    const started = new Promise<void>((resolve) => {
      callbackStarted = resolve;
    });
    let rejectCallback!: (error: unknown) => void;
    const callbackGate = new Promise<void>((_, reject) => {
      rejectCallback = reject;
    });
    const engine = await dev(
      { input, experimental: { devMode: true } },
      { dir: outputDir },
      {
        onHmrUpdates() {
          callbackStarted();
          return callbackGate;
        },
      },
    );
    onTestFinished(async () => {
      await engine.close().catch(() => {});
      if (!process.env.CI) fs.rmSync(dir, { recursive: true, force: true });
    });

    await engine.run();
    await editFile(input, 'console.log(2)');
    await started;

    const buildFinished = engine.ensureCurrentBuildFinish();
    rejectCallback(callbackError);
    await expect(buildFinished).rejects.toBe(callbackError);
    await expect(engine.ensureCurrentBuildFinish()).rejects.toBe(callbackError);
  },
);

test.skipIf(isSingleThread)(
  'compileEntry awaits onAdditionalAssets and propagates its rejection',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const { dir, input, outputDir } = createFixture(
      'dev-async-additional-assets',
      "export const load = () => import('./lazy.js');",
    );
    const lazyInput = path.join(dir, 'lazy.js');
    fs.writeFileSync(lazyInput, 'export const value = 1;');
    const callbackError = new Error('async onAdditionalAssets failed');
    let callbackStarted!: () => void;
    const started = new Promise<void>((resolve) => {
      callbackStarted = resolve;
    });
    let rejectCallback!: (error: unknown) => void;
    const callbackGate = new Promise<void>((_, reject) => {
      rejectCallback = reject;
    });

    const engine = await dev(
      {
        input,
        experimental: { devMode: { lazy: true } },
        plugins: [
          {
            name: 'emit-lazy-asset',
            transform(_code, id) {
              if (id === lazyInput) {
                this.emitFile({
                  type: 'asset',
                  fileName: 'lazy-asset.txt',
                  source: 'lazy asset',
                });
              }
            },
          },
        ],
      },
      { dir: outputDir },
      {
        onAdditionalAssets() {
          callbackStarted();
          return callbackGate;
        },
      },
    );
    onTestFinished(async () => {
      await engine.close().catch(() => {});
      if (!process.env.CI) fs.rmSync(dir, { recursive: true, force: true });
    });

    await engine.run();
    const compilePromise = engine.compileEntry(`${lazyInput}?rolldown-lazy=1`, 'test-client');
    await started;
    let compileSettled = false;
    void compilePromise.then(
      () => {
        compileSettled = true;
      },
      () => {
        compileSettled = true;
      },
    );
    await Promise.resolve();
    expect(compileSettled).toBe(false);

    rejectCallback(callbackError);
    await expect(compilePromise).rejects.toBe(callbackError);
  },
);
