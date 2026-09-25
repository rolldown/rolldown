import fs from 'node:fs';

import { expect, test, vi } from 'vitest';

import { getDefaultDevRuntime } from '../../src/utils/default-dev-runtime';

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
  expect(Object.keys(runtimeModule).sort()).toEqual(['DevRuntime', 'MissingFactoryError']);
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

  expect(runtimeFiles).toEqual(['experimental-runtime.mjs']);
});

test('the default runtime loads when running from source', () => {
  const runtime = getDefaultDevRuntime('example.test', 1234);

  expect(runtime).toContain('class DevRuntime');
  expect(runtime).toContain('example.test:1234');
  expect(runtime).not.toContain('$ADDR');
});

test('registerGraph maintains static + dynamic reverse indexes; getImporters unions them', async () => {
  const { runtime } = await createRuntime();

  // app → foo (static edge), app ⇢ lazy (dynamic import() edge)
  runtime.registerGraph({
    ids: ['app.js', 'foo.js', 'lazy.js'],
    localCount: 3,
    edges: [[1], [], []],
    dynamicEdges: [[2], [], []],
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
    dynamicEdges: [[1], []],
  });
  expect(runtime.getImporters('foo.js')).toEqual([]);
  expect(runtime.getImporters('lazy.js')).toEqual([]);
  expect(runtime.getImporters('both.js')).toEqual(['app.js']);
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
