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

test.each([
  { type: 'chunk', fileName: 'app.js' },
  { type: 'asset', fileName: 'version.txt' },
] as const)(
  'copying a hashed $type with a new file name clears immutable',
  async ({ type, fileName }) => {
    const outputs = await generate(
      { entryFileNames: '[name]-[hash].js', assetFileNames: '[name]-[hash][extname]' },
      [asset],
      [
        {
          name: 'copy-rename',
          generateBundle(_options, bundle) {
            const file = Object.values(bundle).find((item) => item.type === type)!;
            expect(file.immutable).toBe(true);
            Object.assign(file, { ...file, fileName });
          },
        },
        {
          name: 'check-copy-rename',
          generateBundle(_options, bundle) {
            expect(bundle[fileName].immutable).toBe(false);
          },
        },
      ],
    );
    expect(outputs.find((output) => output.fileName === fileName)).toMatchObject({
      type,
      immutable: false,
    });
  },
);

test('content changes in generateBundle preserve immutable', async () => {
  const outputs = await generate(
    {
      entryFileNames: '[name]-[hash].js',
      chunkFileNames: '[name]-[hash].js',
      assetFileNames: '[name]-[hash][extname]',
    },
    [asset],
    [
      {
        name: 'edit',
        generateBundle(_options, bundle) {
          for (const output of Object.values(bundle)) {
            if (output.type === 'asset') {
              output.source = 'edited';
            } else {
              output.code += '\n// edited';
            }
          }
        },
      },
    ],
  );
  const editedChunk = {
    type: 'chunk',
    code: expect.stringContaining('// edited'),
    immutable: true,
  };
  expect(outputs).toMatchObject([
    editedChunk,
    editedChunk,
    { type: 'asset', source: 'edited', immutable: true },
  ]);
});

test('generateBundle can clear immutable for later hooks and the final output', async () => {
  const outputs = await generate(
    {
      entryFileNames: '[name]-[hash].js',
      chunkFileNames: '[name]-[hash].js',
      assetFileNames: '[name]-[hash][extname]',
    },
    [asset],
    [
      {
        name: 'clear-immutable',
        generateBundle(_options, bundle) {
          for (const output of Object.values(bundle)) {
            expect(output.immutable).toBe(true);
            output.immutable = false;
            expect(output.immutable).toBe(false);
          }
        },
      },
      {
        name: 'check-immutable',
        generateBundle(_options, bundle) {
          expect(Object.values(bundle).map((output) => output.immutable)).toEqual([
            false,
            false,
            false,
          ]);
        },
      },
    ],
  );
  expect(outputs.map((output) => output.immutable)).toEqual([false, false, false]);
});

test.each(['before', 'after'] as const)(
  'renaming %s assigning immutable applies the last change',
  async (order) => {
    const immutable = order === 'before';
    const outputs = await generate(
      {
        entryFileNames: '[name].js',
        chunkFileNames: '[name].js',
        assetFileNames: '[name][extname]',
      },
      [asset],
      [
        {
          name: 'rename-and-assign',
          generateBundle(_options, bundle) {
            for (const output of Object.values(bundle)) {
              const renamed = 'renamed-' + output.fileName;
              if (order === 'before') output.fileName = renamed;
              output.immutable = true;
              if (order === 'after') {
                expect(output.immutable).toBe(true);
                output.fileName = renamed;
              }
              expect(output.immutable).toBe(immutable);
            }
          },
        },
        {
          name: 'check-renamed-outputs',
          generateBundle(_options, bundle) {
            expect(Object.values(bundle).map((output) => output.immutable)).toEqual([
              immutable,
              immutable,
              immutable,
            ]);
          },
        },
      ],
    );
    expect(outputs.map((output) => output.immutable)).toEqual([immutable, immutable, immutable]);
  },
);

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
