import { type EmittedFile, type OutputOptions, type Plugin, rolldown } from 'rolldown';
import { expect, test } from 'vitest';
import { virtualPlugin } from '../src/inline-common-chunks/harness';

const modules: Record<string, string> = {
  '/main.js': 'export const load = () => import("/dep.js");',
  '/dep.js': 'export const value = 42;',
  '/extra.js': 'export const extra = 1;',
};

const asset = { type: 'asset', name: 'data.txt', source: 'data' } as const;

async function generate(options: OutputOptions, files: EmittedFile[] = [], plugins: Plugin[] = []) {
  const bundle = await rolldown({
    input: '/main.js',
    plugins: [
      {
        ...virtualPlugin(modules),
        buildStart() {
          for (const file of files) this.emitFile(file);
        },
      },
      ...plugins,
    ],
  });
  try {
    return (await bundle.generate(options)).output;
  } finally {
    await bundle.close();
  }
}

test('entryFileNames hash marks the entry chunk immutable', async () => {
  const [entry, dependency] = await generate({
    entryFileNames: '[name]-[hash].js',
    chunkFileNames: '[name].js',
  });
  expect(entry).toMatchObject({
    fileName: expect.stringMatching(/^main-.+\.js$/),
    immutable: true,
  });
  expect(dependency).toMatchObject({ fileName: 'dep.js', immutable: false });
});

test('chunkFileNames hash marks the dynamic chunk immutable', async () => {
  const [entry, dependency] = await generate({ chunkFileNames: '[name]-[hash].js' });
  expect(entry).toMatchObject({ fileName: 'main.js', immutable: false });
  expect(dependency).toMatchObject({
    fileName: expect.stringMatching(/^dep-.+\.js$/),
    immutable: true,
  });
});

test('assetFileNames hash marks the asset immutable', async () => {
  const outputs = await generate({ assetFileNames: '[name]-[hash][extname]' }, [asset]);
  expect(outputs.find((output) => output.type === 'asset')).toMatchObject({
    fileName: expect.stringMatching(/^data-.+\.txt$/),
    immutable: true,
  });
});

test('name-only templates set immutable=false', async () => {
  const outputs = await generate(
    {
      entryFileNames: '[name].js',
      chunkFileNames: '[name].js',
      assetFileNames: '[name][extname]',
    },
    [asset],
  );
  expect(outputs.map(({ fileName, immutable }) => [fileName, immutable])).toEqual([
    ['main.js', false],
    ['dep.js', false],
    ['data.txt', false],
  ]);
});

test('callbacks determine immutability from the returned filename template', async () => {
  const [entry, dependency, data] = await generate(
    {
      entryFileNames: () => '[name]-[hash].js',
      chunkFileNames: () => '[name].js',
      assetFileNames: () => '[name]-[hash][extname]',
    },
    [asset],
  );
  expect(entry.immutable).toBe(true);
  expect(dependency.immutable).toBe(false);
  expect(data.immutable).toBe(true);
});

test('a hash in the directory marks the output immutable', async () => {
  const [entry] = await generate({ entryFileNames: 'assets/[hash]/[name].js' });
  expect(entry).toMatchObject({
    fileName: expect.stringMatching(/^assets\/[^/]+\/main\.js$/),
    immutable: true,
  });
});

test('a deduplicated asset uses the template of the selected name', async () => {
  const outputs = await generate(
    {
      assetFileNames: ({ name }) =>
        name === 'a.txt' ? '[name][extname]' : '[name]-[hash][extname]',
    },
    [asset, { ...asset, name: 'a.txt' }],
  );
  const assets = outputs.filter((output) => output.type === 'asset');
  expect(assets).toHaveLength(1);
  expect(assets[0]).toMatchObject({ fileName: 'a.txt', immutable: false });
});

test('a numeric suffix preserves the immutable flag', async () => {
  const outputs = await generate({ assetFileNames: '[hash:1][extname]', hashCharacters: 'hex' }, [
    { ...asset, source: 'beta' },
    { ...asset, source: 'two' },
  ]);
  const assets = outputs.filter((output) => output.type === 'asset');
  expect(assets).toHaveLength(2);
  const [first, second] = assets;
  expect(first.fileName).toMatch(/\.txt$/);
  expect(second.fileName).toBe(first.fileName.replace(/\.txt$/, '2.txt'));
  expect(assets.map((output) => output.immutable)).toEqual([true, true]);
});

test.each<{ file: EmittedFile; immutable: boolean }>([
  { file: { ...asset, fileName: 'marked.txt', immutable: true }, immutable: true },
  { file: { ...asset, fileName: 'unmarked.txt' }, immutable: false },
  {
    file: { type: 'prebuilt-chunk', fileName: 'marked.js', code: '', immutable: true },
    immutable: true,
  },
  { file: { type: 'prebuilt-chunk', fileName: 'unmarked.js', code: '' }, immutable: false },
  { file: { type: 'chunk', fileName: 'extra.js', id: '/extra.js' }, immutable: false },
])('emitted $file.type $file.fileName has immutable=$immutable', async ({ file, immutable }) => {
  const outputs = await generate(
    { assetFileNames: '[hash][extname]', chunkFileNames: '[hash].js' },
    [file],
  );
  expect(outputs.find((output) => output.fileName === file.fileName)).toMatchObject({ immutable });
});

test.each(['asset', 'chunk'] as const)(
  'renaming an immutable %s clears the flag for this and later hooks',
  async (type) => {
    const renamed = 'renamed.txt';
    const outputs = await generate(
      { entryFileNames: '[name]-[hash].js', assetFileNames: '[name]-[hash][extname]' },
      [asset],
      [
        {
          name: 'rename',
          generateBundle(_options, bundle) {
            const output = Object.values(bundle).find((output) => output.type === type)!;
            const originalName = output.fileName;
            expect(output.immutable).toBe(true);
            output.fileName = renamed;
            expect(output.immutable).toBe(false);
            output.fileName = originalName;
            expect(output.immutable).toBe(true);
            output.fileName = renamed;
          },
        },
        {
          name: 'check-rename',
          generateBundle(_options, bundle) {
            expect(bundle[renamed].immutable).toBe(false);
          },
        },
      ],
    );
    expect(outputs.find((output) => output.fileName === renamed)?.immutable).toBe(false);
  },
);

test('content changes in generateBundle clear immutable', async () => {
  const build = (version: number) =>
    generate(
      {
        entryFileNames: '[name]-[hash].js',
        chunkFileNames: '[name]-[hash].js',
        assetFileNames: '[name]-[hash][extname]',
      },
      [
        asset,
        { ...asset, name: 'version.json', source: '{}' },
        { ...asset, name: 'same.txt', source: 'café' },
      ],
      [
        {
          name: 'edit',
          generateBundle(_options, bundle) {
            for (const output of Object.values(bundle)) {
              if (output.type === 'chunk') {
                output.code = output.isEntry
                  ? output.code + `\nexport const version = ${version};\n`
                  : output.code;
              } else if (output.names.includes('version.json')) {
                output.source = JSON.stringify({ version });
              } else if (output.names.includes('same.txt')) {
                output.source = new TextEncoder().encode('café');
              }
            }
          },
        },
      ],
    );
  const first = await build(1);
  const second = await build(2);
  expect(first.map(({ fileName }) => fileName).sort()).toEqual(
    second.map(({ fileName }) => fileName).sort(),
  );
  for (const [version, outputs] of [
    [1, first],
    [2, second],
  ] as const) {
    expect(outputs).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          type: 'chunk',
          isEntry: true,
          code: expect.stringContaining(`export const version = ${version};`),
          immutable: false,
        }),
        expect.objectContaining({ type: 'chunk', isEntry: false, immutable: true }),
        expect.objectContaining({
          type: 'asset',
          names: ['version.json'],
          source: JSON.stringify({ version }),
          immutable: false,
        }),
        expect.objectContaining({ type: 'asset', names: ['data.txt'], immutable: true }),
        expect.objectContaining({ type: 'asset', names: ['same.txt'], immutable: true }),
      ]),
    );
  }
});

test('copying an output with Object.assign ignores writes to immutable', async () => {
  const outputs = await generate(
    { entryFileNames: '[name]-[hash].js', chunkFileNames: '[name]-[hash].js' },
    [asset],
    [
      {
        name: 'copy-output',
        generateBundle(_options, bundle) {
          for (const output of Object.values(bundle)) {
            Object.assign(output, { immutable: false });
            expect(output.immutable).toBe(true);
            if (output.type === 'asset') {
              Object.assign(output, { ...output, source: 'copied' });
            } else {
              Object.assign(output, { ...output, code: output.code + '\n// copied' });
            }
          }
        },
      },
    ],
  );
  const editedChunk = {
    type: 'chunk',
    code: expect.stringContaining('// copied'),
    immutable: false,
  };
  expect(outputs).toMatchObject([
    editedChunk,
    editedChunk,
    { type: 'asset', source: 'copied', immutable: false },
  ]);
});

test.each([undefined, 'maps/[name]-[hash].map'])(
  'a generated sourcemap has immutable=false with sourcemapFileNames=%s',
  async (sourcemapFileNames) => {
    const outputs = await generate({
      sourcemap: true,
      entryFileNames: '[name]-[hash].js',
      sourcemapFileNames,
    });
    const maps = outputs.filter((output) => output.type === 'asset');
    expect(maps).toHaveLength(2);
    expect(maps.map((map) => map.immutable)).toEqual([false, false]);
  },
);
