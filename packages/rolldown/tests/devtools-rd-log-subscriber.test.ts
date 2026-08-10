import { spawnSync } from 'node:child_process';
import nodePath from 'node:path';
import { fileURLToPath } from 'node:url';

import { expect, test } from 'vitest';

// Session-directory encoding, canonical/symlinked output roots, per-file
// StringRefs and per-owner failure isolation are unit-tested where that logic
// lives, in crates/rolldown_devtools/src/writer.rs. Only the process-global
// tracing-subscriber interactions below need a spawned Node process.
const testsDir = fileURLToPath(new URL('.', import.meta.url));
const rdLogChildPath = nodePath.join(testsDir, 'fixtures', 'devtools-rd-log', 'child.mjs');
const devtoolsFirstChildPath = nodePath.join(
  testsDir,
  'fixtures',
  'devtools-rd-log',
  'devtools-first.mjs',
);

test(
  'RD_LOG subscriber re-enables devtools callsites after an untraced build',
  { timeout: 30_000 },
  () => {
    const child = spawnSync(process.execPath, [rdLogChildPath], {
      cwd: testsDir,
      encoding: 'utf8',
      env: {
        ...process.env,
        RD_LOG: 'info',
        RD_LOG_OUTPUT: 'readable',
      },
      timeout: 25_000,
    });

    expect(child.error).toBeUndefined();
    expect(child.signal).toBeNull();
    expect(child.status, child.stderr || child.stdout).toBe(0);
    expect(JSON.parse(child.stdout.trim().split('\n').at(-1)!)).toEqual({
      isolatedOptIn: true,
      rdLogCompatible: true,
      untracedFirstThenTraced: true,
    });
  },
);

test(
  'devtools-first initialization reports that RD_LOG cannot be added later',
  { timeout: 30_000 },
  () => {
    const env = { ...process.env };
    delete env.RD_LOG;
    delete env.RD_LOG_OUTPUT;
    const child = spawnSync(process.execPath, [devtoolsFirstChildPath], {
      cwd: testsDir,
      encoding: 'utf8',
      env,
      timeout: 25_000,
    });

    expect(child.error).toBeUndefined();
    expect(child.signal).toBeNull();
    expect(child.status, child.stderr || child.stdout).toBe(0);
    expect(child.stderr).toContain('cannot add normal `RD_LOG` logging after global installation');
    expect(JSON.parse(child.stdout.trim().split('\n').at(-1)!)).toEqual({
      devtoolsFirst: true,
      rdLogRejected: true,
    });
  },
);
