import type { OutputChunk, Plugin } from 'rolldown';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

test('warns when independent chunk mappings can corrupt a shared property', async () => {
  const modules: Record<string, string> = {
    'entry-a': `
      globalThis.crossChunkObject = { _shared: 42 };
      globalThis.keepA = [
        globalThis.crossChunkObject._shared,
        globalThis.crossChunkObject._shared,
        globalThis.crossChunkObject._shared,
        globalThis._onlyA,
      ];
    `,
    'entry-b': `
      globalThis.keepB = [globalThis._onlyB, globalThis._onlyB, globalThis._onlyB];
      globalThis.crossChunkValue = globalThis.crossChunkObject._shared;
    `,
  };
  const warnings: { code?: string; message: string }[] = [];
  const bundle = await rolldown({
    input: { a: 'entry-a', b: 'entry-b' },
    plugins: [
      {
        name: 'virtual',
        resolveId(id) {
          if (id in modules) return id;
        },
        load(id) {
          return modules[id];
        },
      },
    ],
    onwarn(warning) {
      warnings.push(warning);
    },
  });
  const output = await bundle.generate({
    minify: {
      compress: false,
      mangle: false,
      mangleProps: { include: '^_' },
      codegen: false,
    },
  });
  await bundle.close();

  expect(warnings).toHaveLength(1);
  expect(warnings[0]?.code).toBe('CROSS_CHUNK_PROPERTY_MANGLE');
  expect(warnings[0]?.message).toContain('"_shared"');

  const chunks = output.output.filter((item): item is OutputChunk => item.type === 'chunk');
  const chunkA = chunks.find((chunk) => chunk.name === 'a');
  const chunkB = chunks.find((chunk) => chunk.name === 'b');
  expect(chunkA).toBeDefined();
  expect(chunkB).toBeDefined();

  const testGlobal = globalThis as typeof globalThis & {
    crossChunkObject?: Record<string, number>;
    crossChunkValue?: number;
    keepA?: unknown;
    keepB?: unknown;
  };
  try {
    await import(`data:text/javascript,${encodeURIComponent(chunkA!.code)}`);
    await import(`data:text/javascript,${encodeURIComponent(chunkB!.code)}`);
    expect(testGlobal.crossChunkValue).toBeUndefined();
  } finally {
    delete testGlobal.crossChunkObject;
    delete testGlobal.crossChunkValue;
    delete testGlobal.keepA;
    delete testGlobal.keepB;
  }
});

test('uses an explicit cache for properties shared across output chunks', async () => {
  const modules: Record<string, string> = {
    'entry-a': 'globalThis.fromA = [input._shared, input._shared, input._shared, input._onlyA];',
    'entry-b': 'globalThis.fromB = [input._onlyB, input._onlyB, input._onlyB, input._shared];',
  };
  const virtualPlugin: Plugin = {
    name: 'virtual',
    resolveId(id) {
      if (id in modules) return id;
    },
    load(id) {
      return modules[id];
    },
  };

  const bundle = await rolldown({
    input: { a: 'entry-a', b: 'entry-b' },
    plugins: [virtualPlugin],
  });
  const output = await bundle.generate({
    minify: {
      compress: false,
      mangle: false,
      mangleProps: { include: '^_', cache: { _shared: 'shared' } },
      codegen: false,
    },
  });
  await bundle.close();

  const chunks = output.output.filter((item): item is OutputChunk => item.type === 'chunk');
  const chunkA = chunks.find((chunk) => chunk.name === 'a');
  const chunkB = chunks.find((chunk) => chunk.name === 'b');
  expect(chunkA).toBeDefined();
  expect(chunkB).toBeDefined();

  const propertiesA = [...chunkA!.code.matchAll(/input\.([A-Za-z_$][\w$]*)/g)].map(
    (match) => match[1],
  );
  const propertiesB = [...chunkB!.code.matchAll(/input\.([A-Za-z_$][\w$]*)/g)].map(
    (match) => match[1],
  );

  expect(propertiesA).toHaveLength(4);
  expect(propertiesB).toHaveLength(4);
  expect(propertiesA[0]).toBe('shared');
  expect(propertiesA[3]).not.toBe('_onlyA');
  expect(propertiesB[0]).not.toBe('_onlyB');
  expect(propertiesA[0]).toBe(propertiesB[3]);
});

test('mangles the JavaScript emitted by TypeScript transforms', async () => {
  const virtualPlugin: Plugin = {
    name: 'virtual',
    resolveId(id) {
      if (id === 'entry.ts') return id;
    },
    load(id) {
      if (id === 'entry.ts') {
        return 'enum State { _Ready = 7 } globalThis.rolldownManglePropsResult = State._Ready;';
      }
    },
  };
  const bundle = await rolldown({ input: 'entry.ts', plugins: [virtualPlugin] });
  const output = await bundle.generate({
    format: 'iife',
    minify: {
      compress: false,
      mangle: false,
      mangleProps: { include: '^_Ready$', quoted: true },
      codegen: false,
    },
  });
  await bundle.close();

  const chunk = output.output.find((item): item is OutputChunk => item.type === 'chunk');
  expect(chunk).toBeDefined();
  const testGlobal = globalThis as typeof globalThis & { rolldownManglePropsResult?: number };
  try {
    await import(`data:text/javascript,${encodeURIComponent(chunk!.code)}`);
    expect(testGlobal.rolldownManglePropsResult).toBe(7);
  } finally {
    delete testGlobal.rolldownManglePropsResult;
  }
});
