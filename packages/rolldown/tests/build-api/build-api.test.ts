import path from 'node:path';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

test('rolldown write twice', async () => {
  const bundle = await rolldown({
    input: './main.js',
    cwd: import.meta.dirname,
  });
  const esmOutput = await bundle.write({
    format: 'esm',
    entryFileNames: 'main.mjs',
  });
  expect(await bundle.watchFiles).toStrictEqual([
    path.join(import.meta.dirname, 'main.js'),
  ]);
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

test('closeBundle hook is not called if closed directly', async () => {
  await expect(async () => {
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
  }).not.toThrow()
});