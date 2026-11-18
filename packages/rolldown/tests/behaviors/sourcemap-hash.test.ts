import type { OutputOptions } from 'rolldown';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

async function build(output: OutputOptions, extraEntry = false) {
  const modules: Record<string, string> = {
    '/main.js': 'export const load = () => import("/dep.js");',
    '/dep.js': 'export const value = 42;',
    '/extra.js': 'console.log("unrelated");',
  };
  const bundle = await rolldown({
    input: extraEntry ? ['/extra.js', '/main.js'] : ['/main.js'],
    plugins: [
      {
        name: 'virtual',
        resolveId: (id) => (id in modules ? id : null),
        load: (id) => modules[id],
      },
    ],
  });
  try {
    const result = await bundle.generate({
      entryFileNames: '[name]-[hash].js',
      chunkFileNames: '[name]-[hash].js',
      ...output,
    });
    return new Map(
      result.output.filter((chunk) => chunk.type === 'chunk').map((chunk) => [chunk.name, chunk]),
    );
  } finally {
    await bundle.close();
  }
}

// https://github.com/rolldown/rolldown/issues/11508
test('sourcemap comments affect chunk hashes', async () => {
  const disabled = await build({ sourcemap: false });
  const hidden = await build({ sourcemap: 'hidden' });
  const external = await build({ sourcemap: true });
  const inline = await build({ sourcemap: 'inline' });
  for (const name of ['main', 'dep']) {
    expect(hidden.get(name)!.code).toBe(disabled.get(name)!.code);
    expect(hidden.get(name)!.fileName).toBe(disabled.get(name)!.fileName);
    expect(external.get(name)!.code).toContain('//# sourceMappingURL=');
    expect(inline.get(name)!.code).toContain('//# sourceMappingURL=data:');
    expect(
      new Set([disabled, external, inline].map((chunks) => chunks.get(name)!.fileName)).size,
    ).toBe(3);
  }
});

test.each<OutputOptions>([
  { sourcemap: true },
  { sourcemap: 'inline' },
  { sourcemap: true, sourcemapFileNames: '[name]-[hash]-[chunkhash].map' },
  { sourcemap: true, sourcemapBaseUrl: 'https://example.test/maps/' },
])('sourcemap hashes remain stable with unrelated entries: %j', async (options) => {
  const original = await build(options);
  const extended = await build(options, true);
  for (const name of ['main', 'dep']) {
    expect(extended.get(name)!.code).toBe(original.get(name)!.code);
    expect(extended.get(name)!.fileName).toBe(original.get(name)!.fileName);
  }
});

test.each<OutputOptions>([
  { sourcemapBaseUrl: 'https://example.test/maps/' },
  { sourcemapFileNames: '[name]-custom #.map' },
])('external sourcemap URLs affect chunk hashes: %j', async (options) => {
  const original = await build({ sourcemap: true });
  const changed = await build({ sourcemap: true, ...options });
  for (const name of ['main', 'dep']) {
    expect(changed.get(name)!.code).not.toBe(original.get(name)!.code);
    expect(changed.get(name)!.fileName).not.toBe(original.get(name)!.fileName);
  }
});

test.each<OutputOptions>([
  { sourcemap: 'inline' },
  { sourcemap: true, sourcemapFileNames: '[name]-[hash].map' },
])('referenced sourcemap content affects chunk hashes: %j', async (options) => {
  const original = await build(options);
  for (const change of [
    { sourcemapExcludeSources: true },
    { sourcemapPathTransform: (source: string) => `changed/${source}` },
    { sourcemapIgnoreList: () => true },
  ]) {
    const changed = await build({ ...options, ...change });
    for (const name of ['main', 'dep']) {
      expect(changed.get(name)!.code).not.toBe(original.get(name)!.code);
      expect(changed.get(name)!.fileName).not.toBe(original.get(name)!.fileName);
    }
  }
});

test.each([true, 'hidden'] as const)(
  'unreferenced sourcemap content preserves chunk hashes: %s',
  async (sourcemap) => {
    const original = await build({ sourcemap });
    const changed = await build({ sourcemap, sourcemapExcludeSources: true });
    for (const name of ['main', 'dep']) {
      expect(changed.get(name)!.code).toBe(original.get(name)!.code);
      expect(changed.get(name)!.fileName).toBe(original.get(name)!.fileName);
    }
  },
);

test('a changed dependency sourcemap invalidates its importer', async () => {
  const options: OutputOptions = {
    sourcemap: true,
    sourcemapFileNames: ({ name }) => (name === 'main' ? 'main.map' : '[name]-[hash].map'),
  };
  const original = await build(options);
  const changed = await build({
    ...options,
    sourcemapPathTransform: (source) =>
      /[/\\]dep\.js$/.test(source) ? `changed/${source}` : source,
  });
  expect(JSON.parse(changed.get('main')!.map!.toString())).toEqual({
    ...JSON.parse(original.get('main')!.map!.toString()),
    file: changed.get('main')!.fileName,
  });
  for (const name of ['main', 'dep']) {
    expect(changed.get(name)!.code).not.toBe(original.get(name)!.code);
    expect(changed.get(name)!.fileName).not.toBe(original.get(name)!.fileName);
  }
});

test('sourcemap callbacks run once and final maps contain final chunk names', async () => {
  const paths: string[] = [];
  const chunks = await build({
    sourcemap: 'inline',
    sourcemapPathTransform(source, mapPath) {
      paths.push(mapPath);
      return `${mapPath}/${source}`;
    },
  });
  expect(paths).toHaveLength(2);
  for (const chunk of chunks.values()) {
    const data = chunk.code.split('base64,')[1];
    const map = JSON.parse(Buffer.from(data, 'base64').toString());
    expect(map.file).toBe(chunk.fileName);
    expect(map.sources[0]).toContain(`${chunk.fileName}.map/`);
    expect(map.sources[0]).not.toContain('!~{');
  }
});

test.each([true, 'inline', 'hidden'] as const)(
  'debug ID comments affect chunk hashes: %s',
  async (sourcemap) => {
    const original = await build({ sourcemap });
    const changed = await build({ sourcemap, sourcemapDebugIds: true });
    for (const name of ['main', 'dep']) {
      expect(changed.get(name)!.code).toContain('//# debugId=');
      expect(changed.get(name)!.fileName).not.toBe(original.get(name)!.fileName);
    }
  },
);
