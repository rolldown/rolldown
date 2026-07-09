import path from 'node:path';
import { Worker } from 'node:worker_threads';
import { build, rolldown } from 'rolldown';
import { defineParallelPlugin } from 'rolldown/experimental';
import { expect, test, vi } from 'vitest';

test('rolldown write twice', async () => {
  const bundle = await rolldown({
    input: './main.js',
    cwd: import.meta.dirname,
  });
  const esmOutput = await bundle.write({
    format: 'esm',
    entryFileNames: 'main.mjs',
  });
  expect(await bundle.watchFiles).toStrictEqual([path.join(import.meta.dirname, 'main.js')]);
  expect(esmOutput.output[0].fileName).toBe('main.mjs');
  expect(esmOutput.output[0].code).toBeDefined();

  const output = await bundle.write({
    format: 'iife',
    entryFileNames: 'main.js',
  });
  expect(output.output[0].fileName).toBe('main.js');
  expect(output.output[0].code.includes('(function() {')).toBe(true);
});

test('rolldown concurrent write', async () => {
  const bundle = await rolldown({
    input: ['./main.js'],
    cwd: import.meta.dirname,
  });
  await write();
  // Execute twice
  await write();

  async function write() {
    await Promise.all([
      bundle.write({ format: 'esm', dir: './dist' }),
      bundle.write({
        format: 'cjs',
        dir: './dist',
        entryFileNames: 'main.cjs',
      }),
    ]);
  }
});

test('concurrent outputs are not serialized for their full build futures', async () => {
  let buildStarts = 0;
  let releaseBuilds!: () => void;
  const bothBuildsStarted = new Promise<void>((resolve) => {
    releaseBuilds = resolve;
  });
  const bundle = await rolldown({
    input: './main.js',
    cwd: import.meta.dirname,
    plugins: [
      {
        name: 'concurrent-output-barrier',
        async buildStart() {
          buildStarts += 1;
          if (buildStarts === 2) {
            releaseBuilds();
          }
          await bothBuildsStarted;
        },
      },
    ],
  });

  await Promise.all([bundle.generate({ format: 'esm' }), bundle.generate({ format: 'cjs' })]);
  expect(buildStarts).toBe(2);
  await bundle.close();
});

test(
  'public outputs retry admission and retain an older failure-triggered closeBundle failure',
  { timeout: 5_000 },
  async () => {
    const buildError = new Error('first output build failure');
    const olderCloseError = new TypeError('first output closeBundle failure');
    const latestCloseError = new RangeError('latest output closeBundle failure');
    let buildCalls = 0;
    let closeCalls = 0;
    let markOlderCloseStarted!: () => void;
    const olderCloseStarted = new Promise<void>((resolve) => {
      markOlderCloseStarted = resolve;
    });
    let releaseOlderClose!: () => void;
    const olderCloseRelease = new Promise<void>((resolve) => {
      releaseOlderClose = resolve;
    });
    const bundle = await rolldown({
      input: './main.js',
      cwd: import.meta.dirname,
      plugins: [
        {
          name: 'failure-close-admission-retry',
          buildStart() {
            buildCalls += 1;
            if (buildCalls === 1) {
              throw buildError;
            }
          },
          async closeBundle() {
            closeCalls += 1;
            if (closeCalls === 1) {
              markOlderCloseStarted();
              await olderCloseRelease;
              throw olderCloseError;
            }
            throw latestCloseError;
          },
        },
      ],
    });

    try {
      const failedOutput = bundle.generate();
      await olderCloseStarted;

      let laterOutputSettled = false;
      const laterOutput = bundle.generate().finally(() => {
        laterOutputSettled = true;
      });
      await new Promise<void>((resolve) => setImmediate(resolve));
      expect(laterOutputSettled).toBe(false);

      releaseOlderClose();
      await expect(failedOutput).rejects.toMatchObject({
        errors: [buildError],
      });
      await expect(laterOutput).resolves.toBeDefined();

      const closeError = await bundle.close().catch((error: unknown) => error);
      expect(closeError).toBeInstanceOf(AggregateError);
      expect((closeError as AggregateError).errors).toEqual([olderCloseError, latestCloseError]);
      expect(closeCalls).toBe(2);
    } finally {
      releaseOlderClose();
      await bundle.close().catch(() => {});
    }
  },
);

test.each(['generate', 'write'] as const)(
  'closeBundle nested %s rejects without waiting for its failed output',
  { timeout: 10_000 },
  async (operationName) => {
    const buildError = new Error(`${operationName} source output failure`);
    let buildCalls = 0;
    let closeCalls = 0;
    let nestedError: unknown;
    let markNestedAttemptFinished!: () => void;
    const nestedAttemptFinished = new Promise<void>((resolve) => {
      markNestedAttemptFinished = resolve;
    });
    let bundle!: Awaited<ReturnType<typeof rolldown>>;
    bundle = await rolldown({
      input: './main.js',
      cwd: import.meta.dirname,
      plugins: [
        {
          name: `close-bundle-nested-${operationName}`,
          buildStart() {
            buildCalls += 1;
            if (buildCalls === 1) {
              throw buildError;
            }
          },
          async closeBundle() {
            closeCalls += 1;
            if (closeCalls !== 1) return;
            const nestedOutput =
              operationName === 'write'
                ? bundle.write({ dir: path.join(import.meta.dirname, 'dist', operationName) })
                : bundle.generate();
            nestedError = await nestedOutput.then(
              () => new Error(`nested ${operationName} unexpectedly succeeded`),
              (error: unknown) => error,
            );
            markNestedAttemptFinished();
          },
        },
      ],
    });

    try {
      await expect(
        settleWithin(bundle.generate(), `${operationName} failed output`),
      ).rejects.toMatchObject({
        errors: [buildError],
      });
      await settleWithin(nestedAttemptFinished, `closeBundle nested ${operationName} rejection`);
      expect(nestedError).toBeInstanceOf(Error);
      expect((nestedError as Error).message).toContain(
        'Cannot start a new output while closeBundle is still running for a failed output.',
      );

      await expect(
        settleWithin(bundle.generate(), `${operationName} output after failure close`),
      ).resolves.toBeDefined();
      await settleWithin(bundle.close(), `${operationName} bundle close`);
      expect(closeCalls).toBe(2);
    } finally {
      await bundle.close().catch(() => {});
    }
  },
);

test('close waits for output setup and native build entry', { timeout: 5_000 }, async () => {
  let releaseOutputSetup!: () => void;
  const delayedOutputPlugin = new Promise<{ name: string }>((resolve) => {
    releaseOutputSetup = () => resolve({ name: 'delayed-output-setup' });
  });
  const bundle = await rolldown({
    input: './main.js',
    cwd: import.meta.dirname,
  });

  const generatePromise = bundle.generate({ plugins: [delayedOutputPlugin] });
  let closeSettled = false;
  const closePromise = bundle.close();
  const concurrentClosePromise = bundle.close();
  expect(concurrentClosePromise).toBe(closePromise);
  const observedClosePromise = closePromise.finally(() => {
    closeSettled = true;
  });
  await Promise.resolve();
  expect(closeSettled).toBe(false);

  releaseOutputSetup();
  await expect(generatePromise).resolves.toBeDefined();
  await expect(observedClosePromise).resolves.toBeUndefined();
  await expect(concurrentClosePromise).resolves.toBeUndefined();
});

test('bundle.close() can be awaited from active JS callbacks', { timeout: 5_000 }, async () => {
  const completedCallbacks: string[] = [];
  let bundle!: Awaited<ReturnType<typeof rolldown>>;
  bundle = await rolldown({
    input: './main.js',
    cwd: import.meta.dirname,
    plugins: [
      {
        name: 'reentrant-close',
        async buildStart() {
          await bundle.close();
          completedCallbacks.push('buildStart');
        },
        async closeBundle() {
          await bundle.close();
          completedCallbacks.push('closeBundle');
        },
      },
    ],
  });

  await bundle.generate({
    banner: async () => {
      await bundle.close();
      completedCallbacks.push('banner');
      return '';
    },
  });
  await bundle.close();

  expect(completedCallbacks).toEqual(['buildStart', 'banner', 'closeBundle']);
});

test('a callback awaits and preserves an unrelated bundle close failure', async () => {
  const closeError = new Error('unrelated bundle close failed');
  let markCloseStarted!: () => void;
  const closeStarted = new Promise<void>((resolve) => {
    markCloseStarted = resolve;
  });
  let releaseClose!: () => void;
  const closeRelease = new Promise<void>((resolve) => {
    releaseClose = resolve;
  });
  const bundleB = await rolldown({
    input: './main.js',
    cwd: import.meta.dirname,
    plugins: [
      {
        name: 'unrelated-close-target',
        async closeBundle() {
          markCloseStarted();
          await closeRelease;
          throw closeError;
        },
      },
    ],
  });
  const bundleA = await rolldown({
    input: './main.js',
    cwd: import.meta.dirname,
    plugins: [
      {
        name: 'unrelated-close-source',
        async closeBundle() {
          await bundleB.close();
        },
      },
    ],
  });

  try {
    await Promise.all([bundleA.generate(), bundleB.generate()]);
    let closeSettled = false;
    const closeA = bundleA.close().finally(() => {
      closeSettled = true;
    });
    await closeStarted;
    await new Promise<void>((resolve) => setImmediate(resolve));
    expect(closeSettled).toBe(false);

    releaseClose();
    await expect(closeA).rejects.toBe(closeError);
  } finally {
    releaseClose();
    await Promise.allSettled([bundleA.close(), bundleB.close()]);
  }
});

test('concurrent outputs retain and terminate every parallel worker pool', async () => {
  const originalTerminate = Object.getOwnPropertyDescriptor(Worker.prototype, 'terminate')!
    .value as (this: Worker) => Promise<number>;
  const terminateCalls = new Map<Worker, number>();
  const terminateSpy = vi
    .spyOn(Worker.prototype, 'terminate')
    .mockImplementation(function (this: Worker) {
      terminateCalls.set(this, (terminateCalls.get(this) ?? 0) + 1);
      return Reflect.apply(originalTerminate, this, []);
    });
  const state = new Int32Array(new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT * 2));
  const parallelPlugin = defineParallelPlugin<{ state: Int32Array }>(
    path.join(import.meta.dirname, 'parallel-close-plugin.mjs'),
  );
  const bundle = await rolldown({
    input: './main.js',
    cwd: import.meta.dirname,
    plugins: [parallelPlugin({ state })],
  });

  try {
    await Promise.all([bundle.generate(), bundle.generate()]);
    const initializedWorkers = Atomics.load(state, 0);
    expect(initializedWorkers).toBeGreaterThan(0);

    await bundle.close();
    expect(terminateCalls.size).toBe(initializedWorkers);
    expect([...terminateCalls.values()]).toEqual(
      Array.from({ length: initializedWorkers }, () => 1),
    );
  } finally {
    await bundle.close().catch(() => {});
    terminateSpy.mockRestore();
  }
});

test('close retries a superseded worker pool after cleanup failure', async () => {
  const cleanupError = new Error('superseded worker termination failed');
  const originalTerminate = Object.getOwnPropertyDescriptor(Worker.prototype, 'terminate')!
    .value as (this: Worker) => Promise<number>;
  const terminateCalls = new Map<Worker, number>();
  const failedWorkers = new WeakSet<Worker>();
  let injectedFailure = false;
  const terminateSpy = vi
    .spyOn(Worker.prototype, 'terminate')
    .mockImplementation(function (this: Worker) {
      terminateCalls.set(this, (terminateCalls.get(this) ?? 0) + 1);
      if (!injectedFailure) {
        injectedFailure = true;
        failedWorkers.add(this);
        return Promise.reject(cleanupError);
      }
      return Reflect.apply(originalTerminate, this, []);
    });
  const state = new Int32Array(new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT * 2));
  const parallelPlugin = defineParallelPlugin<{ state: Int32Array }>(
    path.join(import.meta.dirname, 'parallel-close-plugin.mjs'),
  );
  const bundle = await rolldown({
    input: './main.js',
    cwd: import.meta.dirname,
    plugins: [parallelPlugin({ state })],
  });

  try {
    await bundle.generate();
    await expect(bundle.generate()).rejects.toBe(cleanupError);

    await expect(bundle.close()).resolves.toBeUndefined();
    expect(terminateCalls.size).toBe(Atomics.load(state, 0));
    for (const [worker, calls] of terminateCalls) {
      expect(calls).toBe(failedWorkers.has(worker) ? 2 : 1);
    }
  } finally {
    await bundle.close().catch(() => {});
    terminateSpy.mockRestore();
  }
});

test('should support `Symbol.asyncDispose` of the rolldown bundle and set closed state to true', async () => {
  const bundle = await rolldown({
    input: ['./main.js'],
    cwd: import.meta.dirname,
  });
  await bundle.generate();
  await bundle[Symbol.asyncDispose]();
  expect(bundle.closed).toBe(true);
});

test('passes errors from closeBundle hook', async () => {
  let handledError = false;
  try {
    const bundle = await rolldown({
      input: './main.js',
      cwd: import.meta.dirname,
      plugins: [
        {
          name: 'test',
          closeBundle() {
            this.error('close bundle error');
          },
        },
      ],
    });
    await bundle.generate();
    await bundle.close();
  } catch (error: any) {
    expect(error.message).toBe('close bundle error');
    handledError = true;
  } finally {
    expect(handledError).toBeTruthy();
  }
});

test('build preserves both the primary build failure and cleanup failure', async () => {
  const buildError = new TypeError('primary build failed');
  const closeError = new RangeError('cleanup close failed');

  const error = await build({
    input: './main.js',
    cwd: import.meta.dirname,
    write: false,
    plugins: [
      {
        name: 'dual-build-failure',
        renderStart() {
          throw buildError;
        },
        closeBundle() {
          throw closeError;
        },
      },
    ],
  }).catch((error: unknown) => error);

  expect(error).toBeInstanceOf(AggregateError);
  const aggregate = error as AggregateError;
  expect(aggregate.errors[0]).toBeInstanceOf(Error);
  expect((aggregate.errors[0] as Error).message).toContain('primary build failed');
  expect((aggregate.errors[0] as { errors: unknown[] }).errors[0]).toBe(buildError);
  expect(aggregate.errors[1]).toBe(closeError);
  expect(aggregate.cause).toBe(aggregate.errors[0]);
  expect(aggregate.message).toBe('Build and cleanup both failed');
});

test('build preserves a lone primary or cleanup failure', async () => {
  const buildError = new Error('primary-only failure');
  const closeError = new Error('cleanup-only failure');

  const primaryOnly = await build({
    input: './main.js',
    cwd: import.meta.dirname,
    write: false,
    plugins: [
      {
        name: 'primary-only-failure',
        renderStart() {
          throw buildError;
        },
      },
    ],
  }).catch((error: unknown) => error);
  expect(primaryOnly).toBeInstanceOf(Error);
  expect((primaryOnly as Error).message).toContain('primary-only failure');
  expect((primaryOnly as { errors: unknown[] }).errors[0]).toBe(buildError);

  const cleanupOnly = await build({
    input: './main.js',
    cwd: import.meta.dirname,
    write: false,
    plugins: [
      {
        name: 'cleanup-only-failure',
        closeBundle() {
          throw closeError;
        },
      },
    ],
  }).catch((error: unknown) => error);
  expect(cleanupOnly).toBe(closeError);
});

test('build option arrays finish cleanup before starting the next option', async () => {
  let firstClosed = false;
  let secondOptionsSawFirstClosed = false;
  const virtualEntry = (id: string) => ({
    name: `virtual-${id}`,
    resolveId(source: string) {
      if (source === id) return `\0${id}`;
    },
    load(source: string) {
      if (source === `\0${id}`) return 'export default 1';
    },
  });

  const outputs = await build([
    {
      input: 'first',
      plugins: [
        virtualEntry('first'),
        {
          name: 'observe-first-close',
          closeBundle() {
            firstClosed = true;
          },
        },
      ],
      write: false,
    },
    {
      input: 'second',
      plugins: [
        {
          name: 'observe-sequential-options',
          options(options) {
            secondOptionsSawFirstClosed = firstClosed;
            return options;
          },
        },
        virtualEntry('second'),
      ],
      write: false,
    },
  ]);

  expect(outputs).toHaveLength(2);
  expect(secondOptionsSawFirstClosed).toBe(true);
});

test('supports closeBundle hook', async () => {
  let closeBundleCalls = 0;
  try {
    const bundle = await rolldown({
      input: './main.js',
      cwd: import.meta.dirname,
      plugins: [
        {
          name: 'test',
          closeBundle() {
            closeBundleCalls++;
          },
        },
      ],
    });
    await bundle.generate();
    await bundle.close();
  } finally {
    expect(closeBundleCalls).toBe(1);
  }
});

test('parallel closeBundle hooks run before workers terminate', async () => {
  const state = new Int32Array(new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT * 2));
  const parallelPlugin = defineParallelPlugin<{ state: Int32Array }>(
    path.join(import.meta.dirname, 'parallel-close-plugin.mjs'),
  );
  const bundle = await rolldown({
    input: './main.js',
    cwd: import.meta.dirname,
    plugins: [parallelPlugin({ state })],
  });

  await bundle.generate();
  const workerCount = Atomics.load(state, 0);
  expect(workerCount).toBeGreaterThan(0);
  await bundle.close();
  expect(Atomics.load(state, 1)).toBe(workerCount);
});

test('failed builds keep parallel workers alive through closeBundle', async () => {
  const state = new Int32Array(new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT * 2));
  const parallelPlugin = defineParallelPlugin<{ state: Int32Array; failRender: boolean }>(
    path.join(import.meta.dirname, 'parallel-close-plugin.mjs'),
  );
  const bundle = await rolldown({
    input: './main.js',
    cwd: import.meta.dirname,
    plugins: [parallelPlugin({ state, failRender: true })],
  });

  await expect(bundle.generate()).rejects.toThrow('parallel render failure');
  const workerCount = Atomics.load(state, 0);
  expect(workerCount).toBeGreaterThan(0);
  expect(Atomics.load(state, 1)).toBe(0);

  await bundle.close();
  expect(Atomics.load(state, 1)).toBe(workerCount);
});

test(
  'superseded failed builds close parallel plugins before terminating their workers',
  { timeout: 10_000 },
  async () => {
    const originalTerminate = Object.getOwnPropertyDescriptor(Worker.prototype, 'terminate')!
      .value as (this: Worker) => Promise<number>;
    const state = new Int32Array(new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT * 2));
    const closeCountsAtTermination: number[] = [];
    const terminateSpy = vi
      .spyOn(Worker.prototype, 'terminate')
      .mockImplementation(function (this: Worker) {
        closeCountsAtTermination.push(Atomics.load(state, 1));
        return Reflect.apply(originalTerminate, this, []);
      });
    const parallelPlugin = defineParallelPlugin<{ state: Int32Array }>(
      path.join(import.meta.dirname, 'parallel-close-plugin.mjs'),
    );
    const firstBuildError = new Error('superseded parallel build failed');
    let buildCalls = 0;
    let markFirstStarted!: () => void;
    const firstStarted = new Promise<void>((resolve) => {
      markFirstStarted = resolve;
    });
    let releaseFirst!: () => void;
    const firstRelease = new Promise<void>((resolve) => {
      releaseFirst = resolve;
    });
    let markSecondStarted!: () => void;
    const secondStarted = new Promise<void>((resolve) => {
      markSecondStarted = resolve;
    });
    let releaseSecond!: () => void;
    const secondRelease = new Promise<void>((resolve) => {
      releaseSecond = resolve;
    });
    const bundle = await rolldown({
      input: './main.js',
      cwd: import.meta.dirname,
      plugins: [
        parallelPlugin({ state }),
        {
          name: 'superseded-failure-barrier',
          async buildStart() {
            buildCalls += 1;
            if (buildCalls === 1) {
              markFirstStarted();
              await firstRelease;
              throw firstBuildError;
            }
            if (buildCalls === 2) {
              markSecondStarted();
              await secondRelease;
            }
          },
        },
      ],
    });

    try {
      const failedOutput = bundle.generate();
      await firstStarted;
      const workersPerBuild = Atomics.load(state, 0);
      expect(workersPerBuild).toBeGreaterThan(0);

      const latestOutput = bundle.generate();
      await secondStarted;
      expect(Atomics.load(state, 0)).toBe(workersPerBuild * 2);

      releaseFirst();
      await expect(
        settleWithin(failedOutput, 'superseded failed parallel output'),
      ).rejects.toMatchObject({
        errors: [firstBuildError],
      });
      expect(Atomics.load(state, 1)).toBe(0);
      expect([...closeCountsAtTermination]).toEqual([]);

      releaseSecond();
      await settleWithin(latestOutput, 'latest parallel output');
      await expect.poll(() => closeCountsAtTermination.length).toBe(workersPerBuild);
      expect(Atomics.load(state, 1)).toBe(workersPerBuild);
      expect(closeCountsAtTermination.every((closeCount) => closeCount >= workersPerBuild)).toBe(
        true,
      );
    } finally {
      releaseFirst();
      releaseSecond();
      await bundle.close().catch(() => {});
      terminateSpy.mockRestore();
    }
  },
);

test('bundle construction failures keep the previous parallel worker pool alive', async () => {
  const state = new Int32Array(new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT * 2));
  const parallelPlugin = defineParallelPlugin<{ state: Int32Array }>(
    path.join(import.meta.dirname, 'parallel-close-plugin.mjs'),
  );
  const bundle = await rolldown({
    input: './main.js',
    cwd: import.meta.dirname,
    plugins: [parallelPlugin({ state })],
  });

  try {
    await bundle.generate();
    const workerCount = Atomics.load(state, 0);
    expect(workerCount).toBeGreaterThan(0);

    await expect(bundle.generate({ file: '/' })).rejects.toThrow('does not contain a file name');
    expect(Atomics.load(state, 1)).toBe(0);

    await bundle.close();
    expect(Atomics.load(state, 1)).toBe(workerCount);
  } finally {
    await bundle.close().catch(() => {});
  }
});

test('close retries only parallel-plugin workers whose termination failed', async () => {
  const cleanupError = new Error('worker termination failed');
  const originalTerminate = Object.getOwnPropertyDescriptor(Worker.prototype, 'terminate')!
    .value as (this: Worker) => Promise<number>;
  const terminateCalls = new Map<Worker, number>();
  const failedWorkers = new WeakSet<Worker>();
  let injectedFailure = false;
  const terminateSpy = vi
    .spyOn(Worker.prototype, 'terminate')
    .mockImplementation(function (this: Worker) {
      terminateCalls.set(this, (terminateCalls.get(this) ?? 0) + 1);
      if (!injectedFailure) {
        injectedFailure = true;
        failedWorkers.add(this);
        return Promise.reject(cleanupError);
      }
      return Reflect.apply(originalTerminate, this, []);
    });

  const state = new Int32Array(new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT * 2));
  const parallelPlugin = defineParallelPlugin<{ state: Int32Array }>(
    path.join(import.meta.dirname, 'parallel-close-plugin.mjs'),
  );
  const bundle = await rolldown({
    input: './main.js',
    cwd: import.meta.dirname,
    plugins: [parallelPlugin({ state })],
  });

  try {
    await bundle.generate();
    await expect(bundle.close()).rejects.toBe(cleanupError);
    expect([...terminateCalls].filter(([worker]) => failedWorkers.has(worker))).toHaveLength(1);
    expect([...terminateCalls].filter(([worker]) => failedWorkers.has(worker))[0][1]).toBe(1);

    await expect(bundle.close()).resolves.toBeUndefined();
    for (const [worker, calls] of terminateCalls) {
      expect(calls).toBe(failedWorkers.has(worker) ? 2 : 1);
    }
  } finally {
    terminateSpy.mockRestore();
    await bundle.close().catch(() => {});
  }
});

test('build retries a transient parallel-plugin worker termination failure', async () => {
  const cleanupError = new Error('worker termination failed');
  const originalTerminate = Object.getOwnPropertyDescriptor(Worker.prototype, 'terminate')!
    .value as (this: Worker) => Promise<number>;
  const terminateCalls = new Map<Worker, number>();
  const failedWorkers = new WeakSet<Worker>();
  let injectedFailure = false;
  const terminateSpy = vi
    .spyOn(Worker.prototype, 'terminate')
    .mockImplementation(function (this: Worker) {
      terminateCalls.set(this, (terminateCalls.get(this) ?? 0) + 1);
      if (!injectedFailure) {
        injectedFailure = true;
        failedWorkers.add(this);
        return Promise.reject(cleanupError);
      }
      return Reflect.apply(originalTerminate, this, []);
    });

  const state = new Int32Array(new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT * 2));
  const parallelPlugin = defineParallelPlugin<{ state: Int32Array }>(
    path.join(import.meta.dirname, 'parallel-close-plugin.mjs'),
  );

  try {
    await expect(
      build({
        input: './main.js',
        cwd: import.meta.dirname,
        plugins: [parallelPlugin({ state })],
        write: false,
      }),
    ).resolves.toBeDefined();
    expect(terminateCalls.size).toBe(Atomics.load(state, 0));
    for (const [worker, calls] of terminateCalls) {
      expect(calls).toBe(failedWorkers.has(worker) ? 2 : 1);
    }
  } finally {
    terminateSpy.mockRestore();
  }
});

test('closeBundle hook is not called if closed directly', async () => {
  const task = async () => {
    const bundle = await rolldown({
      input: './main.js',
      cwd: import.meta.dirname,
      plugins: [
        {
          name: 'test',
          closeBundle() {
            this.error('close bundle error');
          },
        },
      ],
    });
    await bundle.close();
  };
  await expect(task()).resolves.not.toThrow();
});

test('output properties are enumerable and can be spread', async () => {
  const bundle = await rolldown({
    input: './main.js',
    cwd: import.meta.dirname,
  });
  const result = await bundle.generate({ format: 'esm' });

  // Test that fileName is enumerable
  expect(Object.keys(result.output[0])).toContain('fileName');

  // Test that spreading the output object preserves all properties including fileName
  const spread = { ...result.output[0] };
  expect(spread.fileName).toBeDefined();
  expect(spread.fileName).toBe(result.output[0].fileName);

  // Test the exact scenario from the issue
  const fileNames = result.output.map((o) => ({ ...o })).map((o) => o.fileName);
  expect(fileNames).toEqual(['main.js']);

  // Ensure other lazy properties are also enumerable
  expect(Object.keys(result.output[0])).toContain('code');
  expect(Object.keys(result.output[0])).toContain('exports');
});

test('plugins are accessible in buildStart hook', async () => {
  let pluginsInBuildStart: unknown;
  const pluginA = {
    name: 'plugin-a',
    buildStart({ plugins }: { plugins: unknown }) {
      pluginsInBuildStart = plugins;
    },
  };
  const pluginB = { name: 'plugin-b' };
  const pluginC = { name: 'plugin-c' };
  const bundle = await rolldown({
    input: './main.js',
    cwd: import.meta.dirname,
    plugins: [pluginA, pluginB],
  });
  await bundle.generate({ format: 'esm', plugins: [pluginC] });
  expect(Array.isArray(pluginsInBuildStart)).toBe(true);
  const names = (pluginsInBuildStart as Array<{ name: string }>).map((p) => p.name);
  expect(names).toContain('plugin-a');
  expect(names).toContain('plugin-b');
  expect(names).not.toContain('plugin-c');
});

function settleWithin<T>(promise: Promise<T>, operation: string): Promise<T> {
  const timeoutMs = 5_000;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      reject(new Error(`${operation} timed out after ${timeoutMs}ms`));
    }, timeoutMs);
  });
  return Promise.race([promise, timeout]).finally(() => {
    if (timer) clearTimeout(timer);
  });
}
