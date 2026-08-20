import { getDevWatchOptionsForCi } from '@rolldown/test-dev-server';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { setTimeout as sleep } from 'node:timers/promises';
import type { InputOptions, OutputOptions } from 'rolldown';
import type { DevEngine, DevOptions } from 'rolldown/experimental';
import { dev as _dev } from 'rolldown/experimental';
import { isWasiTest } from 'rolldown-tests/utils';
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

// Sibling of the `ensureCurrentBuildFinish` case fixed for #9365
// (see dev-close.test.ts): a close that races an *in-flight* call, rather
// than a call made after close has already returned.
//
// Vite floats `ensureLatestBuildOutput()` without a rejection handler in
// `triggerBundleRegenerationIfStale` (packages/vite/src/node/server/bundledDev.ts).
// When the server closes while that rebuild is still running, the call
// rejects with UNHANDLEABLE_ERROR "channel closed" and the rejection
// becomes an unhandled rejection that fails the host process.
//
// Closing a dev server is normal, so an in-flight `ensureLatestBuildOutput()`
// should resolve on close, the way `wait_for_ongoing_bundle` already does
// (crates/rolldown_dev/src/dev_engine.rs).
test.skipIf(isWasiTest)(
  'in-flight ensureLatestBuildOutput resolves when the engine is closed',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-close-ensure-latest-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    const input = path.join(dir, 'main.js');
    fs.writeFileSync(input, 'console.log(1)');

    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    const engine = await dev(
      {
        input,
        experimental: { devMode: true },
        plugins: [
          {
            name: 'slow-build',
            // Keep the rebuild running long enough that close() lands while
            // it is still in flight.
            async transform(code) {
              await sleep(300);
              return code;
            },
          },
        ],
      },
      { dir: path.join(dir, 'dist') },
      {},
    );

    await engine.run();

    engine.triggerFullBuild();
    const inFlight = engine.ensureLatestBuildOutput();

    // Let the rebuild get started, then close underneath it.
    await sleep(50);
    await engine.close();

    await expect(inFlight).resolves.toBeUndefined();
  },
);
