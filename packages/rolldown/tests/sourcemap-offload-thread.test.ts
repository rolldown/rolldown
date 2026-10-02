import { spawnSync } from 'node:child_process';
import nodePath from 'node:path';
import { fileURLToPath } from 'node:url';

import { isWasiTest } from '@tests/runtime-flavor';
import { expect, test } from 'vitest';

// The scan stage offloads `experimental.nativeMagicString` sourcemaps to a
// dedicated OS thread. That thread is gated on the target being able to spawn
// one, not on the scheduler flavor, so every native flavor -- including
// `ROLLDOWN_RUNTIME=single` -- must offload. Generating the maps inline instead
// runs them on the JavaScript host thread inside the synchronous napi call.
//
// `ROLLDOWN_RUNTIME` is read once, when the binding loads, so each flavor needs
// its own process. The child reports the raw `sendMagicString` return: `null`
// when the channel took the map, a JSON string when it was generated inline.

const testsDir = fileURLToPath(new URL('.', import.meta.url));
const childPath = nodePath.join(testsDir, 'fixtures', 'sourcemap-offload-thread', 'child.mjs');

function runChild(runtime: string | undefined) {
  const env = { ...process.env };
  // The async-runtime CI lane already exports `ROLLDOWN_RUNTIME=single`, so the
  // default-flavor case has to clear it rather than merely leave it unset.
  delete env.ROLLDOWN_RUNTIME;
  if (runtime !== undefined) env.ROLLDOWN_RUNTIME = runtime;

  const child = spawnSync(process.execPath, [childPath], {
    cwd: testsDir,
    encoding: 'utf8',
    env,
    timeout: 25_000,
  });

  expect(child.error).toBeUndefined();
  expect(child.signal).toBeNull();
  expect(child.status, child.stderr || child.stdout).toBe(0);
  return JSON.parse(child.stdout.trim().split('\n').at(-1)!);
}

test.skipIf(isWasiTest).each([
  ['the default flavor', undefined, 'MultiThread'],
  ['ROLLDOWN_RUNTIME=single', 'single', 'CurrentThread'],
])(
  'native magic-string sourcemaps stay on the sourcemap thread under %s',
  { timeout: 60_000 },
  (_label, runtime, expectedFlavor) => {
    const result = runChild(runtime);

    expect(result).toMatchObject({
      flavor: expectedFlavor,
      wasi: false,
      inline: 0,
    });
    expect(result.offloaded).toBe(result.expected);
  },
);
