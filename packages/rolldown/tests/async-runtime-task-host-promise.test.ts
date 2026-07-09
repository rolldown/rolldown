import { spawnSync } from 'node:child_process';
import nodePath from 'node:path';
import { fileURLToPath } from 'node:url';

import { getRuntimeCapabilities } from 'rolldown/experimental';
import { expect, test } from 'vitest';

const capabilities = getRuntimeCapabilities();
const testsDir = fileURLToPath(new URL('.', import.meta.url));
const childPath = nodePath.join(
  testsDir,
  'fixtures',
  'async-runtime-task-host-promise',
  'child.mjs',
);

test.runIf(
  !capabilities.wasi &&
    capabilities.backend === 'shared' &&
    capabilities.flavor === 'CurrentThread',
)(
  'task-host registration rejects a poisoned-Promise callback without invoking it',
  { timeout: 30_000 },
  () => {
    const child = spawnSync(process.execPath, ['--unhandled-rejections=strict', childPath], {
      cwd: testsDir,
      encoding: 'utf8',
      env: { ...process.env },
      timeout: 25_000,
    });

    expect(child.error).toBeUndefined();
    expect(child.signal).toBeNull();
    expect(child.status, child.stderr || child.stdout).toBe(0);
    expect(JSON.parse(child.stdout.trim().split('\n').at(-1)!)).toMatchObject({
      backend: 'shared',
      callbackCalls: 0,
      completed: true,
      constructorGetterCalls: 0,
      flavor: 'CurrentThread',
      registrationError: 'registerCurrentThreadTaskHost does not accept a JavaScript callback',
      taskHostContractVersion: 2,
      unhandled: [],
    });
  },
);
