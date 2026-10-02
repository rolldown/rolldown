import { getDevWatchOptionsForCi } from '@rolldown/test-dev-server';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

import { isSingleThread } from '@tests/runtime-flavor';
import { RUNTIME_MODULE_ID } from 'rolldown';
import { dev } from 'rolldown/experimental';
import { expect, test, vi } from 'vitest';

async function createRuntime() {
  const runtimeUrl = import.meta.resolve('rolldown/experimental/runtime');
  const { DevRuntime, MissingFactoryError } = await import(runtimeUrl);
  return { runtime: new DevRuntime('test-client') as any, MissingFactoryError };
}

test('the standalone runtime uses the canonical runtime helpers', async () => {
  const { runtime } = await createRuntime();
  const commonJsModule = { value: 1 };

  expect(runtime.__toESM(commonJsModule).default).toBe(commonJsModule);
  const esmModule = runtime.__toCommonJS({ default: commonJsModule });
  expect(esmModule.__esModule).toBe(true);
  expect(esmModule.default).toBe(commonJsModule);
  expect(runtime.__exportAll({ value: () => 1 }).value).toBe(1);
  const reExportTarget: Record<string, unknown> = {};
  runtime.__reExport(reExportTarget, { value: 2 });
  expect(reExportTarget.value).toBe(2);
});

test('the runtime entry exports only the runtime classes', async () => {
  const runtimeModule = await import(import.meta.resolve('rolldown/experimental/runtime'));
  expect(Object.keys(runtimeModule).sort()).toStrictEqual(['DevRuntime', 'MissingFactoryError']);
});

// Vite serves this file to the browser as is.
test('the runtime entry is a single file with no imports', () => {
  const entryUrl = new URL(import.meta.resolve('rolldown/experimental/runtime'));
  expect(entryUrl.pathname.endsWith('/experimental-runtime.mjs')).toBe(true);

  const source = fs.readFileSync(entryUrl, 'utf8');
  expect(source).not.toMatch(/^\s*import\b/m);
  expect(source).not.toMatch(/\bfrom\s*['"]/);
});

test('the package emits no other runtime source file', () => {
  const distDir = new URL('.', import.meta.resolve('rolldown/experimental/runtime'));
  const runtimeFiles = fs
    .readdirSync(distDir)
    .filter((name) => name.includes('runtime') && name.endsWith('.mjs'));

  expect(runtimeFiles).toStrictEqual(['experimental-runtime.mjs']);
});

// The HMR plugin's `transform` hook appends the runtime before oxc prints it again, so the
// exact source is only visible to a later `transform` hook, not in the output.
// `dev()` needs a MultiThread runtime; the single-thread (CurrentThread) flavor rejects it.
test.skipIf(isSingleThread)(
  'devMode injects the common runtime and the default client as their exact source',
  {
    timeout: 60_000,
  },
  async ({ onTestFinished }) => {
    const dir = path.join(
      import.meta.dirname,
      'temp',
      `default-runtime-${crypto.randomUUID().slice(0, 8)}`,
    );
    fs.mkdirSync(dir, { recursive: true });
    const input = path.join(dir, 'main.js');
    fs.writeFileSync(input, 'console.log(1);\n');

    let runtimeModuleCode: string | undefined;
    const engine = await dev(
      {
        input,
        experimental: { devMode: { host: 'example.test', port: 1234 } },
        plugins: [
          {
            name: 'capture-runtime-module',
            transform(code, id) {
              if (id === RUNTIME_MODULE_ID) runtimeModuleCode = code;
            },
          },
        ],
      },
      { dir: path.join(dir, 'dist') },
      { watch: getDevWatchOptionsForCi() },
    );
    onTestFinished(async () => {
      await engine.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });
    engine.run().catch(() => {});
    await engine.ensureCurrentBuildFinish();

    const read = (file: string) =>
      fs.readFileSync(
        new URL(`../../../../crates/rolldown_plugin_hmr/src/runtime/${file}`, import.meta.url),
        'utf8',
      );
    const expected = `${read('runtime-extra-dev-common.js')}\n${read(
      'runtime-extra-dev-default.js',
    )}`.replaceAll('$ADDR', 'example.test:1234');
    expect(runtimeModuleCode?.endsWith(expected)).toBe(true);
    // `skipCommonRuntimeInjection` stops Rust from adding its own copy.
    expect(runtimeModuleCode?.split('class DevRuntime')).toHaveLength(2);
  },
);

test('registerGraph maintains static + dynamic reverse indexes; getImporters unions them', async () => {
  const { runtime } = await createRuntime();

  // app → foo (static edge), app ⇢ lazy (dynamic import() edge)
  runtime.registerGraph({
    ids: ['app.js', 'foo.js', 'lazy.js'],
    localCount: 3,
    edges: [[1], [], []],
    dynamicEdges: { 0: [2] },
  });
  expect(runtime.getImporters('foo.js')).toEqual(['app.js']);
  // the dynamic importer is returned too — the client-side dynamic-import HMR feature
  expect(runtime.getImporters('lazy.js')).toEqual(['app.js']);
  expect(runtime.getImporters('app.js')).toEqual([]);

  // re-carrying app: last-write-wins drops the old foo/lazy edges; a target imported
  // both statically and dynamically by app appears once (the union is deduped)
  runtime.registerGraph({
    ids: ['app.js', 'both.js'],
    localCount: 2,
    edges: [[1], []],
    dynamicEdges: { 0: [1] },
  });
  expect(runtime.getImporters('foo.js')).toEqual([]);
  expect(runtime.getImporters('lazy.js')).toEqual([]);
  expect(runtime.getImporters('both.js')).toEqual(['app.js']);

  // a re-carried row with no `dynamicEdges` entry has no dynamic edges
  runtime.registerGraph({
    ids: ['app.js'],
    localCount: 1,
    edges: [[]],
  });
  expect(runtime.getImporters('both.js')).toEqual([]);
});

test('registerGraph keeps the export names each static edge imports', async () => {
  const { runtime } = await createRuntime();

  // app → named (imports `a`, `default`), app → effect (side-effect only), app ⇢ lazy
  runtime.registerGraph({
    ids: ['app.js', 'named.js', 'effect.js', 'lazy.js'],
    localCount: 4,
    edges: [[1, 2], [], [], []],
    bindings: { 0: [['a', 'default'], []] },
    dynamicEdges: { 0: [3] },
  });
  expect(runtime.getImportedBindings('app.js', 'named.js')).toEqual(['a', 'default']);
  expect(runtime.getImportedBindings('app.js', 'effect.js')).toEqual([]);
  // a dynamic import() reads the whole namespace
  expect(runtime.getImportedBindings('app.js', 'lazy.js')).toEqual(['*']);
  expect(runtime.getImportedBindings('app.js', 'missing.js')).toBeUndefined();

  // a static edge and a dynamic import() to the same module: the import() reads everything
  runtime.registerGraph({
    ids: ['both.js', 'dep.js'],
    localCount: 2,
    edges: [[1], []],
    bindings: { 0: [['a']] },
    dynamicEdges: { 0: [1] },
  });
  expect(runtime.getImportedBindings('both.js', 'dep.js')).toEqual(['a', '*']);
  runtime.registerGraph({
    ids: ['both.js', 'dep.js'],
    localCount: 2,
    edges: [[1], []],
    bindings: { 0: [['*', 'a']] },
    dynamicEdges: { 0: [1] },
  });
  expect(runtime.getImportedBindings('both.js', 'dep.js')).toEqual(['*', 'a']);

  // re-carrying a module replaces its names (last write wins)
  runtime.registerGraph({
    ids: ['app.js', 'named.js'],
    localCount: 2,
    edges: [[1], []],
    bindings: { 0: [['b']] },
  });
  expect(runtime.getImportedBindings('app.js', 'named.js')).toEqual(['b']);

  // `null` and missing trailing entries mean "imports everything", and so does a missing row
  runtime.registerGraph({
    ids: ['sparse.js', 'x.js', 'y.js', 'z.js', 'no-row.js'],
    localCount: 5,
    edges: [[1, 2, 3], [], [], [], [1]],
    bindings: { 0: [null, ['a']] },
  });
  expect(runtime.getImportedBindings('sparse.js', 'x.js')).toEqual(['*']);
  expect(runtime.getImportedBindings('sparse.js', 'y.js')).toEqual(['a']);
  expect(runtime.getImportedBindings('sparse.js', 'z.js')).toEqual(['*']);
  expect(runtime.getImportedBindings('no-row.js', 'x.js')).toEqual(['*']);

  // a payload without `bindings` (an older compiler) is read as "imports everything"
  runtime.registerGraph({
    ids: ['old.js', 'dep.js'],
    localCount: 2,
    edges: [[1], []],
  });
  expect(runtime.getImportedBindings('old.js', 'dep.js')).toEqual(['*']);
});

test('initModule is registry-gated and returns the live exports', async () => {
  const { runtime } = await createRuntime();
  const factory = vi.fn((id: string) => {
    runtime.registerModule(id, { exports: { value: 1 } });
  });
  runtime.registerFactory('foo.js', factory);

  expect(runtime.isExecuted('foo.js')).toBe(false);
  expect(runtime.hasFactory('foo.js')).toBe(true);

  expect(runtime.initModule('foo.js')).toEqual({ value: 1 });
  expect(factory).toHaveBeenCalledTimes(1);
  expect(runtime.isExecuted('foo.js')).toBe(true);

  // registered → the factory is skipped, the live exports come back
  expect(runtime.initModule('foo.js')).toEqual({ value: 1 });
  expect(factory).toHaveBeenCalledTimes(1);
});

test('initModule throws MissingFactoryError when no factory is mapped', async () => {
  const { runtime, MissingFactoryError } = await createRuntime();
  expect(() => runtime.initModule('nope.js')).toThrow(MissingFactoryError);
  try {
    runtime.initModule('nope.js');
  } catch (err: any) {
    expect(err.id).toBe('nope.js');
  }
});

test('removeModuleCache deletes only the registry entry, fires the hook, and re-arms the factory', async () => {
  const { runtime } = await createRuntime();
  let generation = 0;
  runtime.registerFactory('foo.js', (id: string) => {
    generation += 1;
    runtime.registerModule(id, { exports: { generation } });
  });

  const onModuleCacheRemoval = vi.fn();
  runtime.hooks = { createModuleHotContext: () => ({}), onModuleCacheRemoval };

  expect(runtime.initModule('foo.js')).toEqual({ generation: 1 });
  expect(runtime.isExecuted('foo.js')).toBe(true);

  runtime.removeModuleCache('foo.js');
  expect(onModuleCacheRemoval).toHaveBeenCalledWith('foo.js');
  expect(runtime.isExecuted('foo.js')).toBe(false);
  // factories persist across a module-cache removal
  expect(runtime.hasFactory('foo.js')).toBe(true);

  // re-init re-runs the factory (cache-gated) → fresh generation
  expect(runtime.initModule('foo.js')).toEqual({ generation: 2 });
});

test('a factory that throws mid-body stays registered', async () => {
  const { runtime } = await createRuntime();
  runtime.registerFactory('broken.js', (id: string) => {
    runtime.registerModule(id, { exports: {} });
    throw new Error('boom');
  });

  expect(() => runtime.initModule('broken.js')).toThrow('boom');
  // registration is the first factory statement and nothing un-registers on unwind
  expect(runtime.isExecuted('broken.js')).toBe(true);
});

test('createModuleHotContext delegates to installed hooks', async () => {
  const { runtime } = await createRuntime();
  expect(() => runtime.createModuleHotContext('foo.js')).toThrow();

  const ctx = { accept: () => {} };
  runtime.hooks = { createModuleHotContext: () => ctx, onModuleCacheRemoval: () => {} };
  expect(runtime.createModuleHotContext('foo.js')).toBe(ctx);
});
