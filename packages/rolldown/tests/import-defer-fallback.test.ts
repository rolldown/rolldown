import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import vm from 'node:vm';
import { rolldown } from 'rolldown';
import type { InputOptions, OutputOptions, RolldownLog } from 'rolldown';
import { afterAll, describe, expect, test } from 'vitest';

// A build that `experimental.importDefer` does not support lowers each deferred import to a normal
// import. Its output must be the output of the build with the option off, and its warnings must
// differ only in the reason.

const LOG_JS = 'export const log = [];\n';
const X_JS = "import { log } from './log.js';\nlog.push('x');\nexport const value = 1;\n";
const MAIN_JS = "import defer * as ns from './x.js';\nexport const read = () => ns.value;\n";
// The closure of `x.js` has a module with top-level await. An import after the deferred import
// makes the build fall back.
const AFTER_ASYNC_DEFER = {
  'x.js': "import './t.js';\n" + X_JS,
  't.js': 'await 0;\n',
  'y.js': "import { log } from './log.js';\nlog.push('y');\n",
};
const AFTER_ASYNC_DEFER_REASON =
  'an import in `main.js` comes after a deferred import of a module that has a top-level `await` or imports one';
const CJS_MAIN_JS = "import defer * as ns from './x.cjs';\nexport const read = () => ns.value;\n";
const CJS_X = 'exports.value = 1;\n';
const CJS_RUN_AT_LOAD_REASON = (id: string) =>
  `a deferred import evaluates the CommonJS module \`${id}\`, and the output file of that module runs it when the file loads`;
const EXTERNAL_MAIN_JS = "import defer * as ns from 'ext';\nexport const read = () => ns.value;\n";
const EXTERNAL_REASON = (format: 'iife' | 'umd') =>
  `a deferred import evaluates the external module \`ext\`, and \`${format}\` output gets that module as a global or as a wrapper argument`;
const EXTERNAL_STAR_X_JS = "export * from 'ext';\n" + X_JS;
const EXTERNAL_STAR_REASON =
  'a deferred import evaluates the external module `ext` through `export *`, and the `cjs` output file of an entry module loads the external modules of its `export *` when the file loads';

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
    name: 'inline common chunks',
    files: { 'main.js': MAIN_JS },
    inputOptions: { preserveEntrySignatures: false },
    output: { codeSplitting: { experimentalInlineCommonChunks: { maxSize: 4096 } } },
    reason: '`output.codeSplitting.experimentalInlineCommonChunks` is on',
  },
  {
    name: 'a code splitting group with includeDependenciesRecursively: false',
    files: { 'main.js': MAIN_JS },
    inputOptions: { preserveEntrySignatures: 'allow-extension' },
    output: {
      codeSplitting: {
        groups: [{ name: 'x', test: /[\\/]x\.js$/, includeDependenciesRecursively: false }],
      },
    },
    reason: 'a code splitting group sets `includeDependenciesRecursively: false`',
  },
  ...[
    ['anonymous default function', 'export default function () { return ns.value; }'],
    ['re-exported namespace', 'export { ns as default };'],
  ].map(([name, declaration]) => ({
    name: `preserved importer cycle (${name})`,
    files: {
      'main.js': `import defer * as ns from './x.js';\nimport './reader.js';\n${declaration}\n`,
      'reader.js': "import value from './main.js';\nconsole.log(value);\n",
    },
    output: { preserveModules: true },
    reason: 'a static import cycle can access the deferred namespace in `main.js`',
  })),
  {
    name: 'an import after a deferred import of a module with top-level await',
    files: { 'main.js': MAIN_JS + "import './y.js';\n", ...AFTER_ASYNC_DEFER },
    reason: AFTER_ASYNC_DEFER_REASON,
  },
  {
    // With more than one chunk, the file of `main.js` imports `ext` before its body prepares the
    // deferred import.
    name: 'an import of an external module after a deferred import of a module with top-level await',
    files: { 'main.js': MAIN_JS + "import 'ext';\n", ...AFTER_ASYNC_DEFER },
    inputOptions: { external: ['ext'] },
    output: { preserveModules: true },
    reason: AFTER_ASYNC_DEFER_REASON,
  },
  {
    name: 'external target, iife output',
    files: { 'main.js': EXTERNAL_MAIN_JS },
    inputOptions: { external: ['ext'] },
    output: { format: 'iife', name: 'lib', globals: { ext: 'Ext' } },
    reason: EXTERNAL_REASON('iife'),
  },
  {
    name: 'external target, umd output',
    files: { 'main.js': EXTERNAL_MAIN_JS },
    inputOptions: { external: ['ext'] },
    output: { format: 'umd', name: 'lib', globals: { ext: 'Ext' } },
    reason: EXTERNAL_REASON('umd'),
  },
  {
    // The output keeps a native `import()` of the external module.
    name: 'external target of import.defer(), iife output',
    files: { 'main.js': "export const load = () => import.defer('ext');\n" },
    inputOptions: { external: ['ext'] },
    output: { format: 'iife', name: 'lib' },
    reason: EXTERNAL_REASON('iife'),
  },
  {
    name: 'import.defer() of a CommonJS module',
    files: {
      'main.js': MAIN_JS + "export const load = () => import.defer('./c.cjs');\n",
      'c.cjs': CJS_X,
    },
    reason: 'an `import.defer()` call targets the CommonJS module `c.cjs`',
  },
  {
    // With code splitting off, the `import()` makes `c.js` a CommonJS module, though `c.js` has no
    // module syntax.
    name: 'import.defer() of a module with no module syntax, with code splitting off',
    files: {
      'main.js': MAIN_JS + "export const load = () => import.defer('./c.js');\n",
      'c.js': 'globalThis.c = 1;\n',
    },
    output: { codeSplitting: false },
    reason: 'an `import.defer()` call targets the CommonJS module `c.js`',
  },
  {
    name: 'external module in the closure',
    files: { 'main.js': MAIN_JS, 'x.js': "import 'ext';\n" + X_JS },
    inputOptions: { external: ['ext'] },
    reason:
      'a deferred import evaluates the external module `ext`, and `esm` output imports that module at the start of the chunk',
  },
  {
    name: 'external module in the closure, umd output',
    files: { 'main.js': MAIN_JS, 'x.js': "import 'ext';\n" + X_JS },
    inputOptions: { external: ['ext'] },
    output: { format: 'umd', name: 'lib', globals: { ext: 'Ext' } },
    reason: EXTERNAL_REASON('umd'),
  },
  {
    // The file of `x.js` loads `ext` for its `export *` when the file loads.
    name: 'export * of an external module in a deferred closure, cjs output with preserveModules',
    files: { 'main.js': MAIN_JS, 'x.js': EXTERNAL_STAR_X_JS },
    inputOptions: { external: ['ext'] },
    output: { format: 'cjs', preserveModules: true },
    reason: EXTERNAL_STAR_REASON,
  },
  {
    // The chunk of the target `x.js` loads `ext` for its `export *` when the chunk loads.
    name: 'export * of an external module in the target of import.defer(), cjs output',
    files: {
      'main.js': "export const load = () => import.defer('./x.js');\n",
      'x.js': EXTERNAL_STAR_X_JS,
    },
    inputOptions: { external: ['ext'] },
    output: { format: 'cjs' },
    reason: EXTERNAL_STAR_REASON,
  },
  {
    // The file of `x.cjs` ends with `export default require_x();`.
    name: 'a CommonJS module in a deferred closure, with preserveModules',
    files: { 'main.js': CJS_MAIN_JS, 'x.cjs': CJS_X },
    output: { preserveModules: true },
    reason: CJS_RUN_AT_LOAD_REASON('x.cjs'),
  },
  {
    // The `require()` makes `c.js` a CommonJS module, though `c.js` has no module syntax.
    name: 'a required module in a deferred closure, with preserveModules',
    files: {
      'main.js': MAIN_JS,
      'x.js': "require('./c.js');\n" + X_JS,
      'c.js': 'globalThis.c = 1;\n',
    },
    output: { preserveModules: true },
    reason: CJS_RUN_AT_LOAD_REASON('c.js'),
  },
  {
    // The chunk of the entry `x.cjs` ends with `module.exports = require_x();`.
    name: 'a CommonJS user entry in a deferred closure, with cjs output',
    files: { 'main.js': CJS_MAIN_JS, 'x.cjs': CJS_X },
    input: ['main.js', 'x.cjs'],
    output: { format: 'cjs' },
    reason: CJS_RUN_AT_LOAD_REASON('x.cjs'),
  },
  {
    name: 'a read statement',
    files: {
      'main.js': "import defer * as ns from './x.js';\nimport './y.js';\nns.value;\n",
      ...AFTER_ASYNC_DEFER,
    },
    reason: AFTER_ASYNC_DEFER_REASON,
  },
];

// A build with one of these shapes keeps the deferral. Each case calls exports of the entry, and
// gives the value and the log after each call. The deferred modules run at the first read, not at
// the load of the entry.
const LOGGED_MAIN = (target: string, extra = '') =>
  `import defer * as ns from '${target}';\nexport { log } from './log.js';\nexport const read = () => ns.value;\n${extra}`;

interface KeptCase {
  name: string;
  files: Record<string, string>;
  input?: InputOptions['input'];
  inputOptions?: InputOptions;
  output?: OutputOptions;
  // A text of each warning of the build.
  warnings?: string[];
  steps: [name: string, value: unknown, log: string[]][];
}

const OTHER_JS = 'export const other = 1;\n';

const keptCases: KeptCase[] = [
  {
    name: 'CommonJS target',
    files: {
      'main.js': LOGGED_MAIN('./x.cjs'),
      'x.cjs': "require('./log.js').log.push('x');\nexports.value = 1;\n",
    },
    steps: [['read', 1, ['x']]],
  },
  {
    name: 'CommonJS module in the closure',
    files: {
      'main.js': LOGGED_MAIN('./x.js'),
      'x.js': "import './c.cjs';\n" + X_JS,
      'c.cjs': "require('./log.js').log.push('c');\n",
    },
    steps: [['read', 1, ['c', 'x']]],
  },
  {
    name: 'JSON target',
    files: { 'main.js': LOGGED_MAIN('./data.json'), 'data.json': '{ "value": 1 }\n' },
    steps: [['read', 1, []]],
  },
  {
    name: 'text target',
    files: {
      'main.js':
        "import defer * as ns from './a.txt';\nexport { log } from './log.js';\nexport const read = () => ns.default;\n",
      'a.txt': 'text',
    },
    steps: [['read', 'text', []]],
  },
  {
    name: 'a closure module that require() loads',
    files: {
      'main.js': LOGGED_MAIN('./x.js', "export const required = () => require('./x.js').value;\n"),
    },
    steps: [
      ['required', 1, ['x']],
      ['read', 1, ['x']],
    ],
  },
  {
    name: 'a closure module with require()',
    files: {
      'main.js': LOGGED_MAIN('./x.js', 'export const readY = () => ns.y();\n'),
      'x.js': X_JS + "export const y = () => require('./y.js').y;\n",
      'y.js': "import { log } from './log.js';\nlog.push('y');\nexport const y = 2;\n",
    },
    steps: [
      ['read', 1, ['x']],
      ['readY', 2, ['x', 'y']],
    ],
  },
  {
    // The early evaluation of a builtin module is not observable.
    name: 'a closure module that imports a builtin module',
    files: {
      'main.js': LOGGED_MAIN('./x.js'),
      'x.js': "import { sep } from 'node:path';\n" + X_JS + 'export const s = sep;\n',
    },
    inputOptions: { external: ['node:path'] },
    steps: [['read', 1, ['x']]],
  },
  {
    name: 'a cycle through the deferred import',
    files: { 'main.js': LOGGED_MAIN('./x.js'), 'x.js': "import './main.js';\n" + X_JS },
    steps: [['read', 1, ['x']]],
  },
  {
    name: 'a cycle that only the deferred import reaches',
    files: {
      'main.js': LOGGED_MAIN('./x.js'),
      'x.js': "import './y.js';\n" + X_JS,
      'y.js': "import { log } from './log.js';\nimport './x.js';\nlog.push('y');\n",
    },
    steps: [['read', 1, ['y', 'x']]],
  },
  {
    name: 'two entries',
    files: { 'main.js': LOGGED_MAIN('./x.js'), 'other.js': OTHER_JS },
    input: ['main.js', 'other.js'],
    steps: [['read', 1, ['x']]],
  },
  {
    name: 'two entries of one module',
    files: { 'main.js': LOGGED_MAIN('./x.js') },
    input: { a: 'main.js', b: 'main.js' },
    steps: [['read', 1, ['x']]],
  },
  {
    name: 'an emitted chunk',
    files: { 'main.js': LOGGED_MAIN('./x.js'), 'other.js': OTHER_JS },
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
    steps: [['read', 1, ['x']]],
  },
  {
    name: 'preserved importer with a self-import',
    files: {
      'main.js':
        "import './main.js';\nimport defer * as ns from './x.js';\nexport { log } from './log.js';\nexport function read() { return ns.value; }\n",
    },
    output: { preserveModules: true },
    steps: [['read', 1, ['x']]],
  },
  {
    name: 'preserved importer in a cycle with only a lexical export',
    files: {
      'main.js': "import './reader.js';\n" + LOGGED_MAIN('./x.js'),
      'reader.js': "import './main.js';\n",
    },
    output: { preserveModules: true },
    steps: [['read', 1, ['x']]],
  },
  {
    name: 'preserveModules',
    files: { 'main.js': LOGGED_MAIN('./x.js') },
    output: { preserveModules: true },
    steps: [['read', 1, ['x']]],
  },
  {
    name: 'code splitting groups',
    files: { 'main.js': LOGGED_MAIN('./x.js') },
    output: { codeSplitting: { groups: [{ name: 'shared', test: /log\.js/ }] } },
    steps: [['read', 1, ['x']]],
  },
  {
    name: 'manualChunks',
    files: { 'main.js': LOGGED_MAIN('./x.js') },
    output: { manualChunks: (id) => (id.endsWith('log.js') ? 'shared' : undefined) },
    steps: [['read', 1, ['x']]],
  },
  {
    name: 'import()',
    files: {
      'main.js': LOGGED_MAIN(
        './x.js',
        "export const load = async () => (await import('./other.js')).other;\n",
      ),
      'other.js': OTHER_JS,
    },
    steps: [
      ['load', 1, []],
      ['read', 1, ['x']],
    ],
  },
  {
    name: 'import() of a deferred module',
    files: {
      'main.js': LOGGED_MAIN(
        './x.js',
        "export const load = async () => (await import('./x.js')).value;\n",
      ),
    },
    // The deferred import is a static import, so `x.js` stays in the chunk of `main.js`.
    warnings: ['x.js is dynamically imported by main.js but also statically imported by main.js'],
    steps: [
      ['load', 1, ['x']],
      ['read', 1, ['x']],
    ],
  },
  {
    name: 'import.defer()',
    files: {
      'main.js': LOGGED_MAIN(
        './x.js',
        "let other;\nexport const load = async () => ((other = await import.defer('./other.js')), 0);\nexport const readOther = () => other.other;\n",
      ),
      'other.js': "import { log } from './log.js';\nlog.push('other');\n" + OTHER_JS,
    },
    steps: [
      ['load', 0, []],
      ['readOther', 1, ['other']],
      ['read', 1, ['other', 'x']],
    ],
  },
  {
    name: 'new URL() in a closure module',
    files: {
      'main.js': LOGGED_MAIN('./x.js', 'export const readUrl = () => ns.url;\n'),
      'x.js':
        X_JS +
        "export const url = new URL('./a.svg', import.meta.url).pathname.split('/').pop();\n",
      'a.svg': '<svg xmlns="http://www.w3.org/2000/svg"/>\n',
    },
    steps: [['readUrl', 'a.svg', ['x']]],
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

// Runs the entry in a new Node.js process, and calls the exports of `steps` in this order.
function run(dir: string, entry: string, steps: string[]): unknown {
  const url = pathToFileURL(path.join(dir, entry)).href;
  const script = `
    const ns = await import(${JSON.stringify(url)});
    const m = ${JSON.stringify(entry)}.endsWith('.cjs') ? ns.default : ns;
    const result = { before: [...m.log], steps: [] };
    for (const name of ${JSON.stringify(steps)}) result.steps.push([await m[name](), [...m.log]]);
    console.log(JSON.stringify(result));
  `;
  return JSON.parse(
    execFileSync(process.execPath, ['--input-type=module', '-e', script], { encoding: 'utf8' }),
  );
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
      files: {
        'main.js': "import defer * as ns from './x.js';\nimport './y.js';\nns.value = 2;\n",
        ...AFTER_ASYNC_DEFER,
      },
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

  // IIFE and UMD output make one chunk. The output sets the global `lib`.
  test.each(['iife', 'umd'] as const)('a build keeps the deferral: %s output', async (format) => {
    const c: Case = {
      name: format,
      files: { 'main.js': LOGGED_MAIN('./x.js') },
      output: { format, name: 'lib' },
    };
    const root = writeFiles(c.files);
    const on = await build(root, c, true);
    expect(on.error).toBeUndefined();
    expect(on.warnings).toStrictEqual([]);
    const context: { lib?: { log: string[]; read: () => unknown } } = {};
    vm.runInNewContext(on.output[0].content, context);
    const lib = context.lib!;
    expect([...lib.log]).toStrictEqual([]);
    expect(lib.read()).toBe(1);
    expect([...lib.log]).toStrictEqual(['x']);
  });

  test.each(
    keptCases.flatMap((c) => (['esm', 'cjs'] as const).map((format) => ({ ...c, format }))),
  )('a build keeps the deferral: $name, $format output', async (c) => {
    const root = writeFiles(c.files);
    const ext = c.format === 'esm' ? 'mjs' : 'cjs';
    const output: OutputOptions = {
      ...c.output,
      format: c.format,
      entryFileNames: `[name].${ext}`,
      chunkFileNames: `[name].${ext}`,
    };
    const on = await build(root, { ...c, output }, true);
    expect(on.error).toBeUndefined();
    expect(on.warnings.map((log) => log.message)).toStrictEqual(
      (c.warnings ?? []).map((text) => expect.stringContaining(text)),
    );
    const dir = path.join(root, 'out');
    for (const file of on.output) {
      fs.mkdirSync(path.dirname(path.join(dir, file.fileName)), { recursive: true });
      fs.writeFileSync(path.join(dir, file.fileName), file.content);
    }
    expect(
      run(
        dir,
        on.output[0].fileName,
        c.steps.map(([name]) => name),
      ),
    ).toStrictEqual({ before: [], steps: c.steps.map(([, value, log]) => [value, log]) });
  });
});
