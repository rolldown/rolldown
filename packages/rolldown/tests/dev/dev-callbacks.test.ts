import { getDevWatchOptionsForCi } from '@rolldown/test-dev-server';
import { isSingleThread } from '@tests/runtime-flavor';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import type { InputOptions, OutputOptions } from 'rolldown';
import type { DevEngine, DevOptions } from 'rolldown/experimental';
import { dev as createDevEngine } from 'rolldown/experimental';
import { expect, test } from 'vitest';
import { BindingDevEngine } from '../../src/binding.cjs';
import { acquireRuntimeLease } from '../../src/runtime-lifecycle';
import { createBundlerOptions } from '../../src/utils/create-bundler-option';
import { normalizeBindingResultErrors } from '../../src/utils/error';

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
  'raw BindingDevEngine acknowledges callback close and replays terminal errors',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const { dir, input, outputDir } = createFixture('raw-dev-reentrant-output-close');
    const closeError = new TypeError('raw dev closeBundle failure');
    const { bundlerOptions, stopWorkers } = await createBundlerOptions(
      {
        input,
        experimental: { devMode: true },
        plugins: [
          {
            name: 'raw-close-failure',
            closeBundle() {
              throw closeError;
            },
          },
        ],
      },
      { dir: outputDir },
      false,
    );
    const runtimeLease = await acquireRuntimeLease();
    let callbackCompleted = false;
    let engine!: BindingDevEngine;
    engine = new BindingDevEngine(bundlerOptions, {
      async onOutput(result) {
        expect(normalizeBindingResultErrors(result)).toEqual([]);
        const acknowledgedClose = await engine.close();
        expect(normalizeBindingResultErrors(acknowledgedClose)).toEqual([]);
        await engine.removeClient('late-client');
        await expect(engine.registerModules('late-client', [input])).rejects.toThrow(
          'Dev engine is closed',
        );
        callbackCompleted = true;
      },
      watch: getDevWatchOptionsForCi(),
    });

    onTestFinished(async () => {
      await engine.close().catch(() => {});
      await stopWorkers?.();
      runtimeLease.release();
      if (!process.env.CI) fs.rmSync(dir, { recursive: true, force: true });
    });

    const runResult = await settleWithin(engine.run(), 'raw binding run');
    expect(normalizeBindingResultErrors(runResult)).toEqual([]);
    expect(callbackCompleted).toBe(true);

    const terminalClose = await settleWithin(engine.close(), 'raw binding terminal close');
    expect(normalizeBindingResultErrors(terminalClose)).toEqual([closeError]);
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
