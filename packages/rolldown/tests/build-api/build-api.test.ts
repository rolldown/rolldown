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

test('a repeated close() does not run closeBundle again', async () => {
  let closeBundleCalls = 0;
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
  await bundle.close();
  expect(closeBundleCalls).toBe(1);
  expect(bundle.closed).toBe(true);
});

test('concurrent close() calls run the native close once', async () => {
  let closeBundleCalls = 0;
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
  await Promise.all([bundle.close(), bundle.close()]);
  expect(closeBundleCalls).toBe(1);
  expect(bundle.closed).toBe(true);
});

test('a failed worker shutdown still closes the native bundler', async () => {
  let closeBundleCalls = 0;
  const parallelNoopPlugin = defineParallelPlugin<void>(
    path.join(import.meta.dirname, 'parallel-noop-plugin-impl.js'),
  );
  const bundle = await rolldown({
    input: './main.js',
    cwd: import.meta.dirname,
    plugins: [
      parallelNoopPlugin(),
      {
        name: 'test',
        closeBundle() {
          closeBundleCalls++;
        },
      },
    ],
  });
  await bundle.generate();
  // Stop the worker for real, then report a failure, so no thread outlives the test.
  const terminate: (this: Worker) => Promise<number> = Object.getOwnPropertyDescriptor(
    Worker.prototype,
    'terminate',
  )!.value;
  const terminateSpy = vi
    .spyOn(Worker.prototype, 'terminate')
    .mockImplementationOnce(async function (this: Worker) {
      await terminate.call(this);
      throw new Error('terminate failed');
    });
  try {
    await expect(bundle.close()).rejects.toThrow('terminate failed');
    expect(terminateSpy).toHaveBeenCalled();
    expect(bundle.closed).toBe(true);
    expect(closeBundleCalls).toBe(1);
    await expect(bundle.close()).resolves.toBeUndefined();
    expect(closeBundleCalls).toBe(1);
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
