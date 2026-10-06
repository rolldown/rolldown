import type { ManglePropertiesOptions, OutputChunk, Plugin } from 'rolldown';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

const multipleChunksError =
  'property mangling is currently supported only when a build generates one JavaScript chunk';

function virtualPlugin(modules: Record<string, string>): Plugin {
  return {
    name: 'virtual',
    resolveId(id) {
      if (id in modules) return id;
    },
    load(id) {
      return modules[id];
    },
  };
}

async function generateMangled(code: string, mangleProps: ManglePropertiesOptions) {
  const bundle = await rolldown({ input: 'entry', plugins: [virtualPlugin({ entry: code })] });
  try {
    return await bundle.generate({
      minify: { compress: false, mangle: false, codegen: false, mangleProps },
    });
  } finally {
    await bundle.close();
  }
}

test('rejects property mangling for multiple output chunks', async () => {
  const modules = { 'entry-a': 'input._field;', 'entry-b': 'input._field;' };
  const bundle = await rolldown({
    input: { a: 'entry-a', b: 'entry-b' },
    plugins: [virtualPlugin(modules)],
  });
  try {
    await expect(
      bundle.generate({
        minify: {
          compress: false,
          mangle: false,
          mangleProps: { include: /^_/ },
          codegen: false,
        },
      }),
    ).rejects.toThrow(multipleChunksError);
  } finally {
    await bundle.close();
  }
});

test('rejects property mangling with an emitted prebuilt chunk', async () => {
  const modules = { entry: 'input._field;' };
  let renderedError: Error | undefined;
  const bundle = await rolldown({
    input: 'entry',
    plugins: [
      virtualPlugin(modules),
      {
        name: 'emit-prebuilt-chunk',
        generateBundle() {
          this.emitFile({
            type: 'prebuilt-chunk',
            fileName: 'prebuilt.js',
            code: 'input._field;',
          });
        },
        renderError(error) {
          renderedError = error;
        },
      },
    ],
  });
  try {
    await expect(
      bundle.generate({
        minify: {
          compress: false,
          mangle: false,
          mangleProps: { include: /^_/ },
          codegen: false,
        },
      }),
    ).rejects.toThrow(multipleChunksError);
    expect(renderedError?.message).toContain(multipleChunksError);
  } finally {
    await bundle.close();
  }
});

test('mangles selected properties and honors reserved and cached names', async () => {
  const modules = {
    'entry.js':
      'class State { _READY = 7; _reserved = 5; _cached = 3 } const state = new State(); globalThis.rolldownManglePropsResult = state._READY + state._reserved + state._cached;',
  };
  const bundle = await rolldown({ input: 'entry.js', plugins: [virtualPlugin(modules)] });
  const output = await bundle.generate({
    format: 'iife',
    minify: {
      compress: false,
      mangle: false,
      mangleProps: {
        include: /^_(?:ready|reserved|cached)$/i,
        reserved: ['_reserved'],
        cache: { _cached: 'cached' },
      },
      codegen: false,
    },
  });
  await bundle.close();

  const chunk = output.output.find((item): item is OutputChunk => item.type === 'chunk');
  expect(chunk).toBeDefined();
  expect(chunk!.code).not.toContain('_READY');
  expect(chunk!.code).toContain('_reserved');
  expect(chunk!.code).toContain('.cached');
  const testGlobal = globalThis as typeof globalThis & { rolldownManglePropsResult?: number };
  try {
    await import(`data:text/javascript,${encodeURIComponent(chunk!.code)}`);
    expect(testGlobal.rolldownManglePropsResult).toBe(15);
  } finally {
    delete testGlobal.rolldownManglePropsResult;
  }
});

test('returns a complete reusable property mangle cache', async () => {
  const inputCache = { e: false, ignored: 'kept' } as const;
  const first = await generateMangled('input.foo;', {
    include: /^foo$/,
    debug: true,
    cache: inputCache,
  });
  expect(inputCache).toEqual({ e: false, ignored: 'kept' });
  expect(first.mangleCache).toEqual({ e: false, foo: '_$foo$_', ignored: 'kept' });

  const second = await generateMangled('input.foo;', {
    include: /^foo$/,
    cache: first.mangleCache,
  });
  expect(second.mangleCache).toEqual(first.mangleCache);
  const chunk = second.output.find((item): item is OutputChunk => item.type === 'chunk');
  expect(chunk!.code).toContain('input._$foo$_');
});

test('distinguishes disabled property mangling from an empty cache', async () => {
  const bundle = await rolldown({
    input: 'entry',
    plugins: [virtualPlugin({ entry: 'input.foo;' })],
  });
  try {
    const output = await bundle.generate({ minify: false });
    expect(output.mangleCache).toBeUndefined();
    expect('mangleCache' in output).toBe(false);
  } finally {
    await bundle.close();
  }

  const output = await generateMangled('input.foo;', { include: /^_/ });
  expect(output.mangleCache).toEqual({});
  expect('mangleCache' in output).toBe(true);
});
