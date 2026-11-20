import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { rolldown } from 'rolldown';
import type { InputOptions, OutputOptions, RolldownLog } from 'rolldown';
import { afterAll, describe, expect, test } from 'vitest';

// A build that `experimental.importDefer` does not support lowers each deferred import to a normal
// import. Its output must be the output of the build with the option off, and its warnings must
// differ only in the reason.

const LOG_JS = 'export const log = [];\n';
const X_JS = "import { log } from './log.js';\nlog.push('x');\nexport const value = 1;\n";
const MAIN_JS = "import defer * as ns from './x.js';\nexport const read = () => ns.value;\n";

interface Case {
  name: string;
  files: Record<string, string>;
  input?: InputOptions['input'];
  inputOptions?: InputOptions;
  output?: OutputOptions;
  // A text of the warning when the build falls back. `undefined` when the build keeps the deferral.
  reason?: string;
}

const cases: Case[] = [
  {
    name: 'iife output',
    files: { 'main.js': MAIN_JS },
    output: { format: 'iife', name: 'lib' },
    reason: 'the output format is not `esm` or `cjs`',
  },
  {
    name: 'umd output',
    files: { 'main.js': MAIN_JS },
    output: { format: 'umd', name: 'lib' },
    reason: 'the output format is not `esm` or `cjs`',
  },
  {
    name: 'two entries',
    files: { 'main.js': MAIN_JS, 'other.js': 'export const other = 1;\n' },
    input: ['main.js', 'other.js'],
    reason: 'the build has more than one entry',
  },
  {
    name: 'two entries of one module',
    files: { 'main.js': MAIN_JS },
    input: { a: 'main.js', b: 'main.js' },
    reason: 'the build has more than one entry',
  },
  {
    name: 'an emitted chunk',
    files: { 'main.js': MAIN_JS, 'other.js': 'export const other = 1;\n' },
    inputOptions: {
      plugins: [
        {
          name: 'emit-chunk',
          buildStart() {
            this.emitFile({ type: 'chunk', id: './other.js' });
          },
        },
      ],
    },
    reason: 'the build has more than one entry',
  },
  {
    name: 'preserveModules',
    files: { 'main.js': MAIN_JS },
    output: { preserveModules: true },
    reason: '`output.preserveModules` is on',
  },
  {
    name: 'code splitting groups',
    files: { 'main.js': MAIN_JS },
    output: { codeSplitting: { groups: [{ name: 'shared', test: /log\.js/ }] } },
    reason: 'the build has code splitting groups',
  },
  {
    name: 'manualChunks',
    files: { 'main.js': MAIN_JS },
    output: { manualChunks: (id) => (id.endsWith('log.js') ? 'shared' : undefined) },
    reason: 'the build has code splitting groups',
  },
  {
    name: 'inline common chunks',
    files: { 'main.js': MAIN_JS },
    inputOptions: { preserveEntrySignatures: false },
    output: { codeSplitting: { experimentalInlineCommonChunks: { maxSize: 4096 } } },
    reason: '`output.codeSplitting.experimentalInlineCommonChunks` is on',
  },
  {
    name: 'import()',
    files: {
      'main.js': MAIN_JS + "export const load = () => import('./other.js');\n",
      'other.js': 'export const other = 1;\n',
    },
    reason: 'the build has an `import()`',
  },
  {
    name: 'import.defer()',
    files: {
      'main.js': MAIN_JS + "export const load = () => import.defer('./other.js');\n",
      'other.js': 'export const other = 1;\n',
    },
    reason: 'the build has an `import()`',
  },
  {
    name: 'top-level await',
    files: { 'main.js': MAIN_JS + 'await 0;\n' },
    reason: 'a module of the build has a top-level `await`',
  },
  {
    name: 'import cycle',
    files: { 'main.js': MAIN_JS, 'x.js': "import './main.js';\n" + X_JS },
    reason: 'the build has a static import cycle',
  },
  {
    name: 'external target',
    files: { 'main.js': "import defer * as ns from 'ext';\nexport const read = () => ns.value;\n" },
    inputOptions: { external: ['ext'] },
    reason: 'a deferred import evaluates the external module `ext`',
  },
  {
    name: 'external module in the closure',
    files: { 'main.js': MAIN_JS, 'x.js': "import 'ext';\n" + X_JS },
    inputOptions: { external: ['ext'] },
    reason: 'a deferred import evaluates the external module `ext`',
  },
  {
    name: 'CommonJS target',
    files: {
      'main.js': "import defer * as ns from './x.cjs';\nexport const read = () => ns.value;\n",
      'x.cjs': 'module.exports = { value: 1 };\n',
    },
    reason: 'a deferred import evaluates the CommonJS module `x.cjs`',
  },
  {
    name: 'JSON target',
    files: {
      'main.js': "import defer * as ns from './data.json';\nexport const read = () => ns.value;\n",
      'data.json': '{ "value": 1 }\n',
    },
    reason: 'a deferred import evaluates `data.json`, which is not a JavaScript module',
  },
  {
    name: 'a closure module that require() loads',
    files: {
      'main.js': MAIN_JS + "export const required = () => require('./x.js');\n",
    },
    output: { format: 'cjs' },
    reason: 'a deferred import evaluates `x.js`, which a `require()` call also loads',
  },
  {
    name: 'a closure module with require()',
    files: {
      'main.js': MAIN_JS,
      'x.js': X_JS + "export const y = () => require('./y.js');\n",
      'y.js': 'export const y = 1;\n',
    },
    output: { format: 'cjs' },
    reason:
      'a deferred import evaluates `x.js`, which has a `require()` call or another import that is not an `import` statement',
  },
  {
    name: 'a read statement',
    files: { 'main.js': "import defer * as ns from './x.js';\nns.value;\nawait 0;\n" },
    reason: 'a module of the build has a top-level `await`',
  },
];

const roots: string[] = [];
afterAll(() => {
  for (const root of roots) {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

function writeFiles(files: Record<string, string>): string {
  // The real path: the module ids are real paths, and the names in the output are relative to
  // `cwd`.
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'rolldown-import-defer-')));
  roots.push(root);
  const all = { 'log.js': LOG_JS, 'x.js': X_JS, ...files };
  for (const [name, content] of Object.entries(all)) {
    fs.writeFileSync(path.join(root, name), content);
  }
  return root;
}

interface BuildResult {
  output: { fileName: string; content: string }[];
  warnings: RolldownLog[];
  buildEndError: string | undefined;
  error: string | undefined;
}

async function build(root: string, c: Case, importDefer: boolean): Promise<BuildResult> {
  const warnings: RolldownLog[] = [];
  let buildEndError: string | undefined;
  let error: string | undefined;
  let output: BuildResult['output'] = [];
  const bundle = await rolldown({
    input: c.input ?? 'main.js',
    cwd: root,
    ...c.inputOptions,
    plugins: [
      c.inputOptions?.plugins,
      {
        name: 'record-build-end',
        buildEnd(err) {
          buildEndError = err?.message;
        },
      },
    ],
    experimental: { importDefer },
    onLog(level, log) {
      if (level === 'warn') {
        warnings.push(log);
      }
    },
  });
  try {
    const result = await bundle.generate(c.output);
    output = result.output.map((item) => ({
      fileName: item.fileName,
      content: item.type === 'chunk' ? item.code : String(item.source),
    }));
  } catch (e) {
    error = (e as Error).message;
  } finally {
    await bundle.close();
  }
  return { output, warnings, buildEndError, error };
}

// The parts of the warnings that must be the same with the option on and off. A fallback adds its
// warnings after the scan, so the order can differ.
const positions = (logs: RolldownLog[]) =>
  logs
    .map((log) => ({ code: log.code, id: log.id, loc: log.loc, pos: log.pos }))
    .sort((a, b) => `${a.id}:${a.pos}:${a.code}`.localeCompare(`${b.id}:${b.pos}:${b.code}`));

describe('import defer fallback', () => {
  test.each(cases)('$name', async (c) => {
    const root = writeFiles(c.files);
    const off = await build(root, c, false);
    const on = await build(root, c, true);
    expect(off.error).toBeUndefined();
    expect(on.error).toBeUndefined();
    expect(on.output).toStrictEqual(off.output);
    expect(positions(on.warnings)).toStrictEqual(positions(off.warnings));
    const deferWarnings = on.warnings.filter((log) => log.code === 'UNSUPPORTED_FEATURE');
    expect(deferWarnings.some((log) => log.message.includes(c.reason!))).toBe(true);
  });

  test('a write with the option off and in a fallback', async () => {
    const c: Case = {
      name: 'write',
      files: { 'main.js': "import defer * as ns from './x.js';\nns.value = 2;\nawait 0;\n" },
    };
    const root = writeFiles(c.files);
    const off = await build(root, c, false);
    const on = await build(root, c, true);
    expect(off.error).toContain('ASSIGN_TO_IMPORT');
    expect(on.error).toStrictEqual(off.error);
    expect(on.buildEndError).toStrictEqual(off.buildEndError);
    expect(on.buildEndError).toContain('ASSIGN_TO_IMPORT');
  });

  test.each([
    { name: 'esm output', output: { format: 'esm' } as OutputOptions },
    { name: 'cjs output', output: { format: 'cjs' } as OutputOptions },
  ])('a build with one chunk keeps the deferral: $name', async ({ output }) => {
    const c: Case = { name: 'supported', files: { 'main.js': MAIN_JS }, output };
    const root = writeFiles(c.files);
    const off = await build(root, c, false);
    const on = await build(root, c, true);
    expect(on.warnings).toStrictEqual([]);
    expect(off.warnings.map((log) => log.code)).toStrictEqual(['UNSUPPORTED_FEATURE']);
    expect(on.output).not.toStrictEqual(off.output);
    expect(on.output[0].content).toContain('(init_x(), 1)');
  });
});
