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

// Regression test for https://github.com/rolldown/rolldown/issues/9365
//
// When the initial build fails, Vite's `waitForInitialBuildFinish` polls
// `ensureCurrentBuildFinish()` in a loop. If the user edits `vite.config.ts`
// during that loop, Vite restarts the server and closes the dev engine, but
// the polling loop is left running in the old environment. The old loop's
// next `ensureCurrentBuildFinish()` call then ran on a closed engine and
// rejected with `Dev engine is closed`. Vite's loop has no `.catch()`, so the
// rejection became an unhandled rejection and crashed the process.
//
// Fix: after `close()`, `ensureCurrentBuildFinish()` is a no-op rather than
// an error — there is no ongoing bundle to wait for.
test(
  'ensureCurrentBuildFinish after close resolves instead of rejecting',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-close-${uniqueId}`);
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
            name: 'force-failure',
            load() {
              // Reproduce the failing-load-hook from the original issue's
              // vite.config.ts so the initial build is in the "failed" state
              // at the moment of close.
              throw new Error('test error');
            },
          },
        ],
      },
      { dir: path.join(dir, 'dist') },
      {},
    );

    // Mirror Vite: kick off run() without awaiting, so the initial build is
    // in flight (and failing) when close() races with the next call.
    engine.run().catch(() => {});

    // Let the build fail at least once before closing.
    await engine.ensureCurrentBuildFinish();

    await engine.close();

    // After close, this call must resolve rather than reject. Vite calls it
    // from a polling loop without a `.catch()` handler — a rejection here
    // surfaces as an unhandled promise rejection that crashes the host.
    await expect(engine.ensureCurrentBuildFinish()).resolves.toBeUndefined();
  },
);
