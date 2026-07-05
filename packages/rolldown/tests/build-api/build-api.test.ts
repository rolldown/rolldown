import path from 'node:path';
import { Worker } from 'node:worker_threads';
import { rolldown } from 'rolldown';
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
