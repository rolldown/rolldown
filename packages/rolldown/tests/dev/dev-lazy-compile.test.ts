import { getDevWatchOptionsForCi } from '@rolldown/test-dev-server';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import type { InputOptions, OutputOptions } from 'rolldown';
import type { DevEngine, DevOptions } from 'rolldown/experimental';
import { dev as _dev } from 'rolldown/experimental';
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
      ...getDevWatchOptionsForCi(),
      ...devOptions.watch,
    },
  });
}

// `compileEntry` (lazy compilation) takes a caller-supplied module id. That id
// is NOT resolved as a filesystem path — it is only a lookup key into the build
// cache. An unknown id (e.g. an attempt to bundle an arbitrary sensitive file)
// must therefore be rejected rather than read from disk. This pins the
// error-path behavior so the gate in `compile_lazy_entry`
// (crates/rolldown/src/hmr/hmr_stage.rs) can't silently regress.
test(
  'compileEntry rejects an unknown module id instead of bundling it',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-lazy-compile-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    const input = path.join(dir, 'main.js');
    fs.writeFileSync(input, 'console.log(1)');

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

    // Run a full build first so the cache is populated. This proves the id is
    // rejected because it is unknown — not merely because the cache is empty.
    await engine.run();

    // An arbitrary id that was never part of the build graph must be rejected.
    // The thrown message is prefixed by the napi binding with
    // "Failed to compile lazy entry: ..." so match on the inner substring.
    await expect(
      engine.compileEntry('/does/not/exist.js?rolldown-lazy=1', 'some-client'),
    ).rejects.toThrow('Lazy entry module not found in cache');
  },
);

// A lazy chunk is pure first-evaluation demand: a module the entry chunk already
// evaluated at top level serves lazy imports from its live exports, so its factory must
// not be re-shipped to a registered client (whose session froze the top-level-evaluated
// map at hello). An unregistered client has no session and must still receive the
// full closure.
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

    const lazyProxyId = `${path.join(dir, 'lazy.js')}?rolldown-lazy=1`;
    const sharedFactory = /registerFactory\("[^"]*shared\.js"/;
    const lazyFactory = /registerFactory\("[^"]*lazy\.js\b[^"]*"/;

    // The hello freezes the top-level-evaluated map into the session: `shared.js` is
    // statically imported by the entry, so its exports are already live.
    await engine.registerClient('registered-client');
    const chunk = await engine.compileEntry(lazyProxyId, 'registered-client');
    expect(chunk.code).toMatch(lazyFactory);
    expect(chunk.code).not.toMatch(sharedFactory);

    // No session → both per-client maps empty → the full closure ships.
    const coldChunk = await engine.compileEntry(lazyProxyId, 'client-without-session');
    expect(coldChunk.code).toMatch(lazyFactory);
    expect(coldChunk.code).toMatch(sharedFactory);
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

  return { lazyPath, lazyProxyId: `${lazyPath}?rolldown-lazy=1`, plugin };
}

/** Line/column of the only `throw` in `code`, as a sourcemap consumer wants it. */
function throwPosition(code: string) {
  const lines = code.split('\n');
  const index = lines.findIndex((line) => line.includes('from lazy'));
  return { line: index + 1, column: lines[index].indexOf('throw') };
}

function inlineSourceMap(code: string) {
  const match = /sourceMappingURL=data:application\/json[^,]+base64,([\w+/=]+)\s*$/.exec(code);
  expect(match, 'the lazy chunk should carry an inline sourcemap').not.toBeNull();
  return JSON.parse(Buffer.from(match![1], 'base64').toString());
}

// A module's sourcemap chain is what maps the rendered output back to the file
// the user wrote; the oxc codegen map is only its last element and on its own
// points at what the plugins produced. A lazy chunk collapses the whole chain,
// the same way a module rendered into a chunk does — otherwise every position in
// a lazily compiled module is off by whatever the transforms shifted, which is
// every stack frame and every devtools jump inside it.
test(
  'lazy chunk sourcemap maps through the plugin sourcemap chain',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-lazy-sourcemap-${uniqueId}`);
    const { lazyPath, lazyProxyId, plugin } = setupShiftedLazyModule(dir);

    const engine = await dev(
      {
        input: path.join(dir, 'main.js'),
        plugins: [plugin],
        experimental: { devMode: { lazy: true } },
      },
      { dir: path.join(dir, 'dist'), sourcemap: 'inline' },
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
    await engine.registerClient('c1');

    const chunk = await engine.compileEntry(lazyProxyId, 'c1');
    const position = await SourceMapConsumer.with(inlineSourceMap(chunk.code), null, (consumer) =>
      consumer.originalPositionFor(throwPosition(chunk.code)),
    );

    // Line 2 of the file on disk, not line 5 of what the plugin handed the bundler.
    expect(position.line).toBe(2);
    expect(path.resolve(path.join(dir, 'dist'), position.source!)).toBe(lazyPath);
  },
);
