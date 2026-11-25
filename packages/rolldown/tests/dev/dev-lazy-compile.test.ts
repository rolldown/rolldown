import { getDevWatchOptionsForCi } from '@rolldown/test-dev-server';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import type { InputOptions, OutputOptions, Plugin } from 'rolldown';
import type { DevEngine, DevOptions } from 'rolldown/experimental';
import { dev as _dev } from 'rolldown/experimental';
import { parseSync } from 'rolldown/utils';
import type * as DevRuntimeModule from 'rolldown/experimental/runtime';
import { isWasiTest } from 'rolldown-tests/utils';
import { SourceMapConsumer, SourceMapGenerator } from 'source-map';
import { expect, test } from 'vitest';

const TEST_TIMEOUT = 60_000;

function dev(
  inputOptions: InputOptions,
  outputOptions: OutputOptions,
  devOptions: DevOptions,
): Promise<DevEngine> {
  return _dev(inputOptions, outputOptions, {
    ...devOptions,
    watch: {
      ...devOptions.watch,
      watcher: {
        ...getDevWatchOptionsForCi().watcher,
        ...devOptions.watch?.watcher,
      },
    },
  });
}

async function loadDevRuntime(): Promise<typeof DevRuntimeModule.DevRuntime> {
  const { DevRuntime }: typeof DevRuntimeModule = await import(
    import.meta.resolve('rolldown/experimental/runtime')
  );
  return DevRuntime;
}

async function createDevRuntime(): Promise<DevRuntimeModule.DevRuntime> {
  const DevRuntime = await loadDevRuntime();
  const runtime = new DevRuntime();
  runtime.hooks = { createModuleHotContext: () => ({}), onModuleCacheRemoval: () => {} };
  return runtime;
}

function runInRuntime(code: string, runtime: DevRuntimeModule.DevRuntime): void {
  vm.runInThisContext(`(__rolldown_runtime__) => {\n${code}\n}`)(runtime);
}

/** Runs only the `registerGraph` statements of `code`, the full-build rows a client reads. */
function registerBuildGraph(code: string, runtime: DevRuntimeModule.DevRuntime): void {
  for (const graph of code.match(/__rolldown_runtime__\.registerGraph\(\{[\s\S]*?\}\);/g) ?? []) {
    runInRuntime(graph, runtime);
  }
}

/**
 * The stable id among the numbered modules of `runtime` that is `target`, or that ends with
 * `/target`.
 */
function lazyTargetId(runtime: DevRuntimeModule.DevRuntime, target: string): string {
  const ids = [...runtime.nums.keys()].filter((id) => id === target || id.endsWith(`/${target}`));
  if (ids.length !== 1) {
    throw new Error(`expected one module id for ${target}, got ${JSON.stringify(ids)}`);
  }
  return ids[0];
}

/** The numbers of a `b` (bitset) or `d` (gap varints) set of a pull query. */
function decodeNumSet(value: string): number[] {
  const bytes = Buffer.from(value.slice(1), 'base64url');
  const nums: number[] = [];
  if (value[0] === 'b') {
    bytes.forEach((byte, index) => {
      for (let bit = 0; bit < 8; bit++) if ((byte >> bit) & 1) nums.push(index * 8 + bit);
    });
    return nums;
  }
  let previous = 0;
  for (let pos = 0; pos < bytes.length;) {
    let gap = 0;
    for (let shift = 0; ; shift += 7) {
      const byte = bytes[pos++];
      gap |= (byte & 0x7f) << shift;
      if (!(byte & 0x80)) break;
    }
    previous += gap;
    nums.push(previous);
  }
  return nums;
}

function numToId(runtime: DevRuntimeModule.DevRuntime, num: number): string | undefined {
  for (const [id, n] of runtime.nums) if (n === num) return id;
}

/**
 * The query of a lazy `import()` of `target` (see `lazyTargetId`) from a client that loaded the
 * full build `code`. The modules of the build's rows for which `executed` is true count as run.
 */
async function lazyQuery(
  code: string,
  target: string,
  executed: (id: string) => boolean = () => true,
): Promise<string> {
  const runtime = await createDevRuntime();
  registerBuildGraph(code, runtime);
  for (const id of runtime.staticImports.keys()) {
    if (executed(id)) runtime.registerModule(id);
  }
  return runtime.encodeHave(new Set(), new Set([lazyTargetId(runtime, target)]));
}

function readBuild(dir: string): string {
  return fs.readFileSync(path.join(dir, 'dist', 'main.js'), 'utf8');
}

/** The stable id of the module `\0virtual:lazy-me`: the null byte is written as `\0`. */
const VIRTUAL_STABLE_ID = '\\0virtual:lazy-me';

/** Pulls with `query`, which names a lazy target (`lazyQuery`). */
async function compileChunk(engine: DevEngine, query: string) {
  const response = await engine.renderPatch(query);
  if (response.type !== 'Patch') {
    throw new Error(`expected a payload, got a reload: ${response.reason}`);
  }
  return response;
}

// A lazy request names its target by a module number that the engine gave out, never by a
// path. The engine reads only the keys of the pull query, so a path in the query is never
// resolved, and a number the engine did not give out gets a reload, not a scan (#9969). The Rust
// tests in `crates/rolldown_dev/src/dev_engine.rs` cover a number whose module left the graph.
test(
  'a lazy request never makes the engine resolve a path',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-lazy-compile-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    const input = path.join(dir, 'main.js');
    fs.writeFileSync(input, 'console.log(1)');
    const secret = path.join(dir, 'secret.js');
    fs.writeFileSync(secret, 'export const secret = "do-not-bundle";\n');

    const engine = await dev(
      {
        input,
        experimental: { devMode: { lazy: true } },
      },
      { dir: path.join(dir, 'dist') },
      {},
    );

    onTestFinished(async () => {
      await engine.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await engine.run();
    const runtime = await createDevRuntime();
    registerBuildGraph(readBuild(dir), runtime);
    const have = runtime.encodeHave(new Set());

    const withPath = await engine.renderPatch(
      `${have}&id=${encodeURIComponent(`${secret}?rolldown-lazy=1`)}`,
    );
    expect(withPath.type).toBe('Patch');
    expect(withPath.type === 'Patch' && withPath.code).not.toContain('do-not-bundle');

    // Number 1000 as a bitset: byte 125 is 01.
    const unknownNumber = await engine.renderPatch(`${have}&l=b${'A'.repeat(167)}B`);
    expect(unknownNumber.type).toBe('Reload');
    expect(engine.moduleGraph.getModuleIds().some((id) => id.includes('secret'))).toBe(false);
  },
);

// A lazy chunk carries only what the client cannot run yet. A module the client already ran
// serves the lazy import from its live exports, so the chunk has no factory for it. A client
// that ran nothing gets the whole closure.
test(
  'lazy chunk omits factories for modules the entry chunk evaluated at top level',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-lazy-evaluated-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(
      path.join(dir, 'main.js'),
      `import { shared } from './shared.js';\nconsole.log(shared);\nimport('./lazy.js');\n`,
    );
    fs.writeFileSync(path.join(dir, 'shared.js'), `export const shared = 'shared';\n`);
    fs.writeFileSync(
      path.join(dir, 'lazy.js'),
      `import { shared } from './shared.js';\nexport const lazy = shared + '-lazy';\n`,
    );

    const engine = await dev(
      {
        input: path.join(dir, 'main.js'),
        experimental: { devMode: { lazy: true } },
      },
      { dir: path.join(dir, 'dist') },
      {},
    );

    onTestFinished(async () => {
      await engine.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await engine.run();

    const sharedFactory = /registerFactory\("[^"]*shared\.js"/;
    const lazyFactory = /registerFactory\("[^"]*lazy\.js"/;
    // The proxy never runs in the client (`requestLazy` runs the real module), so no chunk
    // carries it.
    const proxyFactory = /registerFactory\("[^"]*\?rolldown-lazy=1"/;

    // `shared.js` is statically imported by the entry, so the client ran it.
    const chunk = await compileChunk(engine, await lazyQuery(readBuild(dir), 'lazy.js'));
    expect(chunk.code).toMatch(lazyFactory);
    expect(chunk.code).not.toMatch(sharedFactory);
    expect(chunk.code).not.toMatch(proxyFactory);

    const coldChunk = await compileChunk(
      engine,
      await lazyQuery(readBuild(dir), 'lazy.js', () => false),
    );
    expect(coldChunk.code).toMatch(lazyFactory);
    expect(coldChunk.code).toMatch(sharedFactory);
    expect(coldChunk.code).not.toMatch(proxyFactory);
  },
);

test(
  'lazy chunk includes a factory for an initially unloaded barrel',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-lazy-barrel-factory-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    const files = {
      'main.js': `import { render } from './view.js';\nconsole.log(render());\nimport('./lazy.js');\n`,
      'view.js': `import { format } from './barrel.js';\nexport const render = () => format('before');\nimport.meta.hot.accept();\n`,
      'barrel.js': `export * from './implementation.js';\n`,
      'implementation.js': `export const format = value => 'rendered:' + value;\n`,
      'lazy.js': `import { format } from './barrel.js';\nexport const value = format('lazy');\n`,
    };
    for (const [name, code] of Object.entries(files)) {
      fs.writeFileSync(path.join(dir, name), code);
    }

    const engine = await dev(
      {
        cwd: dir,
        input: './main.js',
        experimental: { devMode: { lazy: true } },
        optimization: { inlineConst: false },
        plugins: [
          {
            name: 'side-effect-free-barrel',
            resolveId(source) {
              if (source === './barrel.js') {
                return { id: path.join(dir, 'barrel.js'), moduleSideEffects: false };
              }
            },
          },
        ],
      },
      {
        dir: path.join(dir, 'dist'),
        codeSplitting: {
          groups: [
            {
              name: 'barrel',
              test: /barrel\.js$/,
              includeDependenciesRecursively: false,
              entriesAware: false,
              minShareCount: 1,
            },
          ],
        },
      },
      {},
    );
    onTestFinished(async () => {
      await engine.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await engine.run();
    // The client ran the modules of the entry chunk, and the barrel is in another chunk.
    const chunk = await compileChunk(engine, await lazyQuery(readBuild(dir), 'lazy.js'));
    expect(chunk.code).toMatch(/registerFactory\("[^"]*barrel\.js"/);
  },
);

/**
 * Writes `main.js` and a `lazy.js` whose `throw` sits on line 2, and returns a
 * plugin that pushes that line down — the shape of any transform that removes or
 * inserts lines, such as TypeScript type stripping.
 */
function setupShiftedLazyModule(dir: string) {
  const lazyPath = path.join(dir, 'lazy.js');
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(dir, 'main.js'), `globalThis.load = () => import('./lazy.js');\n`);
  const original = `export function boom() {\n  throw new Error('from lazy');\n}\n`;
  fs.writeFileSync(lazyPath, original);

  const shiftBy = 3;
  const plugin = {
    name: 'shift-lines',
    transform(code: string, id: string) {
      if (!id.endsWith('lazy.js')) {
        return;
      }
      const generator = new SourceMapGenerator({ file: id });
      code.split('\n').forEach((_, index) => {
        generator.addMapping({
          source: id,
          original: { line: index + 1, column: 0 },
          generated: { line: index + 1 + shiftBy, column: 0 },
        });
      });
      generator.setSourceContent(id, code);
      return { code: '\n'.repeat(shiftBy) + code, map: generator.toJSON() };
    },
  };

  return { lazyPath, plugin };
}

/** Line/column of the only `throw` in `code`, as a sourcemap consumer wants it. */
function throwPosition(code: string) {
  const lines = code.split('\n');
  const index = lines.findIndex((line) => line.includes('from lazy'));
  return { line: index + 1, column: lines[index].indexOf('throw') };
}

/**
 * `'inline'` puts the map at the end of the code. `true` and `'hidden'` return it apart, with no
 * comment in the code: no file is kept for a lazy chunk, so the dev server serves the map.
 */
function chunkSourceMap(
  chunk: { code: string; sourcemap?: string | null },
  sourcemap: 'inline' | 'hidden' | true,
) {
  if (sourcemap === 'inline') {
    const match = /sourceMappingURL=data:application\/json[^,]+base64,([\w+/=]+)\s*$/.exec(
      chunk.code,
    );
    expect(match, 'the lazy chunk should carry an inline sourcemap').not.toBeNull();
    return JSON.parse(Buffer.from(match![1], 'base64').toString());
  }
  expect(chunk.code).not.toContain('sourceMappingURL');
  expect(chunk.sourcemap).toBeTypeOf('string');
  return JSON.parse(chunk.sourcemap!);
}

// A module's sourcemap chain is what maps the rendered output back to the file
// the user wrote; the oxc codegen map is only its last element and on its own
// points at what the plugins produced. A lazy chunk collapses the whole chain,
// the same way a module rendered into a chunk does — otherwise every position in
// a lazily compiled module is off by whatever the transforms shifted, which is
// every stack frame and every devtools jump inside it.
test.for(['inline', true, 'hidden'] as const)(
  'lazy chunk sourcemap maps through the plugin sourcemap chain (sourcemap: %s)',
  { timeout: TEST_TIMEOUT },
  async (sourcemap, { onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-lazy-sourcemap-${uniqueId}`);
    const { lazyPath, plugin } = setupShiftedLazyModule(dir);

    const engine = await dev(
      {
        input: path.join(dir, 'main.js'),
        plugins: [plugin],
        experimental: { devMode: { lazy: true } },
      },
      { dir: path.join(dir, 'dist'), sourcemap },
      {},
    );

    onTestFinished(async () => {
      await engine.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await engine.run();
    await engine.ensureCurrentBuildFinish();
    const chunk = await compileChunk(engine, await lazyQuery(readBuild(dir), 'lazy.js'));
    const position = await SourceMapConsumer.with(
      chunkSourceMap(chunk, sourcemap),
      null,
      (consumer) => consumer.originalPositionFor(throwPosition(chunk.code)),
    );

    // Line 2 of the file on disk, not line 5 of what the plugin handed the bundler.
    expect(position.line).toBe(2);
    expect(path.resolve(path.join(dir, 'dist'), position.source!)).toBe(lazyPath);
  },
);

test(
  'lazy proxy modules skip user plugin hooks',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-lazy-skip-hooks-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    const main = path.join(dir, 'main.js');
    const lazy = path.join(dir, 'lazy.js');
    fs.writeFileSync(main, `globalThis.load = () => import('./lazy.js');\n`);
    fs.writeFileSync(lazy, `export const value = 1;\n`);

    const seenTransformIds: string[] = [];
    const seenModuleParsedIds: string[] = [];
    const plugin = {
      name: 'assert-no-lazy-proxy-hooks',
      load: {
        order: 'pre',
        handler(id: string) {
          if (id.includes('?rolldown-lazy=1')) {
            throw new Error(`load hook saw lazy proxy id: ${id}`);
          }
        },
      },
      transform(code: string, id: string) {
        seenTransformIds.push(id);
        if (id.includes('?rolldown-lazy=1')) {
          throw new Error(`transform hook saw lazy proxy id: ${id}`);
        }
        return code;
      },
      moduleParsed(moduleInfo: { id: string }) {
        seenModuleParsedIds.push(moduleInfo.id);
        if (moduleInfo.id.includes('?rolldown-lazy=1')) {
          throw new Error(`moduleParsed hook saw lazy proxy id: ${moduleInfo.id}`);
        }
      },
    };

    const engine = await dev(
      {
        input: main,
        plugins: [plugin],
        experimental: { devMode: { lazy: true } },
      },
      { dir: path.join(dir, 'dist') },
      {},
    );

    onTestFinished(async () => {
      await engine.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await engine.run();
    await compileChunk(engine, await lazyQuery(readBuild(dir), 'lazy.js'));

    expect(seenTransformIds.some((id) => id.includes('?rolldown-lazy=1'))).toBe(false);
    expect(seenTransformIds.some((id) => id.endsWith('lazy.js'))).toBe(true);
    expect(seenModuleParsedIds.some((id) => id.includes('?rolldown-lazy=1'))).toBe(false);
    expect(seenModuleParsedIds.some((id) => id.endsWith('lazy.js'))).toBe(true);
  },
);

// A virtual module behind a lazy proxy must stay loadable: re-resolving the proxy id
// (`\0virtual:lazy-me?rolldown-lazy=1`) is claimed by the lazy compilation plugin itself
// and never reaches user `resolveId` hooks, which only recognize the bare id.
test(
  'lazy proxy ids resolve without reaching user resolveId hooks',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-lazy-virtual-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    const main = path.join(dir, 'main.js');
    fs.writeFileSync(main, `globalThis.load = () => import('virtual:lazy-me');\n`);

    const virtualId = '\0virtual:lazy-me';
    const lazyProxyId = `${virtualId}?rolldown-lazy=1`;

    const seenByOwner: string[] = [];
    const seenByObserver: string[] = [];
    let resolvedProxyId: string | null | undefined;
    const virtualOwner: Plugin = {
      name: 'virtual-owner',
      resolveId(source) {
        seenByOwner.push(source);
        if (source === 'virtual:lazy-me' || source === virtualId) {
          return virtualId;
        }
      },
      load(id) {
        if (id === virtualId) {
          return `export const value = 1;\n`;
        }
      },
      // The real module is only parsed once the proxy is fetched, so by now the proxy
      // id is a registered lazy entry and re-resolving it must work.
      async moduleParsed(moduleInfo) {
        if (moduleInfo.id === virtualId) {
          resolvedProxyId = (await this.resolve(lazyProxyId))?.id;
        }
      },
    };
    const observer: Plugin = {
      name: 'observe-resolve-ids',
      resolveId(source) {
        seenByObserver.push(source);
      },
    };

    const engine = await dev(
      {
        input: main,
        plugins: [virtualOwner, observer],
        experimental: { devMode: { lazy: true } },
      },
      { dir: path.join(dir, 'dist') },
      {},
    );

    onTestFinished(async () => {
      await engine.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await engine.run();
    const chunk = await compileChunk(engine, await lazyQuery(readBuild(dir), VIRTUAL_STABLE_ID));

    expect(chunk.code).toContain('value');
    expect(resolvedProxyId).toBe(lazyProxyId);
    expect(seenByOwner.some((id) => id.includes('?rolldown-lazy=1'))).toBe(false);
    expect(seenByObserver.some((id) => id.includes('?rolldown-lazy=1'))).toBe(false);
    expect(seenByOwner).toContain('virtual:lazy-me');
    // The fetched proxy resolves the real module with the call that the proxy replaced.
    expect(seenByOwner).not.toContain(virtualId);
  },
);

// The fetched proxy imports the real module by its resolved id. The lazy compilation plugin
// resolves that import with the `resolveId` call that the proxy replaced, so this call creates the
// real module with its `meta` and `moduleSideEffects`, as without lazy compilation.
test.for([
  { name: 'in the build of the first resolution', fullBuildBeforeFetch: false },
  { name: 'after a full build', fullBuildBeforeFetch: true },
])(
  'the real module of a fetched lazy proxy starts with the options of the replaced call ($name)',
  { timeout: TEST_TIMEOUT },
  async ({ fullBuildBeforeFetch }, { onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-lazy-options-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    const main = path.join(dir, 'main.js');
    const lazy = path.join(dir, 'lazy.js');
    fs.writeFileSync(main, `globalThis.load = () => import('./lazy.js');\n`);
    fs.writeFileSync(lazy, `export const value = 'LAZY_VALUE';\n`);

    let calls = 0;
    const seen: { meta: unknown; moduleSideEffects: unknown }[] = [];
    const plugin: Plugin = {
      name: 'lazy-options',
      resolveId(source, importer) {
        if (source === './lazy.js' && importer === main) {
          calls++;
          return { id: lazy, meta: { call: calls }, moduleSideEffects: false };
        }
      },
      transform(_code, id) {
        if (id === lazy) {
          const info = this.getModuleInfo(id)!;
          seen.push({ meta: { ...info.meta }, moduleSideEffects: info.moduleSideEffects });
        }
      },
    };

    const engine = await dev(
      {
        input: main,
        plugins: [plugin],
        experimental: { devMode: { lazy: true } },
      },
      { dir: path.join(dir, 'dist') },
      {},
    );

    onTestFinished(async () => {
      await engine.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await engine.run();
    if (fullBuildBeforeFetch) {
      engine.triggerFullBuild();
      await engine.ensureLatestBuildOutput();
    }
    const query = await lazyQuery(readBuild(dir), 'lazy.js');
    const callsBeforeFetch = calls;
    const chunk = await compileChunk(engine, query);

    expect(chunk.code).toContain('LAZY_VALUE');
    expect(seen[0]).toEqual({ meta: { call: callsBeforeFetch + 1 }, moduleSideEffects: false });
  },
);

// Each build stores the call again, so the fetch runs the call of the latest build.
test(
  'the real module of a fetched lazy proxy starts with the options of the latest replaced call',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-lazy-latest-call-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    const main = path.join(dir, 'main.js');
    const lazy = path.join(dir, 'lazy.js');
    fs.writeFileSync(main, `globalThis.load = () => import('./lazy.js');\n`);
    fs.writeFileSync(lazy, `export const value = 'LAZY_VALUE';\n`);

    const seen: unknown[] = [];
    const plugin: Plugin = {
      name: 'lazy-alias',
      resolveId(source, importer) {
        if (source === './lazy-alias.js' && importer === main) {
          return { id: lazy, meta: { alias: true } };
        }
      },
      transform(_code, id) {
        if (id === lazy) seen.push({ ...this.getModuleInfo(id)!.meta });
      },
    };

    const engine = await dev(
      {
        input: main,
        plugins: [plugin],
        experimental: { devMode: { lazy: true } },
      },
      { dir: path.join(dir, 'dist') },
      {},
    );

    onTestFinished(async () => {
      await engine.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await engine.run();
    // The same proxy, now from another specifier.
    fs.writeFileSync(main, `globalThis.load = () => import('./lazy-alias.js');\n`);
    engine.triggerFullBuild();
    await engine.ensureLatestBuildOutput();
    const chunk = await compileChunk(engine, await lazyQuery(readBuild(dir), 'lazy.js'));

    expect(chunk.code).toContain('LAZY_VALUE');
    expect(seen).toEqual([{ alias: true }]);
  },
);

// The plugin resolves only `virtual:lazy-me`, not the `\0virtual:lazy-me` id that the fetched
// proxy imports.
test(
  'a fetched lazy proxy loads a virtual module whose plugin resolves only the bare id',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-lazy-virtual-bare-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    const main = path.join(dir, 'main.js');
    fs.writeFileSync(main, `globalThis.load = () => import('virtual:lazy-me');\n`);

    const virtualId = '\0virtual:lazy-me';
    const plugin: Plugin = {
      name: 'virtual-owner',
      resolveId(source) {
        if (source === 'virtual:lazy-me') return virtualId;
      },
      load(id) {
        if (id === virtualId) return `export const value = 'VIRTUAL_VALUE';\n`;
      },
    };

    const engine = await dev(
      {
        input: main,
        plugins: [plugin],
        experimental: { devMode: { lazy: true } },
      },
      { dir: path.join(dir, 'dist') },
      {},
    );

    onTestFinished(async () => {
      await engine.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await engine.run();
    const chunk = await compileChunk(engine, await lazyQuery(readBuild(dir), VIRTUAL_STABLE_ID));

    expect(chunk.code).toContain('VIRTUAL_VALUE');
  },
);

// `export * as ns from './dep'` and `export * from './dep'` are the same oxc node,
// separated only by `exported`. Reading it wrong drops the name from the namespace
// object, and the consumer sees `undefined`. The snapshot fixture
// (crates/rolldown/tests/rolldown/topics/hmr/export_star_as) pins the HMR patch; this
// pins the lazy chunk, which reaches the same finalizer by a different route.
test(
  'a lazy chunk keeps the export name of `export * as ns from`',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-lazy-export-star-as-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, 'main.js'), `import('./lazy.js');\n`);
    // reachable only through the namespace re-export
    fs.writeFileSync(path.join(dir, 'dep.js'), `export const value = 'from-dep';\n`);
    fs.writeFileSync(path.join(dir, 'reexport.js'), `export * as NS from './dep.js';\n`);
    fs.writeFileSync(
      path.join(dir, 'lazy.js'),
      `import { NS } from './reexport.js';\nexport const lazy = NS.value;\n`,
    );

    const engine = await dev(
      {
        input: path.join(dir, 'main.js'),
        experimental: { devMode: { lazy: true } },
      },
      { dir: path.join(dir, 'dist') },
      {},
    );

    onTestFinished(async () => {
      await engine.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await engine.run();

    const chunk = await compileChunk(engine, await lazyQuery(readBuild(dir), 'lazy.js'));

    // the namespace object carries `NS`, rather than `dep.js`'s own exports being
    // spread onto the re-exporting module the way `export *` would
    expect(chunk.code).toMatch(/NS:\s*\(\)\s*=>/);
    expect(chunk.code).not.toMatch(/__reExport\(/);
  },
);

// The dev runtime registry only holds modules this build wrapped, so an external can
// only be reached through a real import statement. The three re-export forms used to
// ask the registry for it instead, which yields `{}`; and when the same module also
// imported that external, the two paths emitted one binding name twice, the inner
// `var` shadowing the real import.
test(
  'a lazy chunk imports externals it re-exports instead of asking the registry',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-lazy-reexport-external-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, 'main.js'), `import('./lazy.js');\n`);
    // reachable only through the lazy chunk, and re-exports the external three ways
    // while also importing it — the case that produced the shadowing `var`
    fs.writeFileSync(
      path.join(dir, 'reexport.js'),
      `import { helper } from 'external-dep';\n` +
        `export { helper as reHelper } from 'external-dep';\n` +
        `export * as EXT from 'external-dep';\n` +
        `export * from 'external-dep';\n` +
        `export const helperType = typeof helper;\n`,
    );
    fs.writeFileSync(
      path.join(dir, 'lazy.js'),
      `import { EXT, reHelper, helperType } from './reexport.js';\n` +
        `export const lazy = [EXT, reHelper, helperType];\n`,
    );

    const engine = await dev(
      {
        input: path.join(dir, 'main.js'),
        external: ['external-dep'],
        experimental: { devMode: { lazy: true } },
      },
      { dir: path.join(dir, 'dist') },
      {},
    );

    onTestFinished(async () => {
      await engine.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await engine.run();

    const chunk = await compileChunk(engine, await lazyQuery(readBuild(dir), 'lazy.js'));

    const bindings = externalBindings(chunk.code);
    expect(bindings).toHaveLength(1);
    // the registry never holds an external, so nothing may look it up there
    expect(chunk.code).not.toMatch(/loadExports\("external-dep"\)/);
    // and the one binding is declared once, so the import is not shadowed
    expect(chunk.code).not.toMatch(new RegExp(`var\\s+${escapeRegExp(bindings[0])}\\b`));
  },
);

function externalBindings(code: string): string[] {
  return [...code.matchAll(/import \* as ([\w$]+) from "external-dep"/g)].map((match) => match[1]);
}

function escapeRegExp(text: string): string {
  return text.replace(/[$()*+.?[\\\]^{|}]/g, '\\$&');
}

function semanticErrors(filename: string, code: string) {
  return parseSync(filename, code, { sourceType: 'module', showSemanticErrors: true }).errors;
}

// The same factory code goes into every payload that carries the module. So the name of an
// external's `import * as` binding, which sits at the payload top level, holds the importer's
// id, not the module's place in the payload. Two importers of one external then never declare
// the same name, in whatever payload their factories meet.
test(
  'a lazy chunk gives each importer of an external its own top-level binding',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    // `semanticErrors` must report a duplicate import binding, or the checks below prove nothing
    expect(
      semanticErrors('self-check.js', `import * as a from 'x';\nimport * as a from 'y';\n`),
    ).not.toHaveLength(0);

    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-lazy-external-names-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(
      path.join(dir, 'main.js'),
      `import('./a.js');\nimport('./b.js');\nimport('./c.js');\n`,
    );
    fs.writeFileSync(
      path.join(dir, 'a.js'),
      `import { helper } from 'external-dep';\nexport const a = helper;\n`,
    );
    fs.writeFileSync(
      path.join(dir, 'b.js'),
      `import { helper } from 'external-dep';\nexport const b = helper;\n`,
    );
    fs.writeFileSync(
      path.join(dir, 'c.js'),
      `export { a } from './a.js';\nexport { b } from './b.js';\n`,
    );

    const engine = await dev(
      {
        input: path.join(dir, 'main.js'),
        external: ['external-dep'],
        experimental: { devMode: { lazy: true } },
      },
      { dir: path.join(dir, 'dist') },
      {},
    );

    onTestFinished(async () => {
      await engine.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await engine.run();

    // each query holds none of the routes, so each payload carries its whole route
    const code = readBuild(dir);
    const compile = async (name: string) => compileChunk(engine, await lazyQuery(code, name));
    const chunkA = await compile('a.js');
    const chunkB = await compile('b.js');
    const chunkC = await compile('c.js');

    // a.js and b.js both sit first in their own payloads, and still get different names
    const [bindingA] = externalBindings(chunkA.code);
    const [bindingB] = externalBindings(chunkB.code);
    expect(bindingA).not.toBe(bindingB);
    // c.js's payload carries a.js and b.js again, with the same names
    expect(externalBindings(chunkC.code).sort()).toEqual([bindingA, bindingB].sort());
    expect(semanticErrors('c.js', chunkC.code)).toEqual([]);
  },
);

// Every module registers an exports holder with the runtime, and `loadExports` reads
// `exports` straight off that holder. A module with no exports is no exception: it has to
// register `{ exports: {} }`, matching what a normal build gives such a module. Registering a
// bare `{}` reads back as `undefined`, and every consumer of it then breaks — `__toCommonJS`
// throws "Cannot convert undefined or null to object".
test(
  'a module without exports still registers an empty exports object',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-lazy-require-unused-${uniqueId}`);
    const pkg = path.join(dir, 'pkg');
    fs.mkdirSync(pkg, { recursive: true });
    fs.writeFileSync(path.join(dir, 'main.js'), `import('./pkg/cjs.js');\n`);
    // A `package.json` without `type` makes the files below CommonJS. That is what stops a
    // module with no exports from registering an exports object at all, which is the state
    // the bug needed.
    fs.writeFileSync(path.join(pkg, 'package.json'), `{ "name": "pkg", "main": "cjs.js" }\n`);
    fs.writeFileSync(
      path.join(pkg, 'cjs.js'),
      `require('./side-effect.js');\n\nmodule.exports = 'loaded';\n`,
    );
    fs.writeFileSync(path.join(pkg, 'side-effect.js'), `globalThis.sideEffectRan = true;\n`);

    const engine = await dev(
      {
        input: path.join(dir, 'main.js'),
        experimental: { devMode: { lazy: true, implement: '' } },
      },
      { dir: path.join(dir, 'dist'), format: 'esm' },
      {},
    );

    onTestFinished(async () => {
      await engine.close();
      delete (globalThis as Record<string, unknown>).sideEffectRan;
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await engine.run();
    const chunk = await compileChunk(engine, await lazyQuery(readBuild(dir), 'cjs.js'));

    // `require('./side-effect.js')` is a statement, so its result is thrown away and only the
    // init call is emitted — the same shape a normal build produces for an unused require.
    expect(chunk.code).toMatch(/initModule\("[^"]*side-effect\.js"\)/);
    expect(chunk.code).not.toMatch(/loadExports\("[^"]*side-effect\.js"\)/);

    const runtime = await createDevRuntime();
    runInRuntime(chunk.code, runtime);

    // The registry is keyed by the stable id, which is relative to the process cwd.
    const stableId = (suffix: string) =>
      new RegExp(`registerFactory\\("([^"]*${suffix})"`).exec(chunk.code)![1];

    // A lazy chunk only registers factories — running the module it was compiled for is
    // `requestLazy`'s job, once the chunk has evaluated. Drive it the same way here.
    expect(runtime.initModule(stableId('cjs\\.js'))).toBe('loaded');
    expect((globalThis as Record<string, unknown>).sideEffectRan).toBe(true);

    // The module has no exports, so its exports are empty — never `undefined`.
    expect(runtime.loadExports(stableId('side-effect\\.js'))).toEqual({});
  },
);

// A plugin that emits a chunk from `buildStart` emits it again on every rebuild, including a
// partial one after a lazy compile. The emitted module is unchanged, so that scan never loads
// it, and its module info has to come from the previous build instead (#11318).
test(
  'a chunk emitted in buildStart survives a rebuild after a lazy compile',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-lazy-emit-chunk-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, 'main.js'), `export const load = () => import('./dep.js');\n`);
    fs.writeFileSync(path.join(dir, 'dep.js'), `export const value = 1;\n`);
    fs.writeFileSync(path.join(dir, 'emitted.js'), `export default { a: 1 };\n`);

    const outputFileNames: string[][] = [];
    const engine = await dev(
      {
        input: path.join(dir, 'main.js'),
        plugins: [
          {
            name: 'emit-chunk-in-build-start',
            buildStart() {
              this.emitFile({
                type: 'chunk',
                id: path.join(dir, 'emitted.js'),
                fileName: 'emitted.js',
              });
            },
          },
        ],
        experimental: { devMode: { lazy: true } },
      },
      { dir: path.join(dir, 'dist'), format: 'esm' },
      {
        onOutput(result) {
          if (!(result instanceof Error)) {
            outputFileNames.push(result.output.map((chunk) => chunk.fileName));
          }
        },
      },
    );

    onTestFinished(async () => {
      await engine.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await engine.run();
    await engine.ensureCurrentBuildFinish();
    const chunk = await compileChunk(engine, await lazyQuery(readBuild(dir), 'dep.js'));
    expect(chunk.code).toMatch(/registerFactory\("[^"]*dep\.js"/);

    // The coordinator handles messages in order, so after this reply it watches `dep.js`.
    await engine.getBundleState();
    // An edit of `dep.js` runs a partial rebuild, which runs `buildStart` again. The watcher can
    // miss an event, so each poll edits the file again.
    let edits = 0;
    await expect
      .poll(
        async () => {
          fs.writeFileSync(path.join(dir, 'dep.js'), `export const value = ${++edits + 1};\n`);
          await engine.ensureLatestBuildOutput();
          return outputFileNames.length;
        },
        { timeout: 20_000, interval: 500 },
      )
      .toBeGreaterThanOrEqual(2);
    expect(outputFileNames.at(-1)).toContain('emitted.js');
  },
);

// rolldown#11489: a lazy compile merges its route into the server's graph and starts no build.
// No page loads a lazy route from the build output, because every lazy `import()` calls
// `requestLazy`. So the build output stays current, and N compiles leave one build.
test('lazy compiles start no build', { timeout: TEST_TIMEOUT }, async ({ onTestFinished }) => {
  const uniqueId = crypto.randomUUID().slice(0, 8);
  const dir = path.join(import.meta.dirname, 'temp', `dev-lazy-no-build-${uniqueId}`);
  fs.mkdirSync(dir, { recursive: true });
  const routes = [0, 1, 2, 3, 4];
  fs.writeFileSync(
    path.join(dir, 'main.js'),
    routes.map((i) => `export const load${i} = () => import('./route-${i}.js');\n`).join(''),
  );
  fs.writeFileSync(path.join(dir, 'shared.js'), `export const shared = 'shared';\n`);
  for (const i of routes) {
    fs.writeFileSync(
      path.join(dir, `route-${i}.js`),
      `import { shared } from './shared.js';\nexport const route = shared + '-route-${i}';\n`,
    );
  }

  let builds = 0;
  const engine = await dev(
    {
      input: path.join(dir, 'main.js'),
      experimental: { devMode: { lazy: true } },
    },
    { dir: path.join(dir, 'dist'), format: 'esm' },
    {
      onOutput(result) {
        if (!(result instanceof Error)) {
          builds++;
        }
      },
    },
  );

  onTestFinished(async () => {
    await engine.close();
    if (!process.env.CI) {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  await engine.run();
  await engine.ensureCurrentBuildFinish();
  const code = readBuild(dir);
  for (const i of routes) {
    const chunk = await compileChunk(engine, await lazyQuery(code, `route-${i}.js`));
    expect(chunk.code).toContain(`-route-${i}`);
  }

  await engine.ensureCurrentBuildFinish();
  expect(builds).toBe(1);
  expect((await engine.getBundleState()).hasStaleOutput).toBe(false);
});

// The first build after a visit takes the fetched route from the graph, so the output holds
// the route's code. The `import()` of the route must still call `requestLazy`: a page gets
// built code only at boot. A reload and then a visit of the route must get factories, or a
// module shared with another route runs twice.
// See internal-docs/lazy-compilation/design.md ("After Boot, Only Factories").
test(
  'a build after a lazy compile keeps the fetched route behind requestLazy',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-lazy-build-after-fetch-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    const mainCode = `export const load = () => import('./route.js');\n`;
    fs.writeFileSync(path.join(dir, 'main.js'), mainCode);
    fs.writeFileSync(path.join(dir, 'route.js'), `export const route = 'route-value';\n`);

    const builds: string[][] = [];
    const engine = await dev(
      {
        input: path.join(dir, 'main.js'),
        experimental: { devMode: { lazy: true } },
      },
      { dir: path.join(dir, 'dist'), format: 'esm' },
      {
        onOutput(result) {
          if (!(result instanceof Error)) {
            builds.push(result.output.map((item) => (item.type === 'chunk' ? item.code : '')));
          }
        },
      },
    );

    onTestFinished(async () => {
      await engine.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await engine.run();
    await engine.ensureCurrentBuildFinish();
    expect(builds.at(-1)!.join('\n')).not.toContain('route-value');
    const chunk = await compileChunk(engine, await lazyQuery(readBuild(dir), 'route.js'));
    expect(chunk.code).toContain('route-value');

    // An edit of `main.js` starts a build. The watcher can miss an event, so each poll edits
    // the file again.
    let edits = 0;
    await expect
      .poll(
        async () => {
          fs.writeFileSync(
            path.join(dir, 'main.js'),
            `${mainCode}export const edit = ${++edits};\n`,
          );
          await engine.ensureLatestBuildOutput();
          return builds.length;
        },
        { timeout: 20_000, interval: 500 },
      )
      .toBeGreaterThanOrEqual(2);

    const output = builds.at(-1)!.join('\n');
    expect(output).toContain('route-value');
    const main = fs.readFileSync(path.join(dir, 'dist', 'main.js'), 'utf8');
    expect(main).toMatch(/requestLazy\("[^"]*route\.js"(, 1)?\)/);
    expect(main).not.toMatch(/import\(\s*["'][^"']*route/);
  },
);

// A lazy boundary must stay lazy even when code splitting is off. Without code splitting the
// finalizer's inline path would rewrite `import()` into an eager `(init_foo(), foo_exports)`,
// pulling the module into the bundle — the opposite of lazy compilation. The empty proxy body
// also used to trip the `init_is_noop` assertion, because dev mode adds an HMR header to a
// closure that pass had classified as empty.
test(
  'a lazy import stays lazy when code splitting is disabled',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-lazy-nosplit-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, 'main.js'), `import('./lazy.js').then((m) => m.value);\n`);
    fs.writeFileSync(path.join(dir, 'lazy.js'), `export const value = 'lazy';\n`);

    const engine = await dev(
      { input: path.join(dir, 'main.js'), experimental: { devMode: { lazy: true } } },
      { dir: path.join(dir, 'dist'), codeSplitting: false },
      {},
    );

    onTestFinished(async () => {
      await engine.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await engine.run();

    const code = fs.readFileSync(path.join(dir, 'dist', 'main.js'), 'utf8');
    expect(code).toContain('requestLazy');
    // The lazy module's body must not be in the entry.
    expect(code).not.toContain(`'lazy'`);
  },
);

// The dev server owns the URL, so emitted code carries stable ids only, and a lazy `import()`
// is an ordinary pull that names its target by its session number (`l`). This covers both
// emitters: the full build, and the HMR finalizer for a nested boundary inside a lazy payload.
test(
  'lazy imports pull through DevRuntime#importPatch with the target number',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-lazy-load-chunk-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, 'main.js'), `import('./outer.js');\n`);
    fs.writeFileSync(
      path.join(dir, 'outer.js'),
      `export const loadInner = () => import('./inner.js');\n`,
    );
    fs.writeFileSync(path.join(dir, 'inner.js'), `export const inner = 'inner';\n`);

    const engine = await dev(
      {
        input: path.join(dir, 'main.js'),
        experimental: { devMode: { lazy: true, implement: '' } },
      },
      { dir: path.join(dir, 'dist'), format: 'esm' },
      {},
    );

    onTestFinished(async () => {
      await engine.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await engine.run();
    const fullBuildCode = readBuild(dir);
    const outerChunk = await compileChunk(engine, await lazyQuery(fullBuildCode, 'outer.js'));
    for (const code of [fullBuildCode, outerChunk.code]) {
      expect(code).toContain('requestLazy');
      expect(code).not.toContain('rolldown-lazy');
      expect(code).not.toContain(dir);
    }

    const DevRuntime = await loadDevRuntime();
    const requested: string[] = [];
    class TestDevRuntime extends DevRuntime {
      override async importPatch(url: string) {
        requested.push(url);
        const chunk = await compileChunk(engine, url.slice(url.indexOf('?') + 1));
        runInRuntime(chunk.code, this);
      }
    }
    const runtime = new TestDevRuntime();
    runtime.hooks = { createModuleHotContext: () => ({}), onModuleCacheRemoval: () => {} };
    registerBuildGraph(fullBuildCode, runtime);

    // Run the full build's call, as the browser would. A trailing `1` is the importer's
    // node-mode interop flag.
    const [, outerStableId, nodeMode] = /requestLazy\("([^"]*)"(, 1)?\)/.exec(fullBuildCode)!;
    const outer = await runtime.requestLazy(outerStableId, nodeMode ? 1 : 0);
    const innerNamespace = await outer.loadInner();

    expect(innerNamespace.inner).toBe('inner');
    const lazyTargets = requested.map((url) => {
      expect(url).not.toContain(dir);
      const lazy = new URLSearchParams(url.slice(url.indexOf('?') + 1)).get('l')!;
      return decodeNumSet(lazy).map((num) => numToId(runtime, num));
    });
    expect(lazyTargets).toStrictEqual([[outerStableId], [lazyTargetId(runtime, 'inner.js')]]);
  },
);

// Nothing runs under a proxy id in the client, so the client graph must not route an `import()`
// through the proxy. Otherwise the lazy module's only importer never ran, and the client
// boundary walk stops there: an edit to it is neither hot-updated nor reloaded.
test(
  'the client graph links a lazy import to the real module, not to its proxy',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-lazy-graph-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, 'main.js'), `import('./lazy.js');\n`);
    fs.writeFileSync(path.join(dir, 'lazy.js'), `import './dep.js';\nexport const v = 1;\n`);
    fs.writeFileSync(path.join(dir, 'dep.js'), `export const dep = 1;\n`);

    const engine = await dev(
      {
        input: path.join(dir, 'main.js'),
        experimental: { devMode: { lazy: true, implement: '' } },
      },
      { dir: path.join(dir, 'dist'), format: 'esm' },
      {},
    );

    onTestFinished(async () => {
      await engine.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await engine.run();
    const fullBuildCode = readBuild(dir);
    const lazyChunk = await compileChunk(engine, await lazyQuery(fullBuildCode, 'lazy.js'));

    const runtime = await createDevRuntime();
    registerBuildGraph(fullBuildCode, runtime);
    runInRuntime(lazyChunk.code, runtime);
    const graphIds = [...runtime.staticImports.keys(), ...runtime.importers.keys()];
    expect(graphIds).not.toHaveLength(0);
    for (const id of [...graphIds, ...runtime.dynamicImporters.keys()]) {
      expect(id).not.toContain('rolldown-lazy');
    }

    const stableId = (file: string) =>
      [...runtime.staticImports.keys()].find((id) => id.endsWith(`/${file}`))!;
    expect(runtime.getImporters(stableId('lazy.js'))).toStrictEqual([stableId('main.js')]);
    expect(runtime.getImporters(stableId('dep.js'))).toStrictEqual([stableId('lazy.js')]);
  },
);

// A module that threw while initializing is poisoned, the way the platform treats one:
// `import()` of it rejects every later time. Retrying is not an option — an ESM factory
// registers its module before running the body, so a module that threw stays in the cache and
// re-running `initModule` would hand back its half-initialized exports as success. The browser
// specs only ever click once per page load, so nothing else covers this.
test(
  'a lazy module that throws stays rejected on repeat imports',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-lazy-poison-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, 'main.js'), `import('./boom.js');\n`);
    fs.writeFileSync(
      path.join(dir, 'boom.js'),
      `export const v = 1;\nthrow new Error('boom during lazy init');\n`,
    );

    const engine = await dev(
      {
        input: path.join(dir, 'main.js'),
        experimental: { devMode: { lazy: true, implement: '' } },
      },
      { dir: path.join(dir, 'dist'), format: 'esm' },
      {},
    );

    onTestFinished(async () => {
      await engine.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await engine.run();
    const chunk = await compileChunk(engine, await lazyQuery(readBuild(dir), 'boom.js'));

    const runtime = await createDevRuntime();
    runInRuntime(chunk.code, runtime);

    const realId = /registerFactory\("([^"]*boom\.js)"/.exec(chunk.code)![1];
    // The payload already ran above.
    for (let attempt = 0; attempt < 3; attempt++) {
      await expect(runtime.requestLazy(realId)).rejects.toThrow('boom during lazy init');
    }
  },
);

// The memo must not outlive the module it memoized. `registerModule` installs a fresh exports
// object on every run, so a memo kept across an HMR update would hand a repeat `import()` the
// pre-edit namespace forever — the module is no longer re-executed by any other path that
// could refresh it.
test('a repeat lazy import after an HMR update sees the new exports', async () => {
  const runtime = await createDevRuntime();

  const id = 'lazy.js';
  const registerVersion = (v: string) =>
    runtime.registerFactory(id, (moduleId: string) => {
      runtime.registerModule(moduleId, { exports: runtime.__exportAll({ v: () => v }) });
    });

  runtime.registerGraph({ ids: [id], localCount: 0, edges: [], nums: [0] });
  runtime.importPatch = async () => registerVersion('v1');
  const first = await runtime.requestLazy(id);
  expect(first.v).toBe('v1');

  // What `applyUpdate` does for a patched module: new factory, evict, re-run.
  registerVersion('v2');
  runtime.removeModuleCache(id);
  runtime.initModule(id);

  // A repeat import must see v2, and must not need the server to get it.
  runtime.importPatch = async () => {
    throw new Error('should not re-fetch: the factory is already registered');
  };
  const second = await runtime.requestLazy(id);
  expect(second.v).toBe('v2');
});

// vitejs/vite#23558: `import()` of a CommonJS module must resolve to its ESM view, with
// `default` = `module.exports`, as a production build does.
test(
  'a lazy import of a CommonJS module resolves to its ESM view',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-lazy-cjs-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    // `type: module`: the importer uses node-mode interop, as a production build would.
    fs.writeFileSync(path.join(dir, 'package.json'), `{ "type": "module" }\n`);
    fs.writeFileSync(path.join(dir, 'main.js'), `import('./lazy.cjs').then((m) => m.default);\n`);
    fs.writeFileSync(
      path.join(dir, 'lazy.cjs'),
      `function Lazy() {}\nLazy.version = 'v1';\nmodule.exports = Lazy;\n`,
    );

    const engine = await dev(
      {
        input: path.join(dir, 'main.js'),
        experimental: { devMode: { lazy: true, implement: '' } },
      },
      { dir: path.join(dir, 'dist'), format: 'esm' },
      {},
    );

    onTestFinished(async () => {
      await engine.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await engine.run();

    // The importer passes its interop mode as the second argument.
    const mainCode = fs.readFileSync(path.join(dir, 'dist', 'main.js'), 'utf8');
    expect(mainCode).toMatch(/requestLazy\("[^"]*lazy\.cjs", 1\)/);

    const chunk = await compileChunk(engine, await lazyQuery(mainCode, 'lazy.cjs'));
    // The CommonJS module registers with the flag the runtime reads.
    expect(chunk.code).toContain('registerModule(__rolldown_module_id__, __rolldown_module__, 1)');

    const runtime = await createDevRuntime();
    runInRuntime(chunk.code, runtime);

    const realId = /registerFactory\("([^"]*lazy\.cjs)"/.exec(chunk.code)![1];
    // The payload already ran above.
    const ns = await runtime.requestLazy(realId, 1);
    expect(typeof ns.default).toBe('function');
    expect(ns.default.version).toBe('v1');
    expect(ns.version).toBe('v1');
    expect(runtime.loadExports(realId)).toBe(ns.default);
  },
);

// Resident module (no fetch): the view is still built, per importer, on top of the memoized run.
test('requestLazy applies CommonJS interop per importer, on top of the memoized run', async () => {
  const runtime = await createDevRuntime();

  // `__esModule` + own `default`: node mode keeps `default` = exports, Babel mode unwraps it.
  const cjsExports = { __esModule: true, default: 'inner', named: 'n' };
  const cjsId = 'transpiled.cjs';
  runtime.registerFactory(cjsId, (moduleId: string) => {
    runtime.registerModule(moduleId, { exports: cjsExports }, true);
  });
  runtime.initModule(cjsId);

  runtime.importPatch = async () => {
    throw new Error('should not fetch: the module is resident');
  };
  const nodeView = await runtime.requestLazy(cjsId, 1);
  expect(nodeView.default).toBe(cjsExports);
  expect(nodeView.named).toBe('n');
  const babelView = await runtime.requestLazy(cjsId);
  expect(babelView.default).toBe('inner');
  expect(babelView.named).toBe('n');

  // ESM is handed back as is: no extra `default`, same namespace object each time.
  const esmId = 'esm.js';
  const esmNs = runtime.__exportAll({ value: () => 1 });
  runtime.registerFactory(esmId, (moduleId: string) => {
    runtime.registerModule(moduleId, { exports: esmNs });
  });
  expect(await runtime.requestLazy(esmId, 1)).toBe(esmNs);
  expect(await runtime.requestLazy(esmId)).toBe(esmNs);
});

// After an HMR patch the module is evicted and its factory replaced. A repeat `import()` re-runs
// it, and the new registration must carry the CommonJS flag again.
test('a lazily imported CommonJS module keeps its ESM view after an HMR update', async () => {
  const runtime = await createDevRuntime();

  const id = 'lib.cjs';
  const registerVersion = (version: string) =>
    runtime.registerFactory(id, (moduleId: string) => {
      const module = { exports: { version } };
      runtime.registerModule(moduleId, module, 1);
    });

  runtime.registerGraph({ ids: [id], localCount: 0, edges: [], nums: [0] });
  runtime.importPatch = async () => registerVersion('v1');
  const first = await runtime.requestLazy(id, 1);
  expect(first.default).toEqual({ version: 'v1' });

  // What `applyUpdate` does for a patched module: new factory, evict, re-run.
  registerVersion('v2');
  runtime.removeModuleCache(id);
  runtime.initModule(id);

  runtime.importPatch = async () => {
    throw new Error('should not re-fetch: the factory is already registered');
  };
  const second = await runtime.requestLazy(id, 1);
  expect(second.default).toEqual({ version: 'v2' });
  expect(second.version).toBe('v2');
});

// A lazy compile is the first time a lazy module runs the plugin pipeline, so a plugin
// throwing is the ordinary failure here rather than an exotic one. The engine used to
// answer with a napi error built from `format!("{e:#?}")`, which flattened whatever was
// thrown - message, custom fields, stack - into one string, leaving the caller unable to
// tell which plugin or module was at fault.
test(
  'a plugin error during lazy compile reaches the caller intact',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-lazy-plugin-error-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, 'main.js'), `import('./lazy.js');\n`);
    fs.writeFileSync(path.join(dir, 'lazy.js'), `export const lazy = 'lazy';\n`);

    const plugin = {
      name: 'throws-on-lazy',
      transform(_code: string, id: string) {
        if (id.endsWith('lazy.js')) {
          throw Object.assign(new Error('boom from a user plugin'), { code: 'PLUGIN_BOOM' });
        }
      },
    };

    const engine = await dev(
      {
        input: path.join(dir, 'main.js'),
        plugins: [plugin],
        experimental: { devMode: { lazy: true } },
      },
      { dir: path.join(dir, 'dist') },
      {},
    );

    onTestFinished(async () => {
      await engine.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    // The initial build stubs `lazy.js` with a proxy, so the plugin has not run on it yet.
    await engine.run();

    const query = await lazyQuery(readBuild(dir), 'lazy.js');
    const rejection = await engine.renderPatch(query).then(
      () => null,
      (error: unknown) => error,
    );

    expect(rejection).toBeInstanceOf(Error);
    const [cause] = (rejection as { errors?: unknown[] }).errors ?? [];
    expect(cause).toMatchObject({ message: 'boom from a user plugin' });
    // The wasm binding rebuilds the error from its message alone (see `to_binding_error`).
    if (!isWasiTest) {
      // Which plugin, which hook, which module - and the throw site itself.
      expect(cause).toMatchObject({
        plugin: 'throws-on-lazy',
        hook: 'transform',
        id: path.join(dir, 'lazy.js'),
      });
      expect((cause as Error).stack).toContain('boom from a user plugin');
    }
  },
);

// Any resolution `ctx.resolve` marks external must stay a real dynamic import. Wrapping it
// appends `?rolldown-lazy=1` and routes the import through `requestLazy`, which cannot run an
// id the dev runtime has no factory for (#11273). Config `external` on the raw specifier is not
// listed: it applies before any plugin runs, so the lazy plugin never sees that import.
test.for([
  {
    name: 'a plugin `resolveId` that returns `external: true`',
    specifier: './dep.mjs',
    options: (): InputOptions => ({
      plugins: [
        {
          name: 'external',
          resolveId: (id) => (id === './dep.mjs' ? { id, external: true } : null),
        },
      ],
    }),
  },
  {
    name: 'config `external` on the resolved id',
    specifier: './dep.mjs',
    options: (dir: string): InputOptions => ({ external: [path.join(dir, 'dep.mjs')] }),
  },
  {
    name: 'a Node built-in',
    specifier: 'node:fs',
    options: (): InputOptions => ({ platform: 'node' }),
  },
  {
    name: 'an http(s) URL',
    specifier: 'https://example.com/dep.js',
    options: (): InputOptions => ({}),
  },
])(
  'an external dynamic import is not wrapped in a lazy proxy: $name',
  { timeout: TEST_TIMEOUT },
  async ({ specifier, options }, { onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-lazy-external-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, 'dep.mjs'), `export const value = 1;\n`);
    fs.writeFileSync(
      path.join(dir, 'lazy.mjs'),
      `export const load = () => import(${JSON.stringify(specifier)});\n`,
    );

    let code = '';
    let buildError: Error | undefined;
    const engine = await dev(
      {
        input: path.join(dir, 'lazy.mjs'),
        experimental: { devMode: { lazy: true } },
        ...options(dir),
      },
      { format: 'esm', dir: path.join(dir, 'dist') },
      {
        onOutput(result) {
          if (result instanceof Error) {
            buildError = result;
            return;
          }
          const chunk = result.output.find((item) => item.type === 'chunk' && item.isEntry);
          if (chunk?.type === 'chunk') {
            code = chunk.code;
          }
        },
      },
    );

    onTestFinished(async () => {
      await engine.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await engine.run();
    if (buildError) {
      throw buildError;
    }

    expect(code).toContain(`import(${JSON.stringify(specifier)})`);
    expect(code).not.toContain('rolldown-lazy');
    expect(code).not.toContain('__rolldown_runtime__.requestLazy(');
  },
);
