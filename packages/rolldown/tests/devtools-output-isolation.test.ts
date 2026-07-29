import { spawnSync } from 'node:child_process';
import nodePath from 'node:path';
import { fileURLToPath } from 'node:url';

import { expect, test } from 'vitest';

const testsDir = fileURLToPath(new URL('.', import.meta.url));
const childPath = nodePath.join(testsDir, 'fixtures', 'devtools-output-isolation', 'child.mjs');
const rdLogChildPath = nodePath.join(testsDir, 'fixtures', 'devtools-rd-log', 'child.mjs');
const devtoolsFirstChildPath = nodePath.join(
  testsDir,
  'fixtures',
  'devtools-rd-log',
  'devtools-first.mjs',
);

test(
  'devtools isolates owners and output roots while containing unsafe IDs',
  { timeout: 65_000 },
  () => {
    const child = spawnSync(process.execPath, [childPath], {
      cwd: testsDir,
      encoding: 'utf8',
      env: { ...process.env },
      timeout: 60_000,
    });

    expect(child.error).toBeUndefined();
    expect(child.signal).toBeNull();
    expect(child.status, child.stderr || child.stdout).toBe(0);
    expect(JSON.parse(child.stdout.trim().split('\n').at(-1)!)).toEqual({
      canonicalAliases: process.platform !== 'win32',
      encodedIdBoundaries: true,
      escapedSessionContained: true,
      independentSameKeyOwners: true,
      isolatedOutputRoots: true,
      selfContainedStringRefs: true,
    });
  },
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
