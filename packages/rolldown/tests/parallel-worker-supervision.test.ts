import { spawnSync } from 'node:child_process';
import nodePath from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { expect, test } from 'vitest';
import { isWasiTest } from '@tests/runtime-flavor';

const testsDir = fileURLToPath(new URL('.', import.meta.url));
const childPath = nodePath.join(testsDir, 'fixtures', 'parallel-worker-supervision', 'child.mjs');
const bootstrapChildPath = nodePath.join(
  testsDir,
  'fixtures',
  'parallel-worker-bootstrap',
  'child.mjs',
);
const noncloneableBootstrapChildPath = nodePath.join(
  testsDir,
  'fixtures',
  'parallel-worker-bootstrap',
  'noncloneable-child.mjs',
);
const hangingBootstrapChildPath = nodePath.join(
  testsDir,
  'fixtures',
  'parallel-worker-bootstrap',
  'hanging-child.mjs',
);
const preloadSpoofChildPath = nodePath.join(
  testsDir,
  'fixtures',
  'parallel-worker-bootstrap',
  'preload-spoof-child.mjs',
);
const preloadSpoofPath = nodePath.join(
  testsDir,
  'fixtures',
  'parallel-worker-bootstrap',
  'preload-spoof.mjs',
);

test.skipIf(isWasiTest)(
  'parallel workers keep a one-shot process alive through delayed bootstrap',
  {
    timeout: 30_000,
  },
  () => {
    const child = spawnSync(process.execPath, [bootstrapChildPath], {
      cwd: testsDir,
      encoding: 'utf8',
      env: { ...process.env },
      timeout: 25_000,
    });

    expect(child.error).toBeUndefined();
    expect(child.signal).toBeNull();
    expect(child.status, child.stderr || child.stdout).toBe(0);
    expect(child.stdout).toContain('parallel worker bootstrap completed');
  },
);

test.skipIf(isWasiTest)(
  'parallel file workers discard inherited string-input execArgv',
  { timeout: 30_000 },
  () => {
    const child = spawnSync(
      process.execPath,
      [
        '--input-type=module',
        '--eval',
        `import(${JSON.stringify(pathToFileURL(bootstrapChildPath).href)})`,
      ],
      {
        cwd: testsDir,
        encoding: 'utf8',
        env: { ...process.env },
        timeout: 25_000,
      },
    );

    expect(child.error).toBeUndefined();
    expect(child.signal).toBeNull();
    expect(child.status, child.stderr || child.stdout).toBe(0);
    expect(child.stdout).toContain('parallel worker bootstrap completed');
  },
);

test.skipIf(isWasiTest).each([
  [[], 'parallel worker non-cloneable failure reported'],
  [['--disrupt-reporting'], 'parallel worker reporting capability isolated'],
])(
  'parallel bootstrap failure cannot hang when rejection handling warns (%j)',
  { timeout: 30_000 },
  (fixtureArgs, expectedOutput) => {
    const child = spawnSync(
      process.execPath,
      ['--unhandled-rejections=warn', noncloneableBootstrapChildPath, ...fixtureArgs],
      {
        cwd: testsDir,
        encoding: 'utf8',
        env: { ...process.env },
        timeout: 25_000,
      },
    );

    expect(child.error).toBeUndefined();
    expect(child.signal).toBeNull();
    expect(child.status, child.stderr || child.stdout).toBe(0);
    expect(child.stdout).toContain(expectedOutput);
  },
);

test.skipIf(isWasiTest)(
  'a failed parallel bootstrap terminates a sibling whose initializer never settles',
  { timeout: 30_000 },
  () => {
    const child = spawnSync(process.execPath, [hangingBootstrapChildPath], {
      cwd: testsDir,
      encoding: 'utf8',
      env: { ...process.env },
      timeout: 25_000,
    });

    expect(child.error).toBeUndefined();
    expect(child.signal).toBeNull();
    expect(child.status, child.stderr || child.stdout).toBe(0);
    expect(child.stdout).toContain('hanging parallel worker terminated');
  },
);

test.skipIf(isWasiTest).each([
  {
    args: ['--import', preloadSpoofPath, preloadSpoofChildPath],
    env: {},
    source: 'execArgv',
  },
  {
    args: [preloadSpoofChildPath],
    env: {
      NODE_OPTIONS: `--import=${pathToFileURL(preloadSpoofPath).href}`,
    },
    source: 'NODE_OPTIONS',
  },
])(
  'inherited $source preload injection is stripped from parallel workers',
  { timeout: 30_000 },
  ({ args, env }) => {
    const child = spawnSync(process.execPath, args, {
      cwd: testsDir,
      encoding: 'utf8',
      env: { ...process.env, ...env },
      timeout: 25_000,
    });

    expect(child.error).toBeUndefined();
    expect(child.signal).toBeNull();
    expect(child.status, child.stderr || child.stdout).toBe(0);
    expect(child.stdout).toContain('parallel worker preload injection stripped');
  },
);

test.skipIf(isWasiTest).each([
  ['error', 'delayed parallel-plugin worker fault'],
  ['exit', 'Parallel-plugin worker exited unexpectedly (exit code 23)'],
])(
  'a delayed parallel worker %s is retained through retryable shutdown',
  { timeout: 30_000 },
  (mode, expectedMessage) => {
    const child = spawnSync(process.execPath, [childPath, mode], {
      cwd: testsDir,
      encoding: 'utf8',
      env: { ...process.env },
      timeout: 25_000,
    });

    expect(child.error).toBeUndefined();
    expect(child.signal).toBeNull();
    expect(child.status, child.stderr || child.stdout).toBe(0);
    const output = JSON.parse(child.stdout.trim().split('\n').at(-1)!);
    expect(output.firstCloseErrors).toContain(expectedMessage);
    expect(output.terminateCallsAfterRetry).toBe(output.terminateCallsAfterFirstClose);
  },
);
