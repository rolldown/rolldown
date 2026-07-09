import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { access, mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { setTimeout as delay } from 'node:timers/promises';

import { replaceDirectoriesTransactionally } from './stage-wasi-packages.mjs';

const transactionModuleUrl = new URL('./stage-wasi-packages.mjs', import.meta.url).href;
const transactionChildSource = `
const [moduleUrl, replacementsJson, pauseJson] = process.argv.slice(1);
const { replaceDirectoriesTransactionally } = await import(moduleUrl);
const replacements = JSON.parse(replacementsJson);
const pause = JSON.parse(pauseJson);

try {
  await replaceDirectoriesTransactionally(replacements, {
    async afterOperation(phase, index) {
      if (pause && phase === pause.phase && index === pause.index) {
        process.send({ type: 'paused' });
        await new Promise((resolve, reject) => {
          process.once('message', (message) => {
            if (message?.type === 'continue') resolve();
            else reject(new Error('Unexpected parent message'));
          });
          process.once('disconnect', () => reject(new Error('Parent disconnected')));
        });
      }
    },
  });
  process.send({ type: 'done' });
  process.disconnect();
} catch (error) {
  console.error(error?.stack ?? error);
  process.exitCode = 1;
  process.disconnect();
}
`;

async function writeMarker(directory, marker) {
  await mkdir(directory, { recursive: true });
  await writeFile(path.join(directory, 'marker.txt'), marker);
}

async function readMarker(directory) {
  return readFile(path.join(directory, 'marker.txt'), 'utf8');
}

async function assertMissing(candidate) {
  await assert.rejects(access(candidate), { code: 'ENOENT' });
}

async function assertTransactionStateRemoved(packageRoot) {
  await Promise.all([
    assertMissing(path.join(packageRoot, '.stage-wasi-packages.lock')),
    assertMissing(path.join(packageRoot, '.stage-wasi-packages.transaction')),
  ]);
}

function spawnTransaction(replacements, pause) {
  const child = spawn(
    process.execPath,
    [
      '--input-type=module',
      '--eval',
      transactionChildSource,
      transactionModuleUrl,
      JSON.stringify(replacements),
      JSON.stringify(pause ?? null),
    ],
    { stdio: ['ignore', 'ignore', 'pipe', 'ipc'] },
  );
  let stderr = '';
  child.stderr.setEncoding('utf8');
  child.stderr.on('data', (chunk) => {
    stderr += chunk;
  });
  const exit = new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('exit', (code, signal) => resolve({ code, signal }));
  });
  return { child, exit, stderr: () => stderr };
}

function waitForMessage(child, type) {
  return new Promise((resolve, reject) => {
    function onMessage(message) {
      if (message?.type !== type) return;
      cleanup();
      resolve(message);
    }
    function onExit(code, signal) {
      cleanup();
      reject(new Error(`Child exited before sending ${type}: code=${code}, signal=${signal}`));
    }
    function cleanup() {
      child.off('message', onMessage);
      child.off('exit', onExit);
    }
    child.on('message', onMessage);
    child.on('exit', onExit);
  });
}

async function assertChildSucceeded(run) {
  const { code, signal } = await run.exit;
  assert.equal(signal, null, run.stderr());
  assert.equal(code, 0, run.stderr());
}

async function createTransactionFixture(prefix) {
  const root = await mkdtemp(path.join(tmpdir(), prefix));
  const packageRoot = path.join(root, 'npm');
  const destinations = [
    path.join(packageRoot, 'wasm32-wasi'),
    path.join(packageRoot, 'wasm32-wasip1'),
  ];
  await Promise.all([
    writeMarker(destinations[0], 'old-threaded'),
    writeMarker(destinations[1], 'old-threadless'),
  ]);
  return { root, packageRoot, destinations };
}

async function createStagedReplacements(packageRoot, destinations, name, markers) {
  const replacements = destinations.map((destination) => ({
    destination,
    staged: path.join(packageRoot, name, path.basename(destination)),
  }));
  await Promise.all(replacements.map(({ staged }, index) => writeMarker(staged, markers[index])));
  return replacements;
}

test('directory transaction restores every package after failures at each commit phase', async (t) => {
  for (const [phase, index] of [
    ['backup', 0],
    ['install', 0],
    ['backup', 1],
    ['install', 1],
  ]) {
    await t.test(`${phase} ${index}`, async () => {
      const { root, packageRoot, destinations } =
        await createTransactionFixture('stage-wasi-rollback-');
      try {
        const replacements = await createStagedReplacements(packageRoot, destinations, 'staged', [
          'new-threaded',
          'new-threadless',
        ]);

        await assert.rejects(
          replaceDirectoriesTransactionally(replacements, {
            afterOperation(currentPhase, currentIndex) {
              if (currentPhase === phase && currentIndex === index) {
                throw new Error('injected transaction failure');
              }
            },
          }),
          /injected transaction failure/,
        );

        assert.equal(await readMarker(destinations[0]), 'old-threaded');
        assert.equal(await readMarker(destinations[1]), 'old-threadless');
        assert.equal(await readMarker(replacements[0].staged), 'new-threaded');
        assert.equal(await readMarker(replacements[1].staged), 'new-threadless');
        await assertTransactionStateRemoved(packageRoot);
      } finally {
        await rm(root, { force: true, recursive: true });
      }
    });
  }
});

test('directory transactions serialize concurrent processes', async () => {
  const { root, packageRoot, destinations } =
    await createTransactionFixture('stage-wasi-concurrent-');
  let first;
  let second;
  try {
    const firstReplacements = await createStagedReplacements(
      packageRoot,
      destinations,
      'first-staged',
      ['first-threaded', 'first-threadless'],
    );
    const secondReplacements = await createStagedReplacements(
      packageRoot,
      destinations,
      'second-staged',
      ['second-threaded', 'second-threadless'],
    );

    first = spawnTransaction(firstReplacements, { phase: 'backup', index: 0 });
    await waitForMessage(first.child, 'paused');
    await assertMissing(destinations[0]);

    second = spawnTransaction(secondReplacements);
    await delay(100);
    assert.equal(second.child.exitCode, null, second.stderr());
    assert.equal(second.child.signalCode, null, second.stderr());

    first.child.send({ type: 'continue' });
    await Promise.all([assertChildSucceeded(first), assertChildSucceeded(second)]);

    assert.equal(await readMarker(destinations[0]), 'second-threaded');
    assert.equal(await readMarker(destinations[1]), 'second-threadless');
    await assertTransactionStateRemoved(packageRoot);
  } finally {
    for (const run of [first, second]) {
      if (run && run.child.exitCode === null && run.child.signalCode === null) {
        run.child.kill('SIGKILL');
        await run.exit;
      }
    }
    await rm(root, { force: true, recursive: true });
  }
});

test(
  'directory transaction recovers a SIGKILLed owner before the next commit',
  { skip: process.platform === 'win32' },
  async () => {
    const { root, packageRoot, destinations } =
      await createTransactionFixture('stage-wasi-sigkill-');
    try {
      const interruptedReplacements = await createStagedReplacements(
        packageRoot,
        destinations,
        'interrupted-staged',
        ['interrupted-threaded', 'interrupted-threadless'],
      );
      const retryReplacements = await createStagedReplacements(
        packageRoot,
        destinations,
        'retry-staged',
        ['retry-threaded', 'retry-threadless'],
      );

      const interrupted = spawnTransaction(interruptedReplacements, {
        phase: 'backup',
        index: 0,
      });
      await waitForMessage(interrupted.child, 'paused');
      interrupted.child.kill('SIGKILL');
      const { code, signal } = await interrupted.exit;
      assert.equal(code, null);
      assert.equal(signal, 'SIGKILL');

      await assertMissing(destinations[0]);
      await Promise.all([
        access(path.join(packageRoot, '.stage-wasi-packages.lock')),
        access(path.join(packageRoot, '.stage-wasi-packages.transaction')),
      ]);

      await assert.rejects(
        replaceDirectoriesTransactionally(retryReplacements, {
          afterOperation(phase, index) {
            if (phase === 'install' && index === 0) {
              throw new Error('injected retry failure');
            }
          },
        }),
        /injected retry failure/,
      );

      assert.equal(await readMarker(destinations[0]), 'old-threaded');
      assert.equal(await readMarker(destinations[1]), 'old-threadless');
      assert.equal(await readMarker(interruptedReplacements[0].staged), 'interrupted-threaded');
      assert.equal(await readMarker(interruptedReplacements[1].staged), 'interrupted-threadless');
      assert.equal(await readMarker(retryReplacements[0].staged), 'retry-threaded');
      assert.equal(await readMarker(retryReplacements[1].staged), 'retry-threadless');
      await assertTransactionStateRemoved(packageRoot);
    } finally {
      await rm(root, { force: true, recursive: true });
    }
  },
);
