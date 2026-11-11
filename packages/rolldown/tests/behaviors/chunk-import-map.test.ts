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
