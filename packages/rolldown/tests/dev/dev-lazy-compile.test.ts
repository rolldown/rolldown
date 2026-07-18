import { getDevWatchOptionsForCi } from '@rolldown/test-dev-server';
import { isSingleThread } from '@tests/runtime-flavor';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import type { InputOptions, OutputOptions } from 'rolldown';
import type { DevEngine, DevOptions } from 'rolldown/experimental';
import { dev as _dev } from 'rolldown/experimental';
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
// Dev mode spawns the BindingDevEngine, which is out of scope for the
// single-thread (CurrentThread) runtime flavor.
test.skipIf(isSingleThread)(
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
// Dev mode spawns the BindingDevEngine, which is out of scope for the
// single-thread (CurrentThread) runtime flavor.
test.skipIf(isSingleThread)(
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
