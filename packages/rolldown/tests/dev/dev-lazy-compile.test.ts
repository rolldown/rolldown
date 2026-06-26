import { getDevWatchOptionsForCi } from '@rolldown/test-dev-server';
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
