import { getDevWatchOptionsForCi } from '@rolldown/test-dev-server';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

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
test(
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

test('registerGraph keeps the entry flag of each row it carries', async () => {
  const { runtime } = await createRuntime();

  // `foo.js` is a foreign target here, so this payload says nothing about it
  runtime.registerGraph({
    ids: ['main.js', 'app.js', 'foo.js'],
    localCount: 2,
    edges: [[1], [2]],
    entries: [0],
  });
  expect(runtime.isEntry('main.js')).toBe(true);
  expect(runtime.isEntry('app.js')).toBe(false);
  expect(runtime.isEntry('foo.js')).toBe(false);

  runtime.registerGraph({ ids: ['foo.js'], localCount: 1, edges: [[]] });
  expect(runtime.isEntry('main.js')).toBe(true);

  runtime.registerGraph({ ids: ['main.js', 'app.js'], localCount: 2, edges: [[1], []] });
  expect(runtime.isEntry('main.js')).toBe(false);

  runtime.registerGraph({ ids: ['main.js'], localCount: 1, edges: [[]], entries: [0] });
  runtime.removeModuleCache('main.js');
  expect(runtime.isEntry('main.js')).toBe(true);

  // a late row from an older payload does not change the flag
  runtime.beginPayload(2).registerGraph({
    ids: ['main.js'],
    localCount: 1,
    edges: [[]],
    entries: [0],
  });
  runtime.beginPayload(1).registerGraph({ ids: ['main.js'], localCount: 1, edges: [[]] });
  expect(runtime.isEntry('main.js')).toBe(true);
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

test('a payload that arrives late does not replace newer factories or graph rows', async () => {
  const { runtime } = await createRuntime();

  // an HMR patch rendered at gen 2 lands first
  const patch = runtime.beginPayload(2);
  patch.registerGraph({ ids: ['foo.js', 'bar.js'], localCount: 1, edges: [[1]] });
  patch.registerFactory('foo.js', (id: string) =>
    runtime.registerModule(id, { exports: { v: 2 } }),
  );

  // a lazy chunk rendered before that edit (gen 1) lands after it
  const late = runtime.beginPayload(1);
  late.registerGraph({ ids: ['foo.js'], localCount: 1, edges: [[]] });
  late.registerFactory('foo.js', (id: string) => runtime.registerModule(id, { exports: { v: 1 } }));

  expect(runtime.getImporters('bar.js')).toEqual(['foo.js']);
  expect(runtime.initModule('foo.js')).toEqual({ v: 2 });

  // the same gen replaces (the copies are the same), and so does a newer one
  runtime
    .beginPayload(3)
    .registerFactory('foo.js', (id: string) => runtime.registerModule(id, { exports: { v: 3 } }));
  runtime.removeModuleCache('foo.js');
  expect(runtime.initModule('foo.js')).toEqual({ v: 3 });
});

// Two payloads can carry the same factory text with different rows: `bindings` of a row depend
// on whether an importee calls `acceptExports`. The newer render wins, whatever lands last.
test('the newer of two payloads wins, whatever order they land in', async () => {
  const { runtime } = await createRuntime();

  runtime.beginPayload(5).registerGraph({
    ids: ['x.js', 'y.js'],
    localCount: 1,
    edges: [[1]],
    bindings: { 0: [['a']] },
  });
  runtime.beginPayload(4).registerGraph({ ids: ['x.js', 'y.js'], localCount: 1, edges: [[1]] });
  expect(runtime.getImportedBindings('x.js', 'y.js')).toEqual(['a']);
});

test('gen 0 is a payload gen, and only rows with no gen are full-build rows', async () => {
  const { runtime } = await createRuntime();

  runtime.beginPayload(0).registerGraph({ ids: ['foo.js'], localCount: 1, edges: [[]] });
  expect(runtime.rowGens.get('foo.js')).toBe(0);

  runtime.registerGraph({ ids: ['foo.js'], localCount: 1, edges: [[]], epoch: 7 });
  expect(runtime.rowGens.has('foo.js')).toBe(false);
  expect(runtime.epoch).toBe(7);
  // payload rows carry no epoch, and the first full-build rows set it only once
  runtime.registerGraph({ ids: ['foo.js'], localCount: 1, edges: [[]], epoch: 8 });
  expect(runtime.epoch).toBe(7);
});

test('full-build rows always replace the row and keep the factory floor', async () => {
  const { runtime } = await createRuntime();

  const patch = runtime.beginPayload(2);
  patch.registerGraph({ ids: ['foo.js', 'bar.js'], localCount: 1, edges: [[1]] });
  patch.registerFactory('foo.js', () => {});
  // a full-build chunk calls the runtime's own `registerGraph`, with no gen
  runtime.registerGraph({ ids: ['foo.js'], localCount: 1, edges: [[]] });
  expect(runtime.getImporters('bar.js')).toEqual([]);

  // the full-build row did not lower the floor of the factory
  runtime.factories.delete('foo.js');
  runtime.beginPayload(1).registerFactory('foo.js', () => {});
  expect(runtime.hasFactory('foo.js')).toBe(false);
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

/**
 * Gives `ids` the numbers `0, 1, …` through a full-build row set, the way a page starts. The
 * server tells the page the generation of its output, because the rows do not carry it.
 */
function registerBuildRows(runtime: any, ids: string[], edges: number[][], gen = 0) {
  runtime.setOutputGeneration(gen);
  runtime.registerGraph({
    ids,
    localCount: ids.length,
    edges,
    nums: ids.map((_, i) => i),
    epoch: 7,
  });
}

/** The numbers of a `b` set: a bitset in base64url, bit `n` for number `n`. */
function decodeNumSet(value: string): number[] {
  expect(value[0]).toBe('b');
  const bytes = Buffer.from(value.slice(1), 'base64url');
  const nums: number[] = [];
  bytes.forEach((byte, i) => {
    for (let bit = 0; bit < 8; bit++) {
      if (byte & (1 << bit)) nums.push(i * 8 + bit);
    }
  });
  return nums;
}

test('a request names modules by number in bitsets', async () => {
  const { runtime } = await createRuntime();
  const ids = Array.from({ length: 2000 }, (_, i) => `m${i}.js`);
  registerBuildRows(
    runtime,
    ids,
    ids.map(() => []),
    3,
  );
  for (const i of [0, 3, 8, 17]) runtime.registerModule(ids[i]);
  for (const i of [1999, 1000, 5]) runtime.registerFactory(ids[i], () => {});
  // A module with no number is left out.
  runtime.registerModule('unknown.js');

  const query = new URLSearchParams(runtime.encodeHave(new Set(['m3.js'])));
  expect(query.get('s')).toBe('7');
  expect(query.get('g')).toBe('3');
  // Bit 3 of byte 0.
  expect(query.get('u')).toBe('bCA');
  // Bits 0 and 17 (byte 0 bit 0, byte 2 bit 1) and 8 (byte 1 bit 0), without the update.
  expect(query.get('e')).toBe('bAQEC');
  // A bitset ends at its last nonzero byte.
  expect(decodeNumSet(query.get('f')!)).toEqual([5, 1000, 1999]);
  expect(Buffer.from(query.get('f')!.slice(1), 'base64url')).toHaveLength(250);
  // A new nonce for every request.
  const nonce = new URLSearchParams(runtime.encodeHave(new Set())).get('n');
  expect(nonce).not.toBe(query.get('n'));
  // An empty set is left out.
  expect(runtime.encodeHave(new Set())).not.toContain('u=');
});

test('markStale drops older factories and keeps a late older payload out', async () => {
  const { runtime } = await createRuntime();
  registerBuildRows(runtime, ['a.js'], [[]], 1);
  runtime.beginPayload(1).registerFactory('a.js', () => {});

  runtime.markStale(['a.js'], 2);
  expect(runtime.hasFactory('a.js')).toBe(false);
  expect(runtime.knownGen).toBe(2);

  // A lazy chunk rendered before the update arrives after it.
  runtime.beginPayload(1).registerFactory('a.js', () => {});
  expect(runtime.hasFactory('a.js')).toBe(false);
  runtime.beginPayload(2).registerFactory('a.js', () => {});
  expect(runtime.hasFactory('a.js')).toBe(true);

  // A factory that is already newer stays.
  runtime.markStale(['a.js'], 2);
  expect(runtime.hasFactory('a.js')).toBe(true);
});

test('canRun counts the update as not executed and follows static imports', async () => {
  const { runtime } = await createRuntime();
  // main -> parent -> child
  registerBuildRows(runtime, ['main.js', 'parent.js', 'child.js'], [[1], [2], []]);
  for (const id of ['main.js', 'parent.js', 'child.js']) runtime.registerModule(id);

  expect(runtime.canRun(new Set(['parent.js']))).toBe(false);
  runtime.registerFactory('parent.js', () => {});
  // `child.js` ran and is not in the update.
  expect(runtime.canRun(new Set(['parent.js']))).toBe(true);
  // In the update, `child.js` must run again and has no factory.
  expect(runtime.canRun(new Set(['parent.js', 'child.js']))).toBe(false);
});

test('ensureRunnable pulls at most twice, then asks for a reload', async () => {
  const { runtime } = await createRuntime();
  registerBuildRows(runtime, ['main.js', 'hot.js'], [[1], []], 1);
  runtime.registerModule('main.js');
  runtime.registerModule('hot.js');

  const urls: string[] = [];
  runtime.importPatch = async (url: string) => {
    urls.push(url);
  };
  const result = await runtime.ensureRunnable(new Set(['hot.js']));
  expect(result).toEqual({ type: 'reload', reason: expect.stringContaining('hot.js') });
  expect(urls).toHaveLength(2);
  expect(urls[0]).toMatch(/^\/__rolldown_hmr_patch\.js\?s=7&g=1&u=bAg&e=bAQ&n=\d+$/);

  // A pull that brings the factory is enough.
  urls.length = 0;
  runtime.importPatch = async (url: string) => {
    urls.push(url);
    const payload = runtime.beginPayload(1);
    payload.registerGraph({ ids: ['hot.js'], localCount: 1, edges: [[]], nums: [1] });
    payload.registerFactory('hot.js', () => {});
  };
  expect(await runtime.ensureRunnable(new Set(['hot.js']))).toEqual({ type: 'ready' });
  expect(urls).toHaveLength(1);
  // Nothing to pull.
  expect(await runtime.ensureRunnable(new Set(['hot.js']))).toEqual({ type: 'ready' });
  expect(urls).toHaveLength(1);
});

test('a catch-up marks the missed updates and hands their changes to the walk', async () => {
  const { runtime } = await createRuntime();
  registerBuildRows(runtime, ['main.js', 'a.js', 'b.js'], [[1, 2], [], []], 1);
  for (const id of ['main.js', 'a.js', 'b.js']) runtime.registerModule(id);
  runtime.beginPayload(1).registerFactory('b.js', () => {});

  runtime.importPatch = async () => {
    // The server is at gen 3. It restamped `a.js` at gen 2 and changed `b.js` at gen 3. The
    // patch carries `a.js` as of gen 3.
    const payload = runtime.beginPayload(3);
    payload.catchUp([
      { gen: 2, staleIds: ['a.js'] },
      { gen: 3, changedIds: ['b.js'], staleIds: ['b.js'] },
    ]);
    payload.registerGraph({ ids: ['a.js'], localCount: 1, edges: [[]], nums: [1] });
    payload.registerFactory('a.js', () => {});
  };
  expect(await runtime.ensureRunnable(new Set(['a.js']))).toEqual({
    type: 'caught-up',
    changedIds: ['b.js'],
  });
  expect(runtime.knownGen).toBe(3);
  // The factory of the patch is newer than its catch-up entry, so it stays.
  expect(runtime.hasFactory('a.js')).toBe(true);
  expect(runtime.hasFactory('b.js')).toBe(false);

  // A catch-up the client already applied changes nothing.
  runtime.appliedGen = 3;
  runtime.beginPayload(3).catchUp([{ gen: 3, changedIds: ['a.js'], staleIds: ['a.js'] }]);
  expect(runtime.hasFactory('a.js')).toBe(true);
});

test('a catch-up that lands before an update of its generation applies is not walked again', async () => {
  const { runtime } = await createRuntime();
  const onCaughtUp = vi.fn();
  runtime.hooks = {
    createModuleHotContext: () => ({}),
    onModuleCacheRemoval: () => {},
    onCaughtUp,
  };
  registerBuildRows(runtime, ['main.js', 'a.js', 'b.js'], [[1, 2], [], []], 1);
  for (const id of ['main.js', 'a.js', 'b.js']) runtime.registerModule(id);

  // The update of the push of gen 2 is ready, but it did not apply yet.
  runtime.markStale(['a.js'], 2);
  expect(runtime.knownGen).toBe(2);
  // A lazy chunk asked for at gen 1 lands with the catch-up of gens 2 and 3.
  runtime.beginPayload(3).catchUp([
    { gen: 2, changedIds: ['a.js'], staleIds: ['a.js'] },
    { gen: 3, changedIds: ['b.js'], staleIds: ['b.js'] },
  ]);
  // The update of gen 2 applies and runs `a.js`.
  runtime.appliedGen = 2;

  runtime.notifyCaughtUp();
  expect(onCaughtUp).toHaveBeenCalledTimes(1);
  runtime.importPatch = async () => {};
  expect(await runtime.ensureRunnable(new Set())).toEqual({
    type: 'caught-up',
    changedIds: ['b.js'],
  });

  // When the update of gen 3 also applied first, nothing is left to walk.
  runtime.beginPayload(3).catchUp([{ gen: 3, changedIds: ['b.js'], staleIds: [] }]);
  runtime.appliedGen = 3;
  runtime.notifyCaughtUp();
  expect(onCaughtUp).toHaveBeenCalledTimes(1);
  expect(runtime.hasCaughtUpChanges()).toBe(false);
});

test('a missing factory calls the onMissingFactory hook before it throws', async () => {
  const { runtime, MissingFactoryError } = await createRuntime();
  const onMissingFactory = vi.fn();
  runtime.hooks = {
    createModuleHotContext: () => ({}),
    onModuleCacheRemoval: () => {},
    onMissingFactory,
  };
  expect(() => runtime.initModule('nope.js')).toThrow(MissingFactoryError);
  expect(onMissingFactory).toHaveBeenCalledWith('nope.js');
});

test('requestLazy loads the chunk again once, then asks for a reload', async () => {
  const { runtime, MissingFactoryError } = await createRuntime();
  const onMissingFactory = vi.fn();
  runtime.hooks = {
    createModuleHotContext: () => ({}),
    onModuleCacheRemoval: () => {},
    onMissingFactory,
  };
  const loads: string[] = [];
  runtime.loadLazyChunk = async (proxyModuleId: string) => {
    loads.push(proxyModuleId);
  };
  await expect(runtime.requestLazy('lazy.js', 'lazy.js?rolldown-lazy=1')).rejects.toThrow(
    MissingFactoryError,
  );
  expect(loads).toEqual(['lazy.js?rolldown-lazy=1', 'lazy.js?rolldown-lazy=1']);
  expect(onMissingFactory).toHaveBeenCalledWith('lazy.js');
});

test('loadDynamic pulls a target the client never received, with the target as the root', async () => {
  const { runtime, MissingFactoryError } = await createRuntime();
  const onMissingFactory = vi.fn();
  runtime.hooks = {
    createModuleHotContext: () => ({}),
    onModuleCacheRemoval: () => {},
    onMissingFactory,
  };
  // `route.js` is only a foreign `import()` target of `main.js`, but its number is known.
  runtime.setOutputGeneration(1);
  runtime.registerGraph({
    ids: ['main.js', 'route.js'],
    localCount: 1,
    edges: [[]],
    dynamicEdges: { 0: [1] },
    nums: [0, 1],
    epoch: 7,
  });
  runtime.registerModule('main.js');

  const urls: string[] = [];
  runtime.importPatch = async (url: string) => {
    urls.push(url);
    const payload = runtime.beginPayload(1);
    payload.registerGraph({ ids: ['route.js'], localCount: 1, edges: [[]], nums: [1] });
    payload.registerFactory('route.js', (id: string) =>
      runtime.registerModule(id, { exports: { value: 1 } }),
    );
  };
  expect(await runtime.loadDynamic('route.js')).toEqual({ value: 1 });
  expect(urls).toHaveLength(1);
  expect(urls[0]).toMatch(/^\/__rolldown_hmr_patch\.js\?s=7&g=1&u=bAg&e=bAQ&n=\d+$/);
  // It ran, so a later `import()` needs no pull.
  expect(await runtime.loadDynamic('route.js')).toEqual({ value: 1 });
  expect(urls).toHaveLength(1);

  // A pull that does not bring the target is tried once more, then the client reloads.
  urls.length = 0;
  runtime.importPatch = async (url: string) => {
    urls.push(url);
  };
  await expect(runtime.loadDynamic('gone.js')).rejects.toThrow(MissingFactoryError);
  expect(urls).toHaveLength(2);
  expect(onMissingFactory).toHaveBeenCalledWith('gone.js');
});

test('a reload answer calls the onReloadNeeded hook once and stops all requests', async () => {
  const { runtime } = await createRuntime();
  const onReloadNeeded = vi.fn();
  runtime.hooks = {
    createModuleHotContext: () => ({}),
    onModuleCacheRemoval: () => {},
    onReloadNeeded,
  };
  registerBuildRows(runtime, ['main.js', 'hot.js'], [[1], []], 1);
  runtime.registerModule('main.js');
  runtime.registerModule('hot.js');

  const urls: string[] = [];
  // What the body of a reload answer runs.
  runtime.importPatch = async (url: string) => {
    urls.push(url);
    runtime.reloadNeeded('the epoch does not match');
  };
  expect(await runtime.ensureRunnable(new Set(['hot.js']))).toEqual({
    type: 'reload',
    reason: 'the epoch does not match',
  });
  expect(urls).toHaveLength(1);

  // Later requests are not sent, and the `import()` calls reject.
  const loads: string[] = [];
  runtime.loadLazyChunk = async (proxyModuleId: string) => {
    loads.push(proxyModuleId);
  };
  await expect(runtime.requestLazy('lazy.js', 'lazy.js?rolldown-lazy=1')).rejects.toThrow(
    'the epoch does not match',
  );
  await expect(runtime.loadDynamic('route.js')).rejects.toThrow('the epoch does not match');
  expect(loads).toEqual([]);
  expect(urls).toHaveLength(1);
  runtime.reloadNeeded('another reason');
  expect(onReloadNeeded.mock.calls).toEqual([['the epoch does not match']]);
});

test('a failed pull of loadDynamic rejects without a reload', async () => {
  const { runtime } = await createRuntime();
  const onMissingFactory = vi.fn();
  const onReloadNeeded = vi.fn();
  runtime.hooks = {
    createModuleHotContext: () => ({}),
    onModuleCacheRemoval: () => {},
    onMissingFactory,
    onReloadNeeded,
  };
  runtime.importPatch = async () => {
    throw new Error('500');
  };
  await expect(runtime.loadDynamic('route.js')).rejects.toThrow('500');
  expect(onMissingFactory).not.toHaveBeenCalled();
  expect(onReloadNeeded).not.toHaveBeenCalled();
});

test('a push that skips a generation keeps knownGen, and the next pull asks for the catch-up', async () => {
  const { runtime } = await createRuntime();
  registerBuildRows(runtime, ['main.js', 'a.js', 'b.js'], [[1, 2], [], []], 1);
  for (const id of ['main.js', 'a.js', 'b.js']) runtime.registerModule(id);
  runtime.beginPayload(1).registerFactory('b.js', () => {});

  // The next generation raises knownGen.
  runtime.markStale([], 2);
  expect(runtime.knownGen).toBe(2);
  // The push of gen 3 never came. Gen 4 does not raise knownGen.
  runtime.markStale(['b.js'], 4);
  expect(runtime.knownGen).toBe(2);
  expect(runtime.pushedGen).toBe(4);

  const urls: string[] = [];
  runtime.importPatch = async (url: string) => {
    urls.push(url);
    runtime.beginPayload(4).catchUp([
      { gen: 3, changedIds: ['a.js'], staleIds: ['a.js'] },
      { gen: 4, changedIds: ['b.js'], staleIds: ['b.js'] },
    ]);
  };
  // Nothing of the update is missing, but the client still asks for the missed update.
  expect(await runtime.ensureRunnable(new Set())).toEqual({
    type: 'caught-up',
    changedIds: ['a.js', 'b.js'],
  });
  expect(urls).toHaveLength(1);
  expect(new URLSearchParams(urls[0].split('?')[1]).get('g')).toBe('2');
  expect(runtime.knownGen).toBe(4);
  expect(await runtime.ensureRunnable(new Set())).toEqual({ type: 'ready' });
  expect(urls).toHaveLength(1);

  // A server that never brings the catch-up gets a reload after two requests.
  runtime.markStale([], 6);
  runtime.importPatch = async (url: string) => {
    urls.push(url);
  };
  expect(await runtime.ensureRunnable(new Set())).toEqual({
    type: 'reload',
    reason: 'no catch-up for generation 6 after 2 requests',
  });
  expect(urls).toHaveLength(3);
});

test('a client that ran nothing yet takes no catch-up from a push', async () => {
  const { runtime } = await createRuntime();
  // A module runner before it fetched its entry.
  runtime.knownGen = -1;
  runtime.markStale(['a.js'], 3);
  expect(runtime.knownGen).toBe(-1);
  const importPatch = vi.fn();
  runtime.importPatch = importPatch;
  expect(await runtime.ensureRunnable(new Set())).toEqual({ type: 'ready' });
  expect(importPatch).not.toHaveBeenCalled();

  // The entry is from an output at gen 1, so the update of gen 3 is missed.
  registerBuildRows(runtime, ['main.js', 'a.js'], [[1], []], 1);
  for (const id of ['main.js', 'a.js']) runtime.registerModule(id);
  expect(runtime.knownGen).toBe(1);
  expect(runtime.missedUpdate()).toBe(true);
});

test('a push that comes before the output ran counts as missed', async () => {
  const { runtime } = await createRuntime();
  // The HTML said gen 3, and the push of gen 4 comes while the entry chunk still loads.
  runtime.setOutputGeneration(3);
  runtime.markStale(['a.js'], 4);
  expect(runtime.knownGen).toBe(3);
  expect(runtime.missedUpdate()).toBe(false);

  // The entry of gen 3 runs the old `a.js`.
  registerBuildRows(runtime, ['main.js', 'a.js'], [[1], []], 3);
  for (const id of ['main.js', 'a.js']) runtime.registerModule(id);
  expect(runtime.missedUpdate()).toBe(true);

  const urls: string[] = [];
  runtime.importPatch = async (url: string) => {
    urls.push(url);
    runtime.beginPayload(4).catchUp([{ gen: 4, changedIds: ['a.js'], staleIds: ['a.js'] }]);
  };
  expect(await runtime.ensureRunnable(new Set())).toEqual({
    type: 'caught-up',
    changedIds: ['a.js'],
  });
  expect(new URLSearchParams(urls[0].split('?')[1]).get('g')).toBe('3');
  expect(runtime.knownGen).toBe(4);
});

test('a client that does not know its generation leaves out g and learns it from the answer', async () => {
  const { runtime } = await createRuntime();
  runtime.knownGen = -1;
  runtime.registerGraph({
    ids: ['main.js', 'a.js'],
    localCount: 2,
    edges: [[1], []],
    nums: [0, 1],
    epoch: 7,
  });
  for (const id of ['main.js', 'a.js']) runtime.registerModule(id);
  expect(runtime.encodeHave(new Set())).toMatch(/^s=7&e=bAw&n=\d+$/);

  // The server filled in gen 6 and found the client up to date. A payload with no module is only
  // its prelude, and the prelude tells the gen.
  const urls: string[] = [];
  runtime.importPatch = async (url: string) => {
    urls.push(url);
    runtime.beginPayload(6);
  };
  runtime.markStale(['a.js'], 6);
  expect(runtime.knownGen).toBe(-1);
  expect(await runtime.ensureRunnable(new Set())).toEqual({ type: 'ready' });
  expect(urls).toHaveLength(1);
  expect(new URLSearchParams(urls[0].split('?')[1]).has('g')).toBe(false);
  expect(runtime.knownGen).toBe(6);
  // An older payload that lands later does not lower it.
  runtime.beginPayload(5);
  expect(runtime.knownGen).toBe(6);
});

test('f lists only the factories of modules that did not run or are in the update', async () => {
  const { runtime } = await createRuntime();
  registerBuildRows(runtime, ['main.js', 'a.js', 'b.js', 'c.js'], [[1, 2], [], [], []]);
  for (const id of ['main.js', 'a.js', 'b.js']) runtime.registerModule(id);
  for (const id of ['a.js', 'b.js', 'c.js']) runtime.registerFactory(id, () => {});

  // `b.js` ran and is not in the update, so its factory is left out.
  const query = new URLSearchParams(runtime.encodeHave(new Set(['a.js'])));
  // {1, 3}: `a.js` and `c.js`
  expect(query.get('f')).toBe('bCg');

  // After a module leaves the module cache, its factory is listed again.
  runtime.removeModuleCache('b.js');
  expect(new URLSearchParams(runtime.encodeHave(new Set())).get('f')).toBe('bDA');
  // After it runs, it is not.
  runtime.registerModule('b.js');
  expect(new URLSearchParams(runtime.encodeHave(new Set())).get('f')).toBe('bCA');
});

test('a dropped factory of a module that did not run is pulled again', async () => {
  const { runtime } = await createRuntime();
  // `a.js` ran and requires `b.js` inside a function, so `b.js` did not run.
  registerBuildRows(runtime, ['main.js', 'a.js', 'b.js', 'c.js'], [[1, 3], [2], [], []], 1);
  for (const id of ['main.js', 'a.js', 'c.js']) runtime.registerModule(id);
  const boot = runtime.beginPayload(1);
  boot.registerFactory('b.js', () => {});
  // `c.js` is not stale-dropped: it ran.
  boot.registerFactory('c.js', () => {});

  const urls: string[] = [];
  runtime.importPatch = async (url: string) => {
    urls.push(url);
    const payload = runtime.beginPayload(2);
    payload.registerGraph({ ids: ['b.js'], localCount: 1, edges: [[]], nums: [2] });
    payload.registerFactory('b.js', () => {});
  };
  runtime.markStale(['b.js', 'c.js'], 2);
  expect([...runtime.droppedDeps]).toEqual(['b.js']);

  // No update pulled it, so it is pulled in the background, as a root of `u`.
  await runtime.refetchDropped();
  expect(urls).toHaveLength(1);
  // spellchecker:ignore-next-line
  expect(new URLSearchParams(urls[0].split('?')[1]).get('u')).toBe('bBA');
  expect(runtime.hasFactory('b.js')).toBe(true);
  expect(runtime.droppedDeps.size).toBe(0);
  // Nothing left to pull.
  await runtime.refetchDropped();
  expect(urls).toHaveLength(1);

  // An update that pulls carries it, so no second request is needed.
  runtime.markStale(['b.js'], 3);
  runtime.markStale(['a.js'], 4);
  runtime.importPatch = async (url: string) => {
    urls.push(url);
    const payload = runtime.beginPayload(4);
    payload.registerFactory('a.js', () => {});
    payload.registerFactory('b.js', () => {});
  };
  expect(await runtime.ensureRunnable(new Set(['a.js']))).toEqual({ type: 'ready' });
  expect(urls).toHaveLength(2);
  // {1, 2}: `a.js` and `b.js`
  expect(new URLSearchParams(urls[1].split('?')[1]).get('u')).toBe('bBg');
  await runtime.refetchDropped();
  expect(urls).toHaveLength(2);
});

/** `main.js` ran and requires `b.js` inside a function, and a push at gen 2 dropped `b.js`. */
async function createRuntimeWithDroppedDep() {
  const { runtime } = await createRuntime();
  registerBuildRows(runtime, ['main.js', 'b.js'], [[1], []], 1);
  runtime.registerModule('main.js');
  runtime.beginPayload(1).registerFactory('b.js', () => {});
  runtime.markStale(['b.js'], 2);
  expect([...runtime.droppedDeps]).toEqual(['b.js']);
  return runtime;
}

test('a pull that fails or brings no factory puts the dropped deps back', async () => {
  const runtime = await createRuntimeWithDroppedDep();

  runtime.importPatch = async () => {
    throw new Error('500');
  };
  await expect(runtime.pullPatch(new Set())).rejects.toThrow('500');
  expect([...runtime.droppedDeps]).toEqual(['b.js']);
  // A failed refetch is ignored, and the dep stays for the next pull.
  await runtime.refetchDropped();
  expect([...runtime.droppedDeps]).toEqual(['b.js']);

  // A reload answer brings no factory either.
  runtime.importPatch = async () => {
    runtime.reloadNeeded('the epoch does not match');
  };
  expect(await runtime.pullPatch(new Set())).toBe(1);
  expect([...runtime.droppedDeps]).toEqual(['b.js']);
});

test('a refetch runs again when a newer push dropped the dep while its payload was on the way', async () => {
  const runtime = await createRuntimeWithDroppedDep();

  const urls: string[] = [];
  runtime.importPatch = async (url: string) => {
    urls.push(url);
    if (urls.length === 1) {
      // The push of gen 3 lands while the payload of gen 2 is on the way.
      runtime.markStale(['b.js'], 3);
      runtime.beginPayload(2).registerFactory('b.js', () => {});
    } else {
      runtime.beginPayload(3).registerFactory('b.js', () => {});
    }
  };
  await runtime.refetchDropped();
  expect(urls).toHaveLength(2);
  expect(runtime.hasFactory('b.js')).toBe(true);
  expect(runtime.droppedDeps.size).toBe(0);
});

test('a refetch whose payload is not newer does not run again', async () => {
  const runtime = await createRuntimeWithDroppedDep();
  runtime.beginPayload(2);

  const urls: string[] = [];
  runtime.importPatch = async (url: string) => {
    urls.push(url);
    // The server is still at gen 2, and the payload lacks `b.js`.
    runtime.beginPayload(2);
  };
  await runtime.refetchDropped();
  expect(urls).toHaveLength(1);
  expect([...runtime.droppedDeps]).toEqual(['b.js']);
});

/**
 * A client at gen 1 that holds the factory of `lazy.js` and then missed the update of gen 2:
 * the push of gen 3 skips it.
 */
async function createRuntimeThatMissedAnUpdate() {
  const { runtime } = await createRuntime();
  const onCaughtUp = vi.fn();
  runtime.hooks = {
    createModuleHotContext: () => ({}),
    onModuleCacheRemoval: () => {},
    onCaughtUp,
  };
  registerBuildRows(runtime, ['main.js', 'lazy.js'], [[], []], 1);
  runtime.registerModule('main.js');
  runtime
    .beginPayload(1)
    .registerFactory('lazy.js', (id: string) =>
      runtime.registerModule(id, { exports: { value: 'old' } }),
    );
  runtime.markStale([], 3);
  expect(runtime.missedUpdate()).toBe(true);
  /** The catch-up: gen 2 changed `lazy.js`. */
  const catchUp = () => {
    const payload = runtime.beginPayload(3);
    payload.catchUp([{ gen: 2, changedIds: ['lazy.js'], staleIds: ['lazy.js'] }]);
    payload.registerFactory('lazy.js', (id: string) =>
      runtime.registerModule(id, { exports: { value: 'new' } }),
    );
  };
  return { runtime, onCaughtUp, catchUp };
}

test('requestLazy gets the catch-up before it runs a held factory', async () => {
  const { runtime, onCaughtUp, catchUp } = await createRuntimeThatMissedAnUpdate();
  const loads: string[] = [];
  runtime.loadLazyChunk = async (proxyModuleId: string) => {
    loads.push(proxyModuleId);
    catchUp();
  };
  expect(await runtime.requestLazy('lazy.js', 'lazy.js?rolldown-lazy=1')).toEqual({
    value: 'new',
  });
  expect(loads).toEqual(['lazy.js?rolldown-lazy=1']);
  expect(runtime.knownGen).toBe(3);
  expect(onCaughtUp).toHaveBeenCalledTimes(1);
});

test('loadDynamic gets the catch-up before it runs a held factory', async () => {
  const { runtime, onCaughtUp, catchUp } = await createRuntimeThatMissedAnUpdate();
  const urls: string[] = [];
  runtime.importPatch = async (url: string) => {
    urls.push(url);
    catchUp();
  };
  expect(await runtime.loadDynamic('lazy.js')).toEqual({ value: 'new' });
  expect(urls).toHaveLength(1);
  expect(new URLSearchParams(urls[0].split('?')[1]).get('g')).toBe('1');
  expect(onCaughtUp).toHaveBeenCalledTimes(1);
});

test('requestLazy asks again after a failed load, but not after an init error', async () => {
  const { runtime } = await createRuntime();
  registerBuildRows(runtime, ['main.js'], [[]], 1);
  runtime.registerModule('main.js');

  let loads = 0;
  runtime.loadLazyChunk = async () => {
    loads++;
    if (loads === 1) throw new Error('500');
    const payload = runtime.beginPayload(1);
    payload.registerFactory('ok.js', (id: string) =>
      runtime.registerModule(id, { exports: { value: 1 } }),
    );
    payload.registerFactory('boom.js', (id: string) => {
      runtime.registerModule(id, { exports: {} });
      throw new Error('boom');
    });
  };
  await expect(runtime.requestLazy('ok.js', 'ok.js?rolldown-lazy=1')).rejects.toThrow('500');
  expect(await runtime.requestLazy('ok.js', 'ok.js?rolldown-lazy=1')).toEqual({ value: 1 });
  expect(loads).toBe(2);

  await expect(runtime.requestLazy('boom.js', 'boom.js?rolldown-lazy=1')).rejects.toThrow('boom');
  await expect(runtime.requestLazy('boom.js', 'boom.js?rolldown-lazy=1')).rejects.toThrow('boom');
  // The factory came with the chunk of `ok.js`, so `boom.js` needed no load.
  expect(loads).toBe(2);
});
