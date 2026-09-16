import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type { OutputChunk, OutputOptions, Plugin } from 'rolldown';
import { rolldown } from 'rolldown';
import { describe, expect, test } from 'vitest';

function virtualModules(modules: Record<string, string>): Plugin {
  return {
    name: 'virtual-modules',
    resolveId(id) {
      return id in modules ? id : undefined;
    },
    load(id) {
      return modules[id];
    },
  };
}

async function generate(
  modules: Record<string, string>,
  input: Record<string, string>,
  output: OutputOptions = {},
  plugins: Plugin[] = [],
) {
  const bundle = await rolldown({
    input,
    preserveEntrySignatures: false,
    plugins: [virtualModules(modules), ...plugins],
  });
  try {
    return await bundle.generate({
      format: 'es',
      entryFileNames: '[name].js',
      chunkFileNames: '[name]-[hash].js',
      ...output,
    });
  } finally {
    await bundle.close();
  }
}

function chunksOf(output: Awaited<ReturnType<typeof generate>>) {
  return output.output.filter((item): item is OutputChunk => item.type === 'chunk');
}

function sharedKeys(output: Awaited<ReturnType<typeof generate>>) {
  return chunksOf(output).flatMap((chunk) =>
    [...chunk.code.matchAll(/__rd_share\("(rd:[^"]+)"/g)].map((match) => match[1]),
  );
}

const twoEntryModules = {
  a: 'import { increment } from "shared"; increment();',
  b: 'import { increment } from "shared"; increment();',
  shared: 'let value = 0; export function increment() { return ++value }',
};

describe('experimentalInlineCommonChunks', () => {
  test('reports each physical module placement to renderChunk and generateBundle', async () => {
    const rendered = new Map<string, string[]>();
    const renderedExports = new Map<string, string[]>();
    const generated = new Map<string, string[]>();
    const filenameModules = new Map<string, string[]>();
    const output = await generate(
      twoEntryModules,
      { a: 'a', b: 'b' },
      {
        experimentalInlineCommonChunks: { maxSize: 1000 },
        entryFileNames(chunk) {
          filenameModules.set(chunk.name, [...chunk.moduleIds]);
          return '[name].js';
        },
      },
      [
        {
          name: 'observe-chunks',
          renderChunk(_code, chunk) {
            rendered.set(chunk.fileName, [...chunk.moduleIds]);
            renderedExports.set(chunk.name, [...chunk.exports]);
          },
          generateBundle(_options, bundle) {
            for (const item of Object.values(bundle)) {
              if (item.type === 'chunk') generated.set(item.fileName, [...item.moduleIds]);
            }
          },
        },
      ],
    );

    const entries = chunksOf(output).filter((chunk) => chunk.isEntry);
    expect(entries).toHaveLength(2);
    for (const entry of entries) {
      expect(entry.moduleIds).toContain('shared');
      expect(entry.modules).toHaveProperty('shared');
      expect(rendered.get(entry.fileName)).toContain('shared');
      expect(generated.get(entry.fileName)).toContain('shared');
      expect(filenameModules.get(entry.name)).toContain('shared');
    }
    expect(chunksOf(output).some((chunk) => chunk.name === 'shared')).toBe(false);

    const runtime = chunksOf(output).find((chunk) => chunk.code.includes('function __rd_share('));
    expect(runtime?.exports).toEqual(expect.arrayContaining(['__rd_share', '__rd_share_require']));
    expect(renderedExports.get(runtime!.name)).toEqual(
      expect.arrayContaining(['__rd_share', '__rd_share_require']),
    );
  });

  test('does not expose omitted logical chunks to filename hooks or reserve their names', async () => {
    const callbackNames: string[] = [];
    const output = await generate(
      twoEntryModules,
      { a: 'a', b: 'b' },
      {
        experimentalInlineCommonChunks: { maxSize: 1000 },
        chunkFileNames(chunk) {
          callbackNames.push(chunk.name);
          return 'chunk.js';
        },
      },
    );

    const secondaryChunks = chunksOf(output).filter((chunk) => !chunk.isEntry);
    expect(secondaryChunks).toHaveLength(1);
    expect(secondaryChunks[0].fileName).toBe('chunk.js');
    expect(callbackNames).toHaveLength(1);
    expect(callbackNames).not.toContain('shared');
  });

  test('keeps registry keys stable across content changes and distinct across chunk identities', async () => {
    const first = await generate(
      twoEntryModules,
      { a: 'a', b: 'b' },
      {
        experimentalInlineCommonChunks: { maxSize: 1000 },
      },
    );
    const changed = await generate(
      {
        ...twoEntryModules,
        shared: 'let value = 10; export function increment() { return ++value }',
      },
      { a: 'a', b: 'b' },
      { experimentalInlineCommonChunks: { maxSize: 1000 } },
    );
    expect(new Set(sharedKeys(first))).toEqual(new Set(sharedKeys(changed)));

    const distinct = await generate(
      {
        a: 'import { value } from "shared-one"; console.log(value)',
        b: 'import { value } from "shared-one"; console.log(value)',
        c: 'import { value } from "shared-two"; console.log(value)',
        d: 'import { value } from "shared-two"; console.log(value)',
        'shared-one': 'globalThis.one = true; export let value = 1',
        'shared-two': 'globalThis.two = true; export let value = 1',
      },
      { a: 'a', b: 'b', c: 'c', d: 'd' },
      { experimentalInlineCommonChunks: { maxSize: 1000 } },
    );
    expect(new Set(sharedKeys(distinct)).size).toBe(2);
  });

  test('keeps registry keys stable when the project root moves', async () => {
    const parent = await mkdtemp(join(tmpdir(), 'rolldown-inline-common-chunks-'));
    try {
      const roots = [join(parent, 'one'), join(parent, 'two')];
      for (const root of roots) {
        await mkdir(root);
        await Promise.all(
          Object.entries(twoEntryModules).map(([name, source]) =>
            writeFile(join(root, `${name}.js`), source.replace('"shared"', '"./shared.js"')),
          ),
        );
      }
      const build = async (root: string) => {
        const bundle = await rolldown({
          cwd: root,
          input: { a: join(root, 'a.js'), b: join(root, 'b.js') },
          preserveEntrySignatures: false,
        });
        try {
          return await bundle.generate({
            format: 'es',
            experimentalInlineCommonChunks: { maxSize: 1000 },
          });
        } finally {
          await bundle.close();
        }
      };

      expect(new Set(sharedKeys(await build(roots[0])))).toEqual(
        new Set(sharedKeys(await build(roots[1]))),
      );
    } finally {
      await rm(parent, { recursive: true, force: true });
    }
  });

  test('maxSize: 0 preserves the default output byte-for-byte', async () => {
    const defaults = await generate(twoEntryModules, { a: 'a', b: 'b' });
    const disabled = await generate(
      twoEntryModules,
      { a: 'a', b: 'b' },
      {
        experimentalInlineCommonChunks: { maxSize: 0 },
      },
    );
    const comparable = (output: Awaited<ReturnType<typeof generate>>) =>
      chunksOf(output)
        .map(({ fileName, code, imports, dynamicImports }) => ({
          fileName,
          code,
          imports,
          dynamicImports,
        }))
        .sort((left, right) => left.fileName.localeCompare(right.fileName));
    expect(comparable(disabled)).toEqual(comparable(defaults));
  });

  test.each([
    ['format', { format: 'cjs' } satisfies OutputOptions, 'output.format to be "es"'],
    [
      'code splitting',
      { codeSplitting: false } satisfies OutputOptions,
      'output.codeSplitting to be enabled',
    ],
    [
      'strict order opt-out',
      { strictExecutionOrder: false } satisfies OutputOptions,
      'output.strictExecutionOrder to be true or omitted',
    ],
    [
      'preserve modules',
      { preserveModules: true } satisfies OutputOptions,
      'output.preserveModules to be false',
    ],
  ])('rejects incompatible %s', async (_name, incompatible, message) => {
    await expect(
      generate(
        twoEntryModules,
        { a: 'a', b: 'b' },
        {
          experimentalInlineCommonChunks: { maxSize: 1000 },
          ...incompatible,
        },
      ),
    ).rejects.toThrow(message);
  });

  test('requires preserveEntrySignatures: false', async () => {
    const bundle = await rolldown({
      input: { a: 'a', b: 'b' },
      plugins: [virtualModules(twoEntryModules)],
    });
    try {
      await expect(
        bundle.generate({ experimentalInlineCommonChunks: { maxSize: 1000 } }),
      ).rejects.toThrow('preserveEntrySignatures to be false');
    } finally {
      await bundle.close();
    }
  });

  test('rejects on-demand execution wrapping', async () => {
    const bundle = await rolldown({
      input: { a: 'a', b: 'b' },
      preserveEntrySignatures: false,
      experimental: { onDemandWrapping: true },
      plugins: [virtualModules(twoEntryModules)],
    });
    try {
      await expect(
        bundle.generate({ experimentalInlineCommonChunks: { maxSize: 1000 } }),
      ).rejects.toThrow('experimental.onDemandWrapping to be false');
    } finally {
      await bundle.close();
    }
  });
});
