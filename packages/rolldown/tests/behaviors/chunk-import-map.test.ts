import { execFileSync } from 'node:child_process';
import { join, resolve } from 'node:path';
import type { OutputChunk, OutputOptions, Plugin, RolldownOutput } from 'rolldown';
import { rolldown } from 'rolldown';
import { describe, expect, onTestFinished, test } from 'vitest';
import { virtualPlugin } from '../src/inline-common-chunks/harness';
import { getOutputAsset, getOutputChunk } from '../src/utils';

const documentUrl = 'https://example.com/';
const input = { e1: './e1.js', e2: './e2.js', e3: './e3.js' };
const expectedLogs = {
  e1: ['e1 A:tinyLR C:tinyLR only-C'],
  e2: ['e2 A:tinyLR B:tinyLR'],
  e3: ['e3 B:tinyLR C:tinyLR'],
};

function fixture({ codeEdit = false, reverseMembers = false, copiedMember = false } = {}) {
  const modules: Record<string, string> = {
    './e1.js': `import { fromA } from './a/shared.js'; import { fromC, onlyInC } from './c/shared.js'; globalThis.log('e1 ' + fromA() + ' ' + fromC() + ' ' + onlyInC());`,
    './e2.js': `import { fromA } from './a/shared.js'; import { fromB } from './b/shared.js'; globalThis.log('e2 ' + fromA() + ' ' + fromB());`,
    './e3.js': `import { fromB } from './b/shared.js'; import { fromC } from './c/shared.js'; globalThis.log('e3 ' + fromB() + ' ' + fromC());`,
    './0/tiny.js': copiedMember
      ? `import { suffix } from './0/extra.js'; export const tiny = () => 'tiny' + suffix;`
      : `export const tiny = () => 'tiny';`,
    './0/extra.js': `export const suffix = String(globalThis.suffix ?? '!');`,
  };
  for (const letter of ['a', 'b', 'c']) {
    const imports = [
      `import { left } from './${letter}/left.js';`,
      `import { right } from './${letter}/right.js';`,
    ];
    if (reverseMembers) imports.reverse();
    const label = letter === 'a' && codeEdit ? 'A2' : letter.toUpperCase();
    modules[`./${letter}/shared.js`] = `
      import { tiny } from './0/tiny.js';
      ${imports.join('\n')}
      export const from${letter.toUpperCase()} = () => '${label}:' + tiny() + left + right;
      ${letter === 'c' ? `export const onlyInC = () => 'only-C';` : ''}
    `;
    modules[`./${letter}/left.js`] = `export const left = String(globalThis.left ?? 'L');`;
    modules[`./${letter}/right.js`] = `export const right = String(globalThis.right ?? 'R');`;
  }
  return modules;
}

function options(inline: boolean, extra: OutputOptions = {}): OutputOptions {
  return {
    format: 'esm',
    strictExecutionOrder: true,
    entryFileNames: '[name].js',
    chunkFileNames: 'shared-[hash].js',
    codeSplitting: inline
      ? { experimentalInlineCommonChunks: { maxSize: Infinity, exclude: /\/[abc]\// } }
      : {},
    ...extra,
  };
}

async function build(inline: boolean, modules = fixture(), outputOptions: OutputOptions = {}) {
  const bundle = await rolldown({
    input,
    preserveEntrySignatures: false,
    experimental: { chunkImportMap: true },
    plugins: [virtualPlugin(modules)],
  });
  try {
    return await bundle.generate(options(inline, outputOptions));
  } finally {
    await bundle.close();
  }
}

function importMap(output: RolldownOutput): Record<string, string> {
  const asset = getOutputAsset(output).find((asset) => asset.fileName === 'importmap.json')!;
  return JSON.parse(String(asset.source)).imports;
}

function alias(output: RolldownOutput, chunk: OutputChunk): string {
  const target = new URL(chunk.fileName, documentUrl).href;
  const match = Object.entries(importMap(output)).find(
    ([, filename]) => new URL(filename, documentUrl).href === target,
  );
  expect(match, chunk.fileName).toBeDefined();
  return match![0];
}

function carrierAliases(output: RolldownOutput) {
  return ['a', 'b', 'c'].map((letter) => {
    const chunk = getOutputChunk(output).find((chunk) =>
      chunk.moduleIds.includes(`./${letter}/shared.js`),
    )!;
    return alias(output, chunk);
  });
}

function execute(output: RolldownOutput, entryName: string): string[] {
  const chunks = getOutputChunk(output);
  const entry = chunks.find((chunk) => chunk.isEntry && chunk.name === entryName)!;
  const result = execFileSync(
    process.execPath,
    [
      '--experimental-vm-modules',
      '--input-type=module',
      '-e',
      `
        import { readFileSync } from 'node:fs';
        import { createContext, SourceTextModule } from 'node:vm';
        const { chunks, imports, entry } = JSON.parse(readFileSync(0, 'utf8'));
        const documentUrl = ${JSON.stringify(documentUrl)};
        const logs = [];
        const context = createContext({ log: (value) => logs.push(value) });
        const mappings = new Map(Object.entries(imports).map(([key, value]) => [
          new URL(key, documentUrl).href, new URL(value, documentUrl).href,
        ]));
        const modules = new Map(chunks.map(chunk => {
          const identifier = new URL(chunk.fileName, documentUrl).href;
          return [identifier, new SourceTextModule(chunk.code, { context, identifier })];
        }));
        const root = modules.get(new URL(entry, documentUrl).href);
        await root.link((specifier, parent) => {
          const address = new URL(specifier, parent.identifier).href;
          const target = mappings.get(address) ?? address;
          if (!modules.has(target)) throw new Error('Missing import-map target: ' + target);
          return modules.get(target);
        });
        await root.evaluate();
        process.stdout.write(JSON.stringify(logs));
      `,
    ],
    {
      input: JSON.stringify({ chunks, imports: importMap(output), entry: entry.fileName }),
      encoding: 'utf8',
      stdio: ['pipe', 'pipe', 'pipe'],
    },
  );
  return JSON.parse(result);
}

describe('chunkImportMap', () => {
  test.each([false, true].flatMap((inline) => [false, true].map((minify) => ({ inline, minify }))))(
    'same-named common chunks resolve to their own code: %j',
    async ({ inline, minify }) => {
      const output = await build(inline, fixture(), { minify });
      const chunks = getOutputChunk(output);
      const imports = importMap(output);
      expect(Object.keys(imports)).toHaveLength(chunks.filter((chunk) => !chunk.isEntry).length);
      const keys = Object.keys(imports).map((key) => new URL(key, documentUrl).href);
      expect(new Set(keys).size).toBe(keys.length);
      expect(new Set(carrierAliases(output)).size).toBe(3);
      for (const [name, logs] of Object.entries(expectedLogs)) {
        expect(execute(output, name)).toEqual(logs);
      }
      for (const letter of ['a', 'b', 'c']) {
        const chunk = chunks.find((chunk) => chunk.moduleIds.includes(`./${letter}/shared.js`))!;
        expect(chunk.moduleIds.includes('./0/tiny.js')).toBe(inline);
        expect('./0/tiny.js' in chunk.modules).toBe(inline);
      }
    },
  );

  test.each([false, true])(
    'logical-member aliases survive unrelated edits, inline=%s',
    async (inline) => {
      const baseline = carrierAliases(await build(inline));
      for (const variant of [
        { codeEdit: true },
        { reverseMembers: true },
        { copiedMember: true },
      ]) {
        expect(
          carrierAliases(await build(inline, fixture(variant))),
          JSON.stringify(variant),
        ).toEqual(baseline);
      }
    },
  );

  test('entry aliases use facade identity in both inlining modes', async () => {
    const aliases = [];
    for (const inline of [false, true]) {
      const output = await build(inline, fixture(), { entryFileNames: '[name]-[hash].js' });
      aliases.push(
        Object.keys(input).map((name) =>
          alias(
            output,
            getOutputChunk(output).find((chunk) => chunk.isEntry && chunk.name === name)!,
          ),
        ),
      );
      for (const [name, logs] of Object.entries(expectedLogs)) {
        expect(execute(output, name)).toEqual(logs);
      }
    }
    expect(aliases[1]).toEqual(aliases[0]);
  });
  test('aliases do not depend on the folder the build runs from', async () => {
    const build = async (cwd: string) => {
      const files = Object.fromEntries(
        Object.entries(fixture()).map(([id, code]) => [join(cwd, id), code]),
      );
      const plugin: Plugin = {
        name: 'folder',
        resolveId: (id) => (join(cwd, id) in files ? join(cwd, id) : undefined),
        load: (id) => files[id],
      };
      const bundle = await rolldown({
        input,
        cwd,
        preserveEntrySignatures: false,
        experimental: { chunkImportMap: true },
        plugins: [plugin],
      });
      onTestFinished(() => bundle.close());
      return bundle.generate(options(false, { entryFileNames: '[name]-[hash].js' }));
    };
    const fileNames = (output: RolldownOutput) =>
      getOutputChunk(output)
        .map((chunk) => chunk.fileName)
        .sort();
    const a = await build(resolve('/project-a'));
    const b = await build(resolve('/nested/project-b'));
    expect(Object.keys(importMap(a))).toHaveLength(getOutputChunk(a).length);
    expect(importMap(b)).toEqual(importMap(a));
    expect(fileNames(b)).toEqual(fileNames(a));
  });

  test.each(['entry', 'common'])(
    'path and non-path module IDs have distinct %s aliases',
    async (kind) => {
      const cwd = resolve('/project');
      const members = ['a/shared.js', 'b/shared.js'].flatMap((id) => [id, join(cwd, id)]);
      const entries = members.map((_, index) => join(cwd, `e${index}.js`));
      const files: Record<string, string> = {};
      for (const [index, id] of members.entries()) {
        files[id] = `export const value = () => '${index}';`;
        files[entries[index]] = `
          import { value as left } from ${JSON.stringify(id)};
          import { value as right } from ${JSON.stringify(members[(index + 1) % members.length])};
          globalThis.log(left() + right());
        `;
      }
      const bundle = await rolldown({
        cwd,
        input: kind === 'entry' ? [...entries, ...members] : entries,
        preserveEntrySignatures: false,
        experimental: { chunkImportMap: true },
        plugins: [virtualPlugin(files)],
      });
      onTestFinished(() => bundle.close());
      const output = await bundle.generate(options(false, { entryFileNames: '[name]-[hash].js' }));
      expect(Object.keys(importMap(output))).toHaveLength(getOutputChunk(output).length);
      for (let index = 0; index < members.length; index++) {
        expect(execute(output, `e${index}`)).toEqual([`${index}${(index + 1) % members.length}`]);
      }
    },
  );
});

describe('chunkImportMap.include', () => {
  type Include = string | RegExp | (string | RegExp)[];

  // main -> import(editor) -> common -> vendor, and main -> utils
  function modules({ vendor = 'V', common = 'C', commonImportsUtils = false } = {}) {
    return {
      './main.js': `import { utils } from './utils.js'; globalThis.open = () => import('./editor.js'); globalThis.utils = utils;`,
      './editor.js': `import { common } from './common.js'; export const editor = () => common() + globalThis.editor;`,
      './common.js': `
        import { vendor } from './vendor.js';
        ${commonImportsUtils ? `import { utils } from './utils.js';` : `const utils = () => '';`}
        export const common = () => vendor() + utils() + globalThis.common + '${common}';
      `,
      './vendor.js': `export const vendor = () => globalThis.vendor + '${vendor}';`,
      './utils.js': `export const utils = () => String(globalThis.utils);`,
      './first.js': `globalThis.first = true;`,
    };
  }

  async function build(
    files: Record<string, string>,
    include: Include | false | undefined,
    plugins: Plugin[] = [],
    input: Record<string, string> = { main: './main.js' },
  ) {
    const bundle = await rolldown({
      input,
      preserveEntrySignatures: false,
      experimental: { chunkImportMap: include === false ? false : { include } },
      plugins: [virtualPlugin(files), ...plugins],
    });
    onTestFinished(() => bundle.close());
    return bundle.generate({
      entryFileNames: '[name]-[hash].js',
      chunkFileNames: '[name]-[hash].js',
      codeSplitting: {
        includeDependenciesRecursively: false,
        groups: ['common', 'vendor', 'utils'].map((name) => ({ name, test: new RegExp(name) })),
      },
    });
  }

  const chunk = (output: RolldownOutput, name: string) =>
    getOutputChunk(output).find((chunk) => chunk.name === name)!;

  const renamed = (a: RolldownOutput, b: RolldownOutput) =>
    getOutputChunk(a)
      .filter(
        (chunk) =>
          chunk.fileName !== getOutputChunk(b).find((c) => c.name === chunk.name)!.fileName,
      )
      .map((chunk) => chunk.name)
      .sort();

  test.each<[string, Include]>([
    ['a string', 'common'],
    ['a regex', /^comm/],
    ['an array', ['missing', /^common$/]],
  ])('maps only the chunks matched by %s', async (_, include) => {
    const output = await build(modules(), include);
    const [[key, target], ...rest] = Object.entries(importMap(output));
    expect(rest).toEqual([]);
    expect(target).toBe(`/${chunk(output, 'common').fileName}`);
    expect(chunk(output, 'editor').code).toContain(`"./${key.slice(1)}"`);
    expect(chunk(output, 'common').code).toContain(`"./${chunk(output, 'vendor').fileName}"`);
    expect(chunk(output, 'main').code).toContain(`"./${chunk(output, 'editor').fileName}"`);
  });

  test('a chunk is renamed only when a filename in it can change', async () => {
    const base = await build(modules(), 'common');
    expect(renamed(base, await build(modules({ common: 'C2' }), 'common'))).toEqual(['common']);
    // `main` imports `editor` dynamically, which loads `vendor` through the mapped `common`.
    // `editor` only holds the stable name of `common`.
    expect(renamed(base, await build(modules({ vendor: 'V2' }), 'common'))).toEqual([
      'common',
      'main',
      'vendor',
    ]);
    // An unrelated chunk shifts the hash placeholders.
    const input = { first: './first.js', main: './main.js' };
    expect(renamed(base, await build(modules(), 'common', [], input))).toEqual([]);

    const full = await build(modules(), undefined);
    expect(renamed(full, await build(modules({ vendor: 'V2' }), undefined))).toEqual(['vendor']);
  });

  // Vite writes the files a dynamic import loads into its importer in `generateBundle`, after
  // hashing. `main` reaches `vendor` only through the mapped `common`.
  test('a filename written after hashing is covered by the hash', async () => {
    const preload: Plugin = {
      name: 'preload',
      generateBundle(_options, bundle) {
        const importMap = bundle['importmap.json'];
        const keys = Object.fromEntries(
          Object.entries<string>(
            importMap.type === 'asset' ? JSON.parse(String(importMap.source)).imports : {},
          ).map(([key, file]) => [file.slice(1), key.slice(1)]),
        );
        const closure = (file: string, files = new Set<string>()) => {
          const dep = bundle[file];
          if (dep?.type === 'chunk' && !files.has(file)) {
            files.add(file);
            dep.imports.forEach((file) => closure(file, files));
          }
          return [...files];
        };
        for (const chunk of Object.values(bundle)) {
          if (chunk.type !== 'chunk' || !chunk.dynamicImports.length) continue;
          const lists = chunk.dynamicImports.map((target) =>
            closure(target).map((file) => keys[file] ?? file),
          );
          chunk.code += `\n// preload ${JSON.stringify(lists)}`;
        }
      },
    };
    const base = await build(modules(), 'common', [preload]);
    expect(chunk(base, 'main').code).toContain(chunk(base, 'vendor').fileName);
    for (const edit of [{ vendor: 'V2' }, { commonImportsUtils: true }]) {
      const edited = await build(modules(edit), 'common', [preload]);
      expect(renamed(base, edited), JSON.stringify(edit)).toContain('main');
      const codes = new Map(getOutputChunk(base).map((chunk) => [chunk.fileName, chunk.code]));
      for (const chunk of getOutputChunk(edited)) {
        if (codes.has(chunk.fileName)) expect(chunk.code).toBe(codes.get(chunk.fileName));
      }
    }
  });

  test.each([
    ['no import map', false as const],
    ['a full import map', undefined],
    ['a partial import map', 'common'],
  ])('with %s `augmentChunkHash` is hashed as is', async (_, include) => {
    const declare: Plugin = {
      name: 'declare',
      augmentChunkHash: (chunk) => (chunk.name === 'first' ? chunk.fileName : undefined),
    };
    const input = { first: './first.js', main: './main.js' };
    const [plain, declared, shifted] = await Promise.all([
      build(modules(), include, [], input),
      build(modules(), include, [declare], input),
      build(modules(), include, [declare], { main: './main.js', first: './first.js' }),
    ]);
    expect(chunk(declared, 'first').fileName).not.toBe(chunk(plain, 'first').fileName);
    // The raw placeholder is part of the hash, so its index matters. This matches Rollup.
    expect(chunk(shifted, 'first').fileName).not.toBe(chunk(declared, 'first').fileName);
  });
});
