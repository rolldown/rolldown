import fs from 'node:fs';

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
});

test('the packaged runtime base is byte-identical to the bundled runtime base', () => {
  const packagedRuntimeBaseUrl = new URL(
    './experimental-runtime-base.mjs',
    import.meta.resolve('rolldown/experimental/runtime'),
  );
  const bundledRuntimeBaseUrl = new URL(
    '../../../../crates/rolldown/src/runtime/runtime-base.js',
    import.meta.url,
  );

  expect(fs.existsSync(packagedRuntimeBaseUrl)).toBe(true);
  if (!fs.existsSync(packagedRuntimeBaseUrl)) return;
  expect(fs.readFileSync(packagedRuntimeBaseUrl, 'utf8')).toBe(
    fs.readFileSync(bundledRuntimeBaseUrl, 'utf8'),
  );
});

test('the package does not emit a separate runtime injection source', () => {
  const runtimeSourceUrl = new URL(
    './experimental-runtime-source.mjs',
    import.meta.resolve('rolldown/experimental/runtime'),
  );

  expect(fs.existsSync(runtimeSourceUrl)).toBe(false);
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
  runtime.registerFactory('foo.js', 'esm', factory);

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
  runtime.registerFactory('foo.js', 'esm', (id: string) => {
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
  runtime.registerFactory('broken.js', 'esm', (id: string) => {
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
