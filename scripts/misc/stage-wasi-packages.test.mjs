import assert from 'node:assert/strict';
import { execFile, spawn } from 'node:child_process';
import {
  access,
  mkdtemp,
  mkdir,
  readFile,
  readdir,
  rename,
  rm,
  stat,
  symlink,
  utimes,
  writeFile,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';

import {
  acquireStageWasiPackageReclaimGuard,
  copyWasiPackageForStaging,
  ensureWasiPackageDirectories,
  replaceDirectoriesTransactionally,
  withStageWasiPackageLock,
} from './stage-wasi-packages.mjs';

const transactionModuleUrl = new URL('./stage-wasi-packages.mjs', import.meta.url).href;
const execFileAsync = promisify(execFile);
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
const lockCandidateChildSource = `
const [moduleUrl, packageRoot] = process.argv.slice(1);
const { withStageWasiPackageLock } = await import(moduleUrl);

try {
  await withStageWasiPackageLock(
    packageRoot,
    () => {
      throw new Error('Paused lock candidate unexpectedly acquired the canonical lock');
    },
    {
      beforeLockPublish() {
        process.send({ type: 'paused' });
        return new Promise(() => {});
      },
    },
  );
} catch (error) {
  console.error(error?.stack ?? error);
  process.exitCode = 1;
  process.disconnect();
}
`;
const incompleteLockCandidateChildSource = `
const [moduleUrl, packageRoot] = process.argv.slice(1);
const { withStageWasiPackageLock } = await import(moduleUrl);

function waitForParent(type) {
  return new Promise((resolve, reject) => {
    function onMessage(message) {
      if (message?.type !== type) return;
      cleanup();
      resolve();
    }
    function onDisconnect() {
      cleanup();
      reject(new Error('Parent disconnected'));
    }
    function cleanup() {
      process.off('message', onMessage);
      process.off('disconnect', onDisconnect);
    }
    process.on('message', onMessage);
    process.on('disconnect', onDisconnect);
  });
}

let paused = false;
try {
  await withStageWasiPackageLock(
    packageRoot,
    async () => {
      process.send({ type: 'entered' });
      await waitForParent('release');
    },
    {
      async afterLockCandidatePreparationCreate(preparationPath) {
        if (paused) return;
        paused = true;
        process.send({ type: 'preparation-created', preparationPath });
        await waitForParent('continue');
      },
    },
  );
  process.send({ type: 'done' });
  process.disconnect();
} catch (error) {
  console.error(error?.stack ?? error);
  process.exitCode = 1;
  process.disconnect();
}
`;
const canonicalLockOwnerChildSource = `
const [moduleUrl, packageRoot] = process.argv.slice(1);
const { withStageWasiPackageLock } = await import(moduleUrl);

try {
  await withStageWasiPackageLock(packageRoot, async () => {
    process.send({ type: 'entered' });
    await new Promise(() => {});
  });
} catch (error) {
  console.error(error?.stack ?? error);
  process.exitCode = 1;
  process.disconnect();
}
`;
const reclaimGuardOwnerChildSource = `
const [moduleUrl, packageRoot] = process.argv.slice(1);
const { withStageWasiPackageLock } = await import(moduleUrl);

try {
  await withStageWasiPackageLock(
    packageRoot,
    () => {
      throw new Error('Paused reclaim-guard owner unexpectedly acquired the canonical lock');
    },
    {
      afterReclaimGuardTicketPublish(candidatePath) {
        process.send({ type: 'paused', candidatePath });
        return new Promise(() => {});
      },
    },
  );
} catch (error) {
  console.error(error?.stack ?? error);
  process.exitCode = 1;
  process.disconnect();
}
`;
const reclaimGuardTieChildSource = `
const [moduleUrl, packageRoot] = process.argv.slice(1);
const { acquireStageWasiPackageReclaimGuard } = await import(moduleUrl);

function waitForParent(type) {
  return new Promise((resolve, reject) => {
    function onMessage(message) {
      if (message?.type !== type) return;
      cleanup();
      resolve();
    }
    function onDisconnect() {
      cleanup();
      reject(new Error('Parent disconnected'));
    }
    function cleanup() {
      process.off('message', onMessage);
      process.off('disconnect', onDisconnect);
    }
    process.on('message', onMessage);
    process.on('disconnect', onDisconnect);
  });
}

try {
  const release = await acquireStageWasiPackageReclaimGuard(packageRoot, {
    async beforeReclaimGuardTicketPublish({ candidatePath, ticket }) {
      process.send({ type: 'ticket-ready', candidatePath, ticket });
      await waitForParent('publish');
    },
  });
  process.send({ type: 'entered' });
  await waitForParent('release');
  await release();
  process.send({ type: 'done' });
  process.disconnect();
} catch (error) {
  console.error(error?.stack ?? error);
  process.exitCode = 1;
  process.disconnect();
}
`;
const reclaimGuardPhaseChildSource = `
const [moduleUrl, packageRoot, phase] = process.argv.slice(1);
const { acquireStageWasiPackageReclaimGuard } = await import(moduleUrl);

function waitForParent(type) {
  return new Promise((resolve, reject) => {
    function onMessage(message) {
      if (message?.type !== type) return;
      cleanup();
      resolve();
    }
    function onDisconnect() {
      cleanup();
      reject(new Error('Parent disconnected'));
    }
    function cleanup() {
      process.off('message', onMessage);
      process.off('disconnect', onDisconnect);
    }
    process.on('message', onMessage);
    process.on('disconnect', onDisconnect);
  });
}

async function pause(candidatePath) {
  process.send({ type: 'paused', candidatePath, phase });
  await waitForParent('continue');
}

let ownCandidatePath;
try {
  const release = await acquireStageWasiPackageReclaimGuard(packageRoot, {
    async afterReclaimGuardPreparationCreate(preparationPath) {
      if (phase === 'preparation') await pause(preparationPath);
    },
    async afterReclaimGuardCandidateCreate(candidatePath) {
      ownCandidatePath = candidatePath;
      if (phase === 'candidate') await pause(candidatePath);
    },
    async afterReclaimGuardTicketPublish(candidatePath) {
      if (phase === 'ticket') await pause(candidatePath);
    },
    async beforeReclaimGuardAdmission({ candidatePath }) {
      if (phase === 'admission') await pause(candidatePath);
    },
    async beforeReclaimGuardTicketPublish({ candidatePath }) {
      if (phase === 'owner') await pause(candidatePath);
    },
  });
  if (phase === 'holding') {
    await pause(ownCandidatePath);
  }
  process.send({ type: 'entered' });
  await waitForParent('release');
  await release();
  process.send({ type: 'done' });
  process.disconnect();
} catch (error) {
  console.error(error?.stack ?? error);
  process.exitCode = 1;
  process.disconnect();
}
`;
const staleLockRetireChildSource = `
const [moduleUrl, packageRoot] = process.argv.slice(1);
const { withStageWasiPackageLock } = await import(moduleUrl);

try {
  await withStageWasiPackageLock(
    packageRoot,
    () => {
      throw new Error('Interrupted stale-lock reclaimer unexpectedly entered the operation');
    },
    {
      afterStaleLockRetire(retiredPath) {
        process.send({ type: 'paused', retiredPath });
        return new Promise(() => {});
      },
    },
  );
} catch (error) {
  console.error(error?.stack ?? error);
  process.exitCode = 1;
  process.disconnect();
}
`;
const idleChildSource = `
process.send({ type: 'ready' });
setInterval(() => {}, 1_000);
`;
const windowsFileBlockerSource = `
$stream = [System.IO.File]::Open(
  $env:ROLLDOWN_TEST_BLOCKED_FILE,
  [System.IO.FileMode]::Open,
  [System.IO.FileAccess]::Read,
  [System.IO.FileShare]::ReadWrite
)
try {
  [Console]::Out.WriteLine('ready')
  [Console]::Out.Flush()
  [Console]::In.ReadLine() | Out-Null
} finally {
  $stream.Dispose()
}
`;

async function writeMarker(directory, marker) {
  await mkdir(directory, { recursive: true });
  await writeFile(path.join(directory, 'marker.txt'), marker);
}

async function writePackageSkeleton(directory) {
  await mkdir(directory, { recursive: true });
  await Promise.all([
    writeFile(path.join(directory, 'package.json'), '{}\n'),
    writeFile(path.join(directory, 'README.md'), 'package fixture\n'),
  ]);
}

async function readMarker(directory) {
  return readFile(path.join(directory, 'marker.txt'), 'utf8');
}

async function assertMissing(candidate) {
  await assert.rejects(access(candidate), { code: 'ENOENT' });
}

async function createFifo(candidate) {
  await execFileAsync('mkfifo', [candidate]);
}

async function assertTransactionStateRemoved(packageRoot) {
  await Promise.all([
    assertMissing(path.join(packageRoot, '.stage-wasi-packages.lock')),
    assertMissing(path.join(packageRoot, '.stage-wasi-packages.transaction')),
  ]);
  assert.deepEqual(
    (await readdir(packageRoot)).filter(
      (entry) =>
        entry.startsWith('.stage-wasi-packages.lock.candidate.') ||
        entry.startsWith('.stage-wasi-packages.lock.candidate-preparing.') ||
        entry.startsWith('.stage-wasi-packages.lock.reclaim.') ||
        entry.startsWith('.stage-wasi-packages.lock.reclaim-preparing.') ||
        entry.startsWith('.stage-wasi-packages.lock.retired.'),
    ),
    [],
  );
}

async function readCurrentProcessLockOwner(packageRoot) {
  let owner;
  await withStageWasiPackageLock(packageRoot, async () => {
    owner = JSON.parse(
      await readFile(path.join(packageRoot, '.stage-wasi-packages.lock/owner.json'), 'utf8'),
    );
  });
  assert.equal(typeof owner.incarnation, 'string');
  assert.notEqual(owner.incarnation.length, 0);
  return owner;
}

function differentComparableIncarnation(incarnation) {
  assert.match(incarnation, /\d$/);
  return `${incarnation.slice(0, -1)}${incarnation.endsWith('0') ? '1' : '0'}`;
}

function differentIdentityComponent(component) {
  return `${component}-different`;
}

function permissionMode(stats) {
  return stats.mode & 0o777;
}

async function assertResolvesPromptly(promise, removeBlocker) {
  const outcome = await Promise.race([
    promise.then(() => 'resolved'),
    delay(2_000, 'timed-out', { ref: false }),
  ]);
  if (outcome === 'timed-out') await removeBlocker();
  await promise;
  assert.equal(outcome, 'resolved');
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

function spawnPausedLockCandidate(packageRoot) {
  const child = spawn(
    process.execPath,
    ['--input-type=module', '--eval', lockCandidateChildSource, transactionModuleUrl, packageRoot],
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

function spawnIncompleteLockCandidate(packageRoot) {
  const child = spawn(
    process.execPath,
    [
      '--input-type=module',
      '--eval',
      incompleteLockCandidateChildSource,
      transactionModuleUrl,
      packageRoot,
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

function spawnCanonicalLockOwner(packageRoot) {
  const child = spawn(
    process.execPath,
    [
      '--input-type=module',
      '--eval',
      canonicalLockOwnerChildSource,
      transactionModuleUrl,
      packageRoot,
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

function spawnReclaimGuardOwner(packageRoot) {
  const child = spawn(
    process.execPath,
    [
      '--input-type=module',
      '--eval',
      reclaimGuardOwnerChildSource,
      transactionModuleUrl,
      packageRoot,
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

function spawnReclaimGuardTieContender(packageRoot) {
  const child = spawn(
    process.execPath,
    [
      '--input-type=module',
      '--eval',
      reclaimGuardTieChildSource,
      transactionModuleUrl,
      packageRoot,
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

function spawnReclaimGuardPhase(packageRoot, phase) {
  const child = spawn(
    process.execPath,
    [
      '--input-type=module',
      '--eval',
      reclaimGuardPhaseChildSource,
      transactionModuleUrl,
      packageRoot,
      phase,
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

function spawnStaleLockRetireOwner(packageRoot) {
  const child = spawn(
    process.execPath,
    [
      '--input-type=module',
      '--eval',
      staleLockRetireChildSource,
      transactionModuleUrl,
      packageRoot,
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

function spawnIdleChild() {
  const child = spawn(process.execPath, ['--input-type=module', '--eval', idleChildSource], {
    stdio: ['ignore', 'ignore', 'pipe', 'ipc'],
  });
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

async function holdWindowsFileWithoutDeleteSharing(file) {
  const systemRoot = process.env.SystemRoot;
  const powershell =
    typeof systemRoot === 'string' && systemRoot.length > 0
      ? path.join(systemRoot, 'System32', 'WindowsPowerShell', 'v1.0', 'powershell.exe')
      : 'powershell.exe';
  const child = spawn(
    powershell,
    ['-NoLogo', '-NoProfile', '-NonInteractive', '-Command', windowsFileBlockerSource],
    {
      env: { ...process.env, ROLLDOWN_TEST_BLOCKED_FILE: file },
      stdio: ['pipe', 'pipe', 'pipe'],
    },
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
  await new Promise((resolve, reject) => {
    let stdout = '';
    function onData(chunk) {
      stdout += chunk;
      if (!stdout.includes('ready')) return;
      cleanup();
      resolve();
    }
    function onError(error) {
      cleanup();
      reject(error);
    }
    function onExit(code, signal) {
      cleanup();
      reject(
        new Error(
          `Windows file blocker exited before acquiring the handle: code=${code}, signal=${signal}\n${stderr}`,
        ),
      );
    }
    function cleanup() {
      child.stdout.off('data', onData);
      child.off('error', onError);
      child.off('exit', onExit);
    }
    child.stdout.setEncoding('utf8');
    child.stdout.on('data', onData);
    child.on('error', onError);
    child.on('exit', onExit);
  });

  let release;
  return () =>
    (release ??= (async () => {
      child.stdin.end();
      const { code, signal } = await exit;
      assert.equal(signal, null, stderr);
      assert.equal(code, 0, stderr);
    })());
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

async function abruptlyTerminateChild(run) {
  assert.equal(run.child.kill(), true, `Failed to terminate child process:\n${run.stderr()}`);
  const result = await run.exit;
  assert.ok(
    result.code !== 0 || result.signal !== null,
    `Abruptly terminated child exited successfully:\n${run.stderr()}`,
  );
  return result;
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

test('directory transaction recovers crashes after every durable protocol boundary', async (t) => {
  for (const { committed, index, phase } of [
    { committed: false, index: -1, phase: 'journal-root' },
    { committed: false, index: -1, phase: 'backup-root' },
    { committed: false, index: -1, phase: 'journal' },
    { committed: false, index: 0, phase: 'backup' },
    { committed: false, index: 0, phase: 'install' },
    { committed: false, index: 1, phase: 'backup' },
    { committed: false, index: 1, phase: 'install' },
    { committed: true, index: -1, phase: 'commit' },
    { committed: true, index: -1, phase: 'cleanup-backups' },
    { committed: true, index: -1, phase: 'cleanup-state' },
    { committed: true, index: -1, phase: 'cleanup-journal' },
  ]) {
    await t.test(`${phase} ${index}`, async () => {
      const { root, packageRoot, destinations } = await createTransactionFixture(
        'stage-wasi-durable-crash-',
      );
      const replacements = await createStagedReplacements(packageRoot, destinations, 'staged', [
        'new-threaded',
        'new-threadless',
      ]);
      const interrupted = spawnTransaction(replacements, { index, phase });

      try {
        await waitForMessage(interrupted.child, 'paused');
        await abruptlyTerminateChild(interrupted);
        await withStageWasiPackageLock(packageRoot, () => {});

        assert.equal(
          await readMarker(destinations[0]),
          committed ? 'new-threaded' : 'old-threaded',
        );
        assert.equal(
          await readMarker(destinations[1]),
          committed ? 'new-threadless' : 'old-threadless',
        );
        if (committed) {
          await Promise.all(replacements.map(({ staged }) => assertMissing(staged)));
        } else {
          assert.equal(await readMarker(replacements[0].staged), 'new-threaded');
          assert.equal(await readMarker(replacements[1].staged), 'new-threadless');
        }
        await assertTransactionStateRemoved(packageRoot);
      } finally {
        if (interrupted.child.exitCode === null && interrupted.child.signalCode === null) {
          await abruptlyTerminateChild(interrupted);
        }
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
        await abruptlyTerminateChild(run);
      }
    }
    await rm(root, { force: true, recursive: true });
  }
});

test('package lock creates and serializes a missing transaction root', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-missing-root-'));
  const packageRoot = path.join(root, 'npm');
  let enterFirst;
  let releaseFirst;
  const firstEntered = new Promise((resolve) => {
    enterFirst = resolve;
  });
  const firstMayFinish = new Promise((resolve) => {
    releaseFirst = resolve;
  });
  let secondEntered = false;

  try {
    const first = withStageWasiPackageLock(packageRoot, async () => {
      enterFirst();
      await firstMayFinish;
    });
    await firstEntered;

    const second = withStageWasiPackageLock(packageRoot, () => {
      secondEntered = true;
    });
    await delay(100);
    assert.equal(secondEntered, false);

    releaseFirst();
    await Promise.all([first, second]);

    await access(packageRoot);
    await assertTransactionStateRemoved(packageRoot);
  } finally {
    releaseFirst?.();
    await rm(root, { force: true, recursive: true });
  }
});

test(
  'transaction metadata uses explicit cooperative Unix permissions',
  { skip: process.platform === 'win32' },
  async () => {
    const { root, packageRoot, destinations } =
      await createTransactionFixture('stage-wasi-permissions-');
    const replacements = await createStagedReplacements(packageRoot, destinations, 'staged', [
      'new-threaded',
      'new-threadless',
    ]);
    let inspectedJournal = false;

    try {
      await replaceDirectoriesTransactionally(replacements, {
        async afterOperation(phase) {
          if (phase !== 'journal') return;
          inspectedJournal = true;
          const journalRoot = path.join(packageRoot, '.stage-wasi-packages.transaction');
          const [lock, owner, journal, backups, state] = await Promise.all([
            stat(path.join(packageRoot, '.stage-wasi-packages.lock')),
            stat(path.join(packageRoot, '.stage-wasi-packages.lock/owner.json')),
            stat(journalRoot),
            stat(path.join(journalRoot, 'backups')),
            stat(path.join(journalRoot, 'state.json')),
          ]);
          assert.equal(permissionMode(lock), 0o775);
          assert.equal(permissionMode(owner), 0o644);
          assert.equal(permissionMode(journal), 0o775);
          assert.equal(permissionMode(backups), 0o775);
          assert.equal(permissionMode(state), 0o644);
        },
      });

      assert.equal(inspectedJournal, true);
      await assertTransactionStateRemoved(packageRoot);
    } finally {
      await rm(root, { force: true, recursive: true });
    }
  },
);

test(
  'Darwin lock identity uses IOPlatformUUID and kern.bootsessionuuid',
  { skip: process.platform !== 'darwin' },
  async () => {
    const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-darwin-identity-'));
    const packageRoot = path.join(root, 'npm');

    try {
      const owner = await readCurrentProcessLockOwner(packageRoot);
      const [{ stdout: ioreg }, { stdout: bootSessionUuid }] = await Promise.all([
        execFileAsync('/usr/sbin/ioreg', ['-rd1', '-c', 'IOPlatformExpertDevice'], {
          encoding: 'utf8',
        }),
        execFileAsync('/usr/sbin/sysctl', ['-n', 'kern.bootsessionuuid'], {
          encoding: 'utf8',
        }),
      ]);
      const platformUuid = ioreg.match(/"IOPlatformUUID"\s*=\s*"([0-9a-f-]{36})"/i)?.[1];
      assert.ok(platformUuid);
      assert.equal(owner.machineId, `darwin-machine:${platformUuid.toLowerCase()}`);
      assert.equal(owner.bootId, `darwin-boot:${bootSessionUuid.trim().toLowerCase()}`);
      await assertTransactionStateRemoved(packageRoot);
    } finally {
      await rm(root, { force: true, recursive: true });
    }
  },
);

test(
  'package lock retries Windows EPERM when the canonical lock retires before inspection',
  { skip: process.platform !== 'win32' },
  async () => {
    const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-windows-eperm-race-'));
    const packageRoot = path.join(root, 'npm');
    let publishAttempts = 0;
    let publishFailures = 0;

    try {
      await withStageWasiPackageLock(packageRoot, () => {}, {
        async beforeLockPublish() {
          publishAttempts++;
          if (publishAttempts === 1) {
            await mkdir(path.join(packageRoot, '.stage-wasi-packages.lock'));
          }
        },
        async afterLockPublishFailure({ error, lockPath }) {
          publishFailures++;
          assert.equal(error.code, 'EPERM');
          await rm(lockPath, { recursive: true });
        },
      });

      assert.equal(publishAttempts, 2);
      assert.equal(publishFailures, 1);
      await assertTransactionStateRemoved(packageRoot);
    } finally {
      await rm(root, { force: true, recursive: true });
    }
  },
);

test(
  'package lock retries Windows EPERM while retiring a failed publication candidate',
  { skip: process.platform !== 'win32' },
  async () => {
    const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-windows-candidate-retire-eperm-'));
    const packageRoot = path.join(root, 'npm');
    let blockerReleaseStarted = false;
    let publishAttempts = 0;
    let publishFailures = 0;
    let releaseBlocker;
    let scheduledRelease = Promise.resolve();

    try {
      await withStageWasiPackageLock(packageRoot, () => {}, {
        async afterLockCandidateRetire() {
          assert.equal(blockerReleaseStarted, true);
        },
        async afterLockPublishFailure({ error, lockPath }) {
          publishFailures++;
          assert.ok(['EEXIST', 'ENOTEMPTY', 'EPERM'].includes(error.code));
          await rm(lockPath, { recursive: true });
        },
        async beforeLockPublish({ candidateLockPath, lockPath }) {
          publishAttempts++;
          if (publishAttempts !== 1) return;
          await mkdir(lockPath);
          releaseBlocker = await holdWindowsFileWithoutDeleteSharing(
            path.join(candidateLockPath, 'owner.json'),
          );
          scheduledRelease = delay(80).then(async () => {
            blockerReleaseStarted = true;
            await releaseBlocker();
          });
        },
      });
      await scheduledRelease;
      releaseBlocker = undefined;

      assert.equal(publishAttempts, 2);
      assert.equal(publishFailures, 1);
      await assertTransactionStateRemoved(packageRoot);
    } finally {
      await releaseBlocker?.();
      await scheduledRelease;
      await rm(root, { force: true, recursive: true });
    }
  },
);

test(
  'package lock retries Windows EPERM before retiring its canonical lock',
  { skip: process.platform !== 'win32' },
  async () => {
    const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-windows-canonical-retire-eperm-'));
    const packageRoot = path.join(root, 'npm');
    let blockerReleaseStarted = false;
    let releaseBlocker;
    let scheduledRelease = Promise.resolve();

    try {
      await withStageWasiPackageLock(
        packageRoot,
        async (canonicalRoot) => {
          releaseBlocker = await holdWindowsFileWithoutDeleteSharing(
            path.join(canonicalRoot, '.stage-wasi-packages.lock', 'owner.json'),
          );
          scheduledRelease = delay(80).then(async () => {
            blockerReleaseStarted = true;
            await releaseBlocker();
          });
        },
        {
          afterLockRetire() {
            assert.equal(blockerReleaseStarted, true);
          },
        },
      );
      await scheduledRelease;
      releaseBlocker = undefined;
      await assertTransactionStateRemoved(packageRoot);
    } finally {
      await releaseBlocker?.();
      await scheduledRelease;
      await rm(root, { force: true, recursive: true });
    }
  },
);

test(
  'package lock retries Windows EPERM while removing its retired lock',
  { skip: process.platform !== 'win32' },
  async () => {
    const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-windows-retired-eperm-'));
    const packageRoot = path.join(root, 'npm');
    let releaseBlocker;
    let scheduledRelease = Promise.resolve();

    try {
      await withStageWasiPackageLock(packageRoot, () => {}, {
        async afterLockRetire(retiredPath) {
          releaseBlocker = await holdWindowsFileWithoutDeleteSharing(
            path.join(retiredPath, 'owner.json'),
          );
          scheduledRelease = delay(80).then(() => releaseBlocker());
        },
      });
      await scheduledRelease;
      releaseBlocker = undefined;
      await assertTransactionStateRemoved(packageRoot);
    } finally {
      await releaseBlocker?.();
      await scheduledRelease;
      await rm(root, { force: true, recursive: true });
    }
  },
);

test(
  'reclaim guard retries Windows EPERM before retiring its candidate',
  { skip: process.platform !== 'win32' },
  async () => {
    const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-windows-reclaim-retire-eperm-'));
    const packageRoot = path.join(root, 'npm');
    let blockerReleaseStarted = false;
    let releaseBlocker;
    let releaseGuard;
    let scheduledRelease = Promise.resolve();

    try {
      await mkdir(packageRoot);
      releaseGuard = await acquireStageWasiPackageReclaimGuard(packageRoot, {
        afterReclaimGuardRetire() {
          assert.equal(blockerReleaseStarted, true);
        },
        async beforeReclaimGuardRetire(candidatePath) {
          releaseBlocker = await holdWindowsFileWithoutDeleteSharing(
            path.join(candidatePath, 'owner.json'),
          );
          scheduledRelease = delay(80).then(async () => {
            blockerReleaseStarted = true;
            await releaseBlocker();
          });
        },
      });
      await releaseGuard();
      releaseGuard = undefined;
      await scheduledRelease;
      releaseBlocker = undefined;
      await assertTransactionStateRemoved(packageRoot);
    } finally {
      await releaseBlocker?.();
      await scheduledRelease;
      await releaseGuard?.().catch(() => {});
      await rm(root, { force: true, recursive: true });
    }
  },
);

test(
  'reclaim guard retries Windows EPERM while removing its retired candidate',
  { skip: process.platform !== 'win32' },
  async () => {
    const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-windows-reclaim-eperm-'));
    const packageRoot = path.join(root, 'npm');
    let releaseBlocker;
    let releaseGuard;
    let scheduledRelease = Promise.resolve();

    try {
      await mkdir(packageRoot);
      releaseGuard = await acquireStageWasiPackageReclaimGuard(packageRoot, {
        async afterReclaimGuardRetire(retiredPath) {
          releaseBlocker = await holdWindowsFileWithoutDeleteSharing(
            path.join(retiredPath, 'owner.json'),
          );
          scheduledRelease = delay(80).then(() => releaseBlocker());
        },
      });
      await releaseGuard();
      releaseGuard = undefined;
      await scheduledRelease;
      releaseBlocker = undefined;
      await assertTransactionStateRemoved(packageRoot);
    } finally {
      await releaseBlocker?.();
      await scheduledRelease;
      await releaseGuard?.().catch(() => {});
      await rm(root, { force: true, recursive: true });
    }
  },
);

test('package lock preserves its publication error when candidate cleanup also fails', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-lock-publish-error-precedence-'));
  const packageRoot = path.join(root, 'npm');
  const primaryError = new Error('primary lock publication failure');
  const cleanupError = new Error('lock candidate cleanup failure');

  try {
    await assert.rejects(
      withStageWasiPackageLock(packageRoot, () => {}, {
        afterLockCandidateRetire() {
          throw cleanupError;
        },
        beforeLockPublish() {
          throw primaryError;
        },
      }),
      (error) => {
        assert.ok(error instanceof AggregateError);
        assert.equal(error.errors[0], primaryError);
        assert.equal(error.errors[1], cleanupError);
        assert.equal(error.cause, primaryError);
        return true;
      },
    );
    await assertTransactionStateRemoved(packageRoot);
  } finally {
    await rm(root, { force: true, recursive: true });
  }
});

test('package lock aggregates operation and release failures with the operation as cause', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-lock-release-error-cause-'));
  const packageRoot = path.join(root, 'npm');
  const operationError = new Error('primary package operation failure');
  const releaseError = new Error('transaction lock release failure');

  try {
    await assert.rejects(
      withStageWasiPackageLock(
        packageRoot,
        () => {
          throw operationError;
        },
        {
          afterLockRetire() {
            throw releaseError;
          },
        },
      ),
      (error) => {
        assert.ok(error instanceof AggregateError);
        assert.deepEqual(error.errors, [operationError, releaseError]);
        assert.equal(error.cause, operationError);
        return true;
      },
    );
    await assertTransactionStateRemoved(packageRoot);
  } finally {
    await rm(root, { force: true, recursive: true });
  }
});

test('package lock preserves falsy operation and retirement-hook failures', async (t) => {
  for (const { label, options, operation } of [
    {
      label: 'operation',
      operation: () => {
        throw undefined;
      },
      options: {},
    },
    {
      label: 'retirement hook',
      operation: () => {},
      options: {
        afterLockRetire() {
          throw undefined;
        },
      },
    },
  ]) {
    await t.test(label, async () => {
      const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-falsy-error-'));
      const packageRoot = path.join(root, 'npm');
      let rejected = false;

      try {
        try {
          await withStageWasiPackageLock(packageRoot, operation, options);
        } catch (error) {
          rejected = true;
          assert.equal(error, undefined);
        }
        assert.equal(rejected, true);
        await assertTransactionStateRemoved(packageRoot);
      } finally {
        await rm(root, { force: true, recursive: true });
      }
    });
  }
});

test('package lock publishes a complete owner before contenders can acquire it', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-lock-publish-'));
  const packageRoot = path.join(root, 'npm');
  let pauseFirstPublish;
  let resumeFirstPublish;
  const firstPublishPaused = new Promise((resolve) => {
    pauseFirstPublish = resolve;
  });
  const firstMayPublish = new Promise((resolve) => {
    resumeFirstPublish = resolve;
  });
  let releaseSecond;
  const secondMayFinish = new Promise((resolve) => {
    releaseSecond = resolve;
  });
  let firstPublishAttempt = true;
  let firstEntered = false;
  let activeOwners = 0;
  let maximumActiveOwners = 0;
  let first;
  let second;

  try {
    first = withStageWasiPackageLock(
      packageRoot,
      () => {
        firstEntered = true;
        activeOwners++;
        maximumActiveOwners = Math.max(maximumActiveOwners, activeOwners);
        activeOwners--;
      },
      {
        async beforeLockPublish() {
          if (!firstPublishAttempt) return;
          firstPublishAttempt = false;
          pauseFirstPublish();
          await firstMayPublish;
        },
      },
    );
    await firstPublishPaused;

    let notifySecondEntered;
    const secondEntered = new Promise((resolve) => {
      notifySecondEntered = resolve;
    });
    second = withStageWasiPackageLock(packageRoot, async () => {
      activeOwners++;
      maximumActiveOwners = Math.max(maximumActiveOwners, activeOwners);
      notifySecondEntered();
      await secondMayFinish;
      activeOwners--;
    });
    await secondEntered;

    resumeFirstPublish();
    await delay(100);
    assert.equal(firstEntered, false);
    assert.equal(maximumActiveOwners, 1);

    releaseSecond();
    await Promise.all([first, second]);
    assert.equal(firstEntered, true);
    assert.equal(maximumActiveOwners, 1);
    await assertTransactionStateRemoved(packageRoot);
    assert.deepEqual(
      (await readdir(packageRoot)).filter((entry) =>
        entry.startsWith('.stage-wasi-packages.lock.candidate.'),
      ),
      [],
    );
  } finally {
    resumeFirstPublish?.();
    releaseSecond?.();
    await Promise.allSettled([Promise.resolve(first), Promise.resolve(second)]);
    await rm(root, { force: true, recursive: true });
  }
});

test('package lock rejects a canonical legacy v1 owner without reclaiming it', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-legacy-canonical-owner-'));
  const packageRoot = path.join(root, 'npm');
  const lockPath = path.join(packageRoot, '.stage-wasi-packages.lock');
  const legacyOwner = {
    version: 1,
    createdAt: Date.now(),
    pid: 2_147_483_647,
    token: 'legacy-canonical-owner',
  };

  try {
    await mkdir(lockPath, { recursive: true });
    await writeFile(path.join(lockPath, 'owner.json'), `${JSON.stringify(legacyOwner)}\n`);
    let operationRan = false;
    let staleOwnerObserved = false;
    const settled = withStageWasiPackageLock(
      packageRoot,
      () => {
        operationRan = true;
      },
      {
        afterStaleLockObserved() {
          staleOwnerObserved = true;
        },
      },
    ).then(
      () => ({ status: 'fulfilled' }),
      (error) => ({ error, status: 'rejected' }),
    );
    const outcome = await Promise.race([
      settled,
      delay(2_000, { status: 'timed-out' }, { ref: false }),
    ]);
    if (outcome.status === 'timed-out') {
      await rm(lockPath, { force: true, recursive: true });
      await settled;
    }

    assert.equal(outcome.status, 'rejected');
    assert.match(outcome.error.message, /Unsupported legacy WASI package transaction lock owner/);
    assert.equal(operationRan, false);
    assert.equal(staleOwnerObserved, false);
    assert.deepEqual(
      JSON.parse(await readFile(path.join(lockPath, 'owner.json'), 'utf8')),
      legacyOwner,
    );
    assert.deepEqual(await readdir(packageRoot), ['.stage-wasi-packages.lock']);
  } finally {
    await rm(root, { force: true, recursive: true });
  }
});

test('package lock reclaims a live reused PID with a different incarnation promptly', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-reused-canonical-pid-'));
  const packageRoot = path.join(root, 'npm');
  const lockPath = path.join(packageRoot, '.stage-wasi-packages.lock');

  try {
    const currentOwner = await readCurrentProcessLockOwner(packageRoot);
    await mkdir(lockPath);
    await writeFile(
      path.join(lockPath, 'owner.json'),
      `${JSON.stringify({
        ...currentOwner,
        createdAt: Date.now(),
        token: 'reused-canonical-pid',
        incarnation: differentComparableIncarnation(currentOwner.incarnation),
      })}\n`,
    );

    let operationRan = false;
    const recovery = withStageWasiPackageLock(packageRoot, () => {
      operationRan = true;
    });
    await assertResolvesPromptly(recovery, () => rm(lockPath, { force: true, recursive: true }));

    assert.equal(operationRan, true);
    await assertTransactionStateRemoved(packageRoot);
  } finally {
    await rm(root, { force: true, recursive: true });
  }
});

test('package lock fails closed for owners from another machine or PID namespace', async (t) => {
  for (const field of ['machineId', 'pidNamespaceId']) {
    await t.test(field, async () => {
      const root = await mkdtemp(path.join(tmpdir(), `stage-wasi-non-local-${field}-`));
      const packageRoot = path.join(root, 'npm');
      const lockPath = path.join(packageRoot, '.stage-wasi-packages.lock');
      let acquisition;

      try {
        const currentOwner = await readCurrentProcessLockOwner(packageRoot);
        await mkdir(lockPath);
        await writeFile(
          path.join(lockPath, 'owner.json'),
          `${JSON.stringify({
            ...currentOwner,
            createdAt: Date.now(),
            [field]: differentIdentityComponent(currentOwner[field]),
            token: `non-local-${field}`,
          })}\n`,
        );

        let operationRan = false;
        acquisition = withStageWasiPackageLock(packageRoot, () => {
          operationRan = true;
        });
        await delay(100);
        assert.equal(operationRan, false);
        await access(lockPath);

        const retiredPath = `${lockPath}.test-retired`;
        await rename(lockPath, retiredPath);
        await rm(retiredPath, { force: true, recursive: true });
        await acquisition;
        assert.equal(operationRan, true);
        await assertTransactionStateRemoved(packageRoot);
      } finally {
        await rm(lockPath, { force: true, recursive: true });
        await Promise.allSettled([Promise.resolve(acquisition)]);
        await rm(root, { force: true, recursive: true });
      }
    });
  }
});

test('package lock reclaims an owner from a previous boot on the same machine', async (t) => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-previous-boot-'));
  const packageRoot = path.join(root, 'npm');
  const lockPath = path.join(packageRoot, '.stage-wasi-packages.lock');

  try {
    const currentOwner = await readCurrentProcessLockOwner(packageRoot);
    if (currentOwner.bootId === 'unavailable') {
      t.skip('Boot identity is unavailable on this platform');
      return;
    }
    await mkdir(lockPath);
    await writeFile(
      path.join(lockPath, 'owner.json'),
      `${JSON.stringify({
        ...currentOwner,
        bootId: differentIdentityComponent(currentOwner.bootId),
        createdAt: Date.now(),
        token: 'previous-boot',
      })}\n`,
    );

    let operationRan = false;
    const recovery = withStageWasiPackageLock(packageRoot, () => {
      operationRan = true;
    });
    await assertResolvesPromptly(recovery, () => rm(lockPath, { force: true, recursive: true }));

    assert.equal(operationRan, true);
    await assertTransactionStateRemoved(packageRoot);
  } finally {
    await rm(root, { force: true, recursive: true });
  }
});

test('stale lock takeover cannot retire a successor after a delayed observation', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-stale-lock-cas-'));
  const packageRoot = path.join(root, 'npm');
  const staleOwner = spawnCanonicalLockOwner(packageRoot);
  let notifyFirstObserved;
  const firstObserved = new Promise((resolve) => {
    notifyFirstObserved = resolve;
  });
  let resumeFirstReclaimer;
  const firstMayReclaim = new Promise((resolve) => {
    resumeFirstReclaimer = resolve;
  });
  let firstObservation = true;
  let firstEntered = false;
  let notifySecondEntered;
  const secondEntered = new Promise((resolve) => {
    notifySecondEntered = resolve;
  });
  let releaseSecond;
  const secondMayFinish = new Promise((resolve) => {
    releaseSecond = resolve;
  });
  let activeOwners = 0;
  let maximumActiveOwners = 0;
  let first;
  let second;

  try {
    await waitForMessage(staleOwner.child, 'entered');
    await abruptlyTerminateChild(staleOwner);

    first = withStageWasiPackageLock(
      packageRoot,
      () => {
        firstEntered = true;
        activeOwners++;
        maximumActiveOwners = Math.max(maximumActiveOwners, activeOwners);
        activeOwners--;
      },
      {
        async afterStaleLockObserved() {
          if (!firstObservation) return;
          firstObservation = false;
          notifyFirstObserved();
          await firstMayReclaim;
        },
      },
    );
    await firstObserved;

    second = withStageWasiPackageLock(packageRoot, async () => {
      activeOwners++;
      maximumActiveOwners = Math.max(maximumActiveOwners, activeOwners);
      notifySecondEntered();
      await secondMayFinish;
      activeOwners--;
    });
    await secondEntered;

    resumeFirstReclaimer();
    await delay(100);
    assert.equal(firstEntered, false);
    assert.equal(maximumActiveOwners, 1);
    await access(path.join(packageRoot, '.stage-wasi-packages.lock/owner.json'));

    releaseSecond();
    await Promise.all([first, second]);
    assert.equal(firstEntered, true);
    assert.equal(maximumActiveOwners, 1);
    await assertTransactionStateRemoved(packageRoot);
  } finally {
    resumeFirstReclaimer?.();
    releaseSecond?.();
    if (staleOwner.child.exitCode === null && staleOwner.child.signalCode === null) {
      await abruptlyTerminateChild(staleOwner);
    }
    await Promise.allSettled([Promise.resolve(first), Promise.resolve(second)]);
    await rm(root, { force: true, recursive: true });
  }
});

test('stale reclaim guard waits for a live chooser before applying ticket order', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-live-reclaim-chooser-'));
  const packageRoot = path.join(root, 'npm');
  const staleOwner = spawnCanonicalLockOwner(packageRoot);
  let notifyFirstChoosing;
  const firstChoosing = new Promise((resolve) => {
    notifyFirstChoosing = resolve;
  });
  let resumeFirstChoosing;
  const firstMayChoose = new Promise((resolve) => {
    resumeFirstChoosing = resolve;
  });
  let notifySecondEntered;
  const secondEntered = new Promise((resolve) => {
    notifySecondEntered = resolve;
  });
  let releaseSecond;
  const secondMayFinish = new Promise((resolve) => {
    releaseSecond = resolve;
  });
  let firstEntered = false;
  let secondHasEntered = false;
  let activeOwners = 0;
  let maximumActiveOwners = 0;
  let first;
  let second;

  try {
    await waitForMessage(staleOwner.child, 'entered');
    await abruptlyTerminateChild(staleOwner);

    first = withStageWasiPackageLock(
      packageRoot,
      () => {
        firstEntered = true;
        activeOwners++;
        maximumActiveOwners = Math.max(maximumActiveOwners, activeOwners);
        activeOwners--;
      },
      {
        async afterReclaimGuardCandidateCreate(candidatePath) {
          notifyFirstChoosing(candidatePath);
          await firstMayChoose;
        },
      },
    );
    const firstCandidatePath = await firstChoosing;
    const firstCandidateOwner = JSON.parse(
      await readFile(path.join(firstCandidatePath, 'owner.json'), 'utf8'),
    );
    assert.equal(firstCandidateOwner.pid, process.pid);
    await assertMissing(path.join(firstCandidatePath, 'ticket.json'));
    const oldTime = new Date(Date.now() - 10_000);
    await utimes(firstCandidatePath, oldTime, oldTime);

    second = withStageWasiPackageLock(packageRoot, async () => {
      secondHasEntered = true;
      activeOwners++;
      maximumActiveOwners = Math.max(maximumActiveOwners, activeOwners);
      notifySecondEntered();
      await secondMayFinish;
      activeOwners--;
    });
    await delay(100);
    assert.equal(firstEntered, false);
    assert.equal(secondHasEntered, false);

    resumeFirstChoosing();
    await secondEntered;
    assert.equal(firstEntered, false);
    assert.equal(maximumActiveOwners, 1);

    releaseSecond();
    await Promise.all([first, second]);
    assert.equal(firstEntered, true);
    assert.equal(maximumActiveOwners, 1);
    await assertTransactionStateRemoved(packageRoot);
  } finally {
    resumeFirstChoosing?.();
    releaseSecond?.();
    if (staleOwner.child.exitCode === null && staleOwner.child.signalCode === null) {
      await abruptlyTerminateChild(staleOwner);
    }
    await Promise.allSettled([Promise.resolve(first), Promise.resolve(second)]);
    await rm(root, { force: true, recursive: true });
  }
});

test('reclaim guard preserves a live stalled preparation beyond the old grace period', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-live-reclaim-preparation-'));
  const packageRoot = path.join(root, 'npm');
  await mkdir(packageRoot);
  const first = spawnReclaimGuardPhase(packageRoot, 'preparation');
  let releaseSecond;

  try {
    const { candidatePath: preparationPath } = await waitForMessage(first.child, 'paused');
    const oldTime = new Date(Date.now() - 10_000);
    await utimes(preparationPath, oldTime, oldTime);

    releaseSecond = await acquireStageWasiPackageReclaimGuard(packageRoot);
    await access(preparationPath);
    await releaseSecond();
    releaseSecond = undefined;

    const firstEntered = waitForMessage(first.child, 'entered');
    first.child.send({ type: 'continue' });
    await firstEntered;
    const firstDone = waitForMessage(first.child, 'done');
    first.child.send({ type: 'release' });
    await firstDone;
    await assertChildSucceeded(first);

    await assertMissing(preparationPath);
    await assertTransactionStateRemoved(packageRoot);
  } finally {
    if (first.child.exitCode === null && first.child.signalCode === null) {
      await abruptlyTerminateChild(first);
    }
    await releaseSecond?.().catch(() => {});
    await rm(root, { force: true, recursive: true });
  }
});

test('reclaim guard proceeds when its process-incarnation probe is unavailable', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-uncomparable-preparation-'));
  const packageRoot = path.join(root, 'npm');
  await mkdir(packageRoot);
  let notifyPreparationCreated;
  const preparationCreated = new Promise((resolve) => {
    notifyPreparationCreated = resolve;
  });
  let resumeFirstPreparation;
  const firstMayPublish = new Promise((resolve) => {
    resumeFirstPreparation = resolve;
  });
  let firstRelease;
  let secondRelease;
  let publishedOwner;
  let first;

  try {
    first = acquireStageWasiPackageReclaimGuard(packageRoot, {
      async afterReclaimGuardCandidateCreate(candidatePath) {
        publishedOwner = JSON.parse(await readFile(path.join(candidatePath, 'owner.json'), 'utf8'));
      },
      async afterReclaimGuardPreparationCreate(preparationPath) {
        notifyPreparationCreated(preparationPath);
        await firstMayPublish;
      },
      probeCurrentProcessIncarnation() {
        return undefined;
      },
    }).then((release) => {
      firstRelease = release;
    });

    const preparationPath = await preparationCreated;
    assert.match(
      path.basename(preparationPath),
      new RegExp(
        `^\\.stage-wasi-packages\\.lock\\.reclaim-preparing\\.v2\\.${process.pid}\\.` +
          String.raw`[A-Za-z0-9_-]{22}\.[A-Za-z0-9_-]{22}\.[A-Za-z0-9_-]{22}\.unavailable\.`,
      ),
    );

    secondRelease = await acquireStageWasiPackageReclaimGuard(packageRoot);
    await access(preparationPath);
    await secondRelease();
    secondRelease = undefined;

    resumeFirstPreparation();
    await first;
    assert.equal(publishedOwner.pid, process.pid);
    assert.equal(publishedOwner.incarnation, 'unavailable');
    await firstRelease();
    firstRelease = undefined;

    await assertMissing(preparationPath);
    await assertTransactionStateRemoved(packageRoot);
  } finally {
    resumeFirstPreparation?.();
    await Promise.allSettled([Promise.resolve(first)]);
    await secondRelease?.().catch(() => {});
    await firstRelease?.().catch(() => {});
    await rm(root, { force: true, recursive: true });
  }
});

test('reclaim guard orders equal-ticket contenders across processes', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-reclaim-ticket-tie-'));
  const packageRoot = path.join(root, 'npm');
  await mkdir(packageRoot);
  const first = spawnReclaimGuardTieContender(packageRoot);
  let second;

  try {
    const firstReady = await waitForMessage(first.child, 'ticket-ready');
    second = spawnReclaimGuardTieContender(packageRoot);
    const secondReady = await waitForMessage(second.child, 'ticket-ready');
    assert.equal(firstReady.ticket, 1);
    assert.equal(secondReady.ticket, 1);

    const contenders = [
      { ready: firstReady, run: first },
      { ready: secondReady, run: second },
    ].sort((left, right) =>
      path.basename(left.ready.candidatePath) < path.basename(right.ready.candidatePath) ? -1 : 1,
    );
    const [winner, loser] = contenders;
    let loserEntered = false;
    const winnerEntered = waitForMessage(winner.run.child, 'entered');
    const loserEnteredMessage = waitForMessage(loser.run.child, 'entered').then((message) => {
      loserEntered = true;
      return message;
    });

    first.child.send({ type: 'publish' });
    second.child.send({ type: 'publish' });
    await winnerEntered;
    await delay(100);
    assert.equal(loserEntered, false);

    const winnerDone = waitForMessage(winner.run.child, 'done');
    winner.run.child.send({ type: 'release' });
    await winnerDone;
    await assertChildSucceeded(winner.run);

    await loserEnteredMessage;
    const loserDone = waitForMessage(loser.run.child, 'done');
    loser.run.child.send({ type: 'release' });
    await loserDone;
    await assertChildSucceeded(loser.run);

    await assertTransactionStateRemoved(packageRoot);
  } finally {
    for (const run of [first, second]) {
      if (run && run.child.exitCode === null && run.child.signalCode === null) {
        await abruptlyTerminateChild(run);
      }
    }
    await rm(root, { force: true, recursive: true });
  }
});

test('reclaim guard preserves a legacy ownerless candidate until explicit cleanup', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-legacy-reclaim-pid-'));
  const packageRoot = path.join(root, 'npm');
  const unpublishedCandidatePath = path.join(packageRoot, '.legacy-ownerless-reclaim');
  let idle;
  let release;

  try {
    await mkdir(unpublishedCandidatePath, { recursive: true });
    await delay(process.platform === 'win32' ? 50 : 1_100);
    idle = spawnIdleChild();
    await waitForMessage(idle.child, 'ready');
    const legacyCandidatePath = path.join(
      packageRoot,
      `.stage-wasi-packages.lock.reclaim.${idle.child.pid}.legacy-ownerless`,
    );
    await rename(unpublishedCandidatePath, legacyCandidatePath);

    let acquired = false;
    const acquisition = acquireStageWasiPackageReclaimGuard(packageRoot).then((acquiredRelease) => {
      acquired = true;
      release = acquiredRelease;
    });
    await delay(100);
    assert.equal(acquired, false);
    await access(legacyCandidatePath);

    await rm(legacyCandidatePath, { force: true, recursive: true });
    await acquisition;
    await assertMissing(legacyCandidatePath);
    await release();
    release = undefined;
    await assertTransactionStateRemoved(packageRoot);
  } finally {
    if (release) await release().catch(() => {});
    if (idle && idle.child.exitCode === null && idle.child.signalCode === null) {
      await abruptlyTerminateChild(idle);
    }
    await rm(root, { force: true, recursive: true });
  }
});

test('reclaim guard removes a live reused PID candidate with a different incarnation promptly', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-reused-reclaim-pid-'));
  const packageRoot = path.join(root, 'npm');
  const staleToken = 'reused-reclaim-pid';
  const staleCandidatePath = path.join(
    packageRoot,
    `.stage-wasi-packages.lock.reclaim.${process.pid}.${staleToken}`,
  );
  let release;

  try {
    const currentOwner = await readCurrentProcessLockOwner(packageRoot);
    await mkdir(staleCandidatePath);
    await Promise.all([
      writeFile(
        path.join(staleCandidatePath, 'owner.json'),
        `${JSON.stringify({
          ...currentOwner,
          createdAt: Date.now(),
          token: staleToken,
          incarnation: differentComparableIncarnation(currentOwner.incarnation),
        })}\n`,
      ),
      writeFile(
        path.join(staleCandidatePath, 'ticket.json'),
        `${JSON.stringify({ ticket: 1, version: 1 })}\n`,
      ),
    ]);

    let publishedOwner;
    const acquisition = acquireStageWasiPackageReclaimGuard(packageRoot, {
      async afterReclaimGuardTicketPublish(candidatePath) {
        publishedOwner = JSON.parse(await readFile(path.join(candidatePath, 'owner.json'), 'utf8'));
      },
    }).then((acquiredRelease) => {
      release = acquiredRelease;
    });
    await assertResolvesPromptly(acquisition, () =>
      rm(staleCandidatePath, { force: true, recursive: true }),
    );

    assert.equal(publishedOwner.incarnation, currentOwner.incarnation);
    await assertMissing(staleCandidatePath);
    await release();
    release = undefined;
    await assertTransactionStateRemoved(packageRoot);
  } finally {
    if (release) await release().catch(() => {});
    await rm(root, { force: true, recursive: true });
  }
});

test('reclaim guard preserves a legacy ownerless preparation with unknown scope', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-reused-reclaim-preparation-pid-'));
  const packageRoot = path.join(root, 'npm');
  let release;

  try {
    const currentOwner = await readCurrentProcessLockOwner(packageRoot);
    const staleIncarnation = differentComparableIncarnation(currentOwner.incarnation);
    const encodedIncarnation = Buffer.from(staleIncarnation, 'utf8').toString('base64url');
    const preparationPath = path.join(
      packageRoot,
      `.stage-wasi-packages.lock.reclaim-preparing.v1.${process.pid}.${encodedIncarnation}.ownerless`,
    );
    await mkdir(preparationPath, { recursive: true });

    release = await acquireStageWasiPackageReclaimGuard(packageRoot);
    await access(preparationPath);
    await rm(preparationPath, { force: true, recursive: true });
    await release();
    release = undefined;
    await assertTransactionStateRemoved(packageRoot);
  } finally {
    if (release) await release().catch(() => {});
    await rm(root, { force: true, recursive: true });
  }
});

test('reclaim guard preserves its primary error when candidate cleanup also fails', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-reclaim-error-precedence-'));
  const packageRoot = path.join(root, 'npm');
  const primaryError = new Error('primary reclaim-guard failure');
  const cleanupError = new Error('reclaim-guard cleanup failure');

  try {
    await mkdir(packageRoot);
    await assert.rejects(
      acquireStageWasiPackageReclaimGuard(packageRoot, {
        afterReclaimGuardRetire() {
          throw cleanupError;
        },
        beforeReclaimGuardTicketPublish() {
          throw primaryError;
        },
      }),
      (error) => {
        assert.ok(error instanceof AggregateError);
        assert.equal(error.errors[0], primaryError);
        assert.equal(error.errors[1], cleanupError);
        assert.equal(error.cause, primaryError);
        return true;
      },
    );
    await assertTransactionStateRemoved(packageRoot);
  } finally {
    await rm(root, { force: true, recursive: true });
  }
});

test('reclaim guard rejects malformed ticket metadata without removing the owner', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-hostile-ticket-'));
  const packageRoot = path.join(root, 'npm');
  const token = 'hostile-ticket';
  const candidatePath = path.join(
    packageRoot,
    `.stage-wasi-packages.lock.reclaim.${process.pid}.${token}`,
  );

  try {
    const currentOwner = await readCurrentProcessLockOwner(packageRoot);
    await mkdir(candidatePath);
    await Promise.all([
      writeFile(
        path.join(candidatePath, 'owner.json'),
        `${JSON.stringify({
          ...currentOwner,
          createdAt: Date.now(),
          token,
        })}\n`,
      ),
      writeFile(path.join(candidatePath, 'ticket.json'), '{'),
    ]);

    await assert.rejects(
      acquireStageWasiPackageReclaimGuard(packageRoot),
      /WASI package reclaim-guard ticket contains invalid JSON/,
    );
    await access(candidatePath);
    await rm(candidatePath, { force: true, recursive: true });
    await assertTransactionStateRemoved(packageRoot);
  } finally {
    await rm(root, { force: true, recursive: true });
  }
});

test('reclaim guard preserves a live PID candidate with an unknown incarnation format', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-unknown-reclaim-incarnation-'));
  const packageRoot = path.join(root, 'npm');
  const staleToken = 'unknown-reclaim-incarnation';
  const staleCandidatePath = path.join(
    packageRoot,
    `.stage-wasi-packages.lock.reclaim.${process.pid}.${staleToken}`,
  );
  let acquisition;
  let release;

  try {
    const currentOwner = await readCurrentProcessLockOwner(packageRoot);
    await mkdir(staleCandidatePath);
    await Promise.all([
      writeFile(
        path.join(staleCandidatePath, 'owner.json'),
        `${JSON.stringify({
          ...currentOwner,
          createdAt: Date.now(),
          token: staleToken,
          incarnation: `future-v2:${currentOwner.incarnation}`,
        })}\n`,
      ),
      writeFile(
        path.join(staleCandidatePath, 'ticket.json'),
        `${JSON.stringify({ ticket: 1, version: 1 })}\n`,
      ),
    ]);

    let acquired = false;
    acquisition = acquireStageWasiPackageReclaimGuard(packageRoot).then((acquiredRelease) => {
      acquired = true;
      release = acquiredRelease;
    });
    await delay(100);
    assert.equal(acquired, false);
    await access(staleCandidatePath);

    await rm(staleCandidatePath, { force: true, recursive: true });
    await acquisition;
    await release();
    release = undefined;
    await assertTransactionStateRemoved(packageRoot);
  } finally {
    await rm(staleCandidatePath, { force: true, recursive: true });
    await Promise.allSettled([Promise.resolve(acquisition)]);
    if (release) await release().catch(() => {});
    await rm(root, { force: true, recursive: true });
  }
});

test('reclaim guard recovers crashes at every publication phase', async (t) => {
  for (const phase of ['preparation', 'candidate', 'owner', 'ticket', 'holding']) {
    await t.test(phase, async () => {
      const root = await mkdtemp(path.join(tmpdir(), `stage-wasi-reclaim-${phase}-`));
      const packageRoot = path.join(root, 'npm');
      await mkdir(packageRoot);
      const interrupted = spawnReclaimGuardPhase(packageRoot, phase);

      try {
        const { candidatePath } = await waitForMessage(interrupted.child, 'paused');
        await access(candidatePath);
        if (phase === 'preparation') {
          await assertMissing(path.join(candidatePath, 'owner.json'));
        } else {
          await access(path.join(candidatePath, 'owner.json'));
        }
        await abruptlyTerminateChild(interrupted);

        const release = await acquireStageWasiPackageReclaimGuard(packageRoot);
        await release();

        await assertMissing(candidatePath);
        await assertTransactionStateRemoved(packageRoot);
      } finally {
        if (interrupted.child.exitCode === null && interrupted.child.signalCode === null) {
          await abruptlyTerminateChild(interrupted);
        }
        await rm(root, { force: true, recursive: true });
      }
    });
  }
});

test('reclaim guard makes a post-enumeration entrant wait for the admitted owner', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-reclaim-late-entrant-'));
  const packageRoot = path.join(root, 'npm');
  await mkdir(packageRoot);
  const first = spawnReclaimGuardPhase(packageRoot, 'admission');
  let releaseSecond;
  let second;

  try {
    await waitForMessage(first.child, 'paused');
    let secondEntered = false;
    second = acquireStageWasiPackageReclaimGuard(packageRoot).then((release) => {
      secondEntered = true;
      releaseSecond = release;
    });
    await delay(100);
    assert.equal(secondEntered, false);

    const firstEntered = waitForMessage(first.child, 'entered');
    first.child.send({ type: 'continue' });
    await firstEntered;
    await delay(100);
    assert.equal(secondEntered, false);

    const firstDone = waitForMessage(first.child, 'done');
    first.child.send({ type: 'release' });
    await firstDone;
    await assertChildSucceeded(first);

    await second;
    assert.equal(secondEntered, true);
    const release = releaseSecond;
    releaseSecond = undefined;
    await release();
    await assertTransactionStateRemoved(packageRoot);
  } finally {
    if (first.child.exitCode === null && first.child.signalCode === null) {
      await abruptlyTerminateChild(first);
    }
    await Promise.allSettled([Promise.resolve(second)]);
    if (releaseSecond) await releaseSecond().catch(() => {});
    await rm(root, { force: true, recursive: true });
  }
});

test('stale reclaim guard recovers an abruptly terminated owner without residue', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-stale-reclaim-guard-'));
  const packageRoot = path.join(root, 'npm');
  const staleOwner = spawnCanonicalLockOwner(packageRoot);
  let reclaimGuardOwner;

  try {
    await waitForMessage(staleOwner.child, 'entered');
    await abruptlyTerminateChild(staleOwner);

    reclaimGuardOwner = spawnReclaimGuardOwner(packageRoot);
    const { candidatePath } = await waitForMessage(reclaimGuardOwner.child, 'paused');
    assert.match(
      path.basename(candidatePath),
      new RegExp(`^\\.stage-wasi-packages\\.lock\\.reclaim\\.${reclaimGuardOwner.child.pid}\\.`),
    );
    await Promise.all([
      access(path.join(candidatePath, 'owner.json')),
      access(path.join(candidatePath, 'ticket.json')),
    ]);
    await abruptlyTerminateChild(reclaimGuardOwner);

    let operationRan = false;
    await withStageWasiPackageLock(packageRoot, () => {
      operationRan = true;
    });

    assert.equal(operationRan, true);
    await assertTransactionStateRemoved(packageRoot);
    await assertMissing(candidatePath);
  } finally {
    if (staleOwner.child.exitCode === null && staleOwner.child.signalCode === null) {
      await abruptlyTerminateChild(staleOwner);
    }
    if (
      reclaimGuardOwner &&
      reclaimGuardOwner.child.exitCode === null &&
      reclaimGuardOwner.child.signalCode === null
    ) {
      await abruptlyTerminateChild(reclaimGuardOwner);
    }
    await rm(root, { force: true, recursive: true });
  }
});

test('package lock recovers a crash after stale canonical retirement', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-stale-retire-crash-'));
  const packageRoot = path.join(root, 'npm');
  const staleOwner = spawnCanonicalLockOwner(packageRoot);
  let interrupted;

  try {
    await waitForMessage(staleOwner.child, 'entered');
    await abruptlyTerminateChild(staleOwner);

    interrupted = spawnStaleLockRetireOwner(packageRoot);
    const { retiredPath } = await waitForMessage(interrupted.child, 'paused');
    await access(retiredPath);
    await assertMissing(path.join(packageRoot, '.stage-wasi-packages.lock'));
    await abruptlyTerminateChild(interrupted);

    let operationRan = false;
    await withStageWasiPackageLock(packageRoot, () => {
      operationRan = true;
    });

    assert.equal(operationRan, true);
    await assertMissing(retiredPath);
    await assertTransactionStateRemoved(packageRoot);
  } finally {
    if (staleOwner.child.exitCode === null && staleOwner.child.signalCode === null) {
      await abruptlyTerminateChild(staleOwner);
    }
    if (
      interrupted &&
      interrupted.child.exitCode === null &&
      interrupted.child.signalCode === null
    ) {
      await abruptlyTerminateChild(interrupted);
    }
    await rm(root, { force: true, recursive: true });
  }
});

test('retired lock cleanup cannot remove a successor canonical lock', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-lock-retire-race-'));
  const packageRoot = path.join(root, 'npm');
  let notifyFirstRetired;
  const firstRetired = new Promise((resolve) => {
    notifyFirstRetired = resolve;
  });
  let resumeFirstCleanup;
  const firstMayCleanup = new Promise((resolve) => {
    resumeFirstCleanup = resolve;
  });
  let notifySecondEntered;
  const secondEntered = new Promise((resolve) => {
    notifySecondEntered = resolve;
  });
  let releaseSecond;
  const secondMayFinish = new Promise((resolve) => {
    releaseSecond = resolve;
  });
  let activeOwners = 0;
  let maximumActiveOwners = 0;
  let secondFinished = false;
  let first;
  let second;

  try {
    first = withStageWasiPackageLock(
      packageRoot,
      () => {
        activeOwners++;
        maximumActiveOwners = Math.max(maximumActiveOwners, activeOwners);
        activeOwners--;
      },
      {
        async afterLockRetire(retiredPath) {
          notifyFirstRetired(retiredPath);
          await firstMayCleanup;
        },
      },
    );
    const retiredPath = await firstRetired;
    await access(retiredPath);
    await assertMissing(path.join(packageRoot, '.stage-wasi-packages.lock'));

    second = withStageWasiPackageLock(packageRoot, async () => {
      activeOwners++;
      maximumActiveOwners = Math.max(maximumActiveOwners, activeOwners);
      notifySecondEntered();
      await secondMayFinish;
      activeOwners--;
    });
    void second.then(
      () => {
        secondFinished = true;
      },
      () => {},
    );
    await secondEntered;
    await assertMissing(retiredPath);

    resumeFirstCleanup();
    await first;
    await access(path.join(packageRoot, '.stage-wasi-packages.lock/owner.json'));
    assert.equal(secondFinished, false);
    assert.equal(maximumActiveOwners, 1);

    releaseSecond();
    await second;
    await assertTransactionStateRemoved(packageRoot);
    assert.deepEqual(
      (await readdir(packageRoot)).filter((entry) =>
        entry.startsWith('.stage-wasi-packages.lock.retired.'),
      ),
      [],
    );
  } finally {
    resumeFirstCleanup?.();
    releaseSecond?.();
    await Promise.allSettled([Promise.resolve(first), Promise.resolve(second)]);
    await rm(root, { force: true, recursive: true });
  }
});

test('package lock removes abandoned retired directories while holding canonical ownership', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-retired-cleanup-'));
  const packageRoot = path.join(root, 'npm');
  const retiredPath = path.join(packageRoot, '.stage-wasi-packages.lock.retired.abandoned-fixture');
  await writeMarker(retiredPath, 'abandoned');

  try {
    await withStageWasiPackageLock(packageRoot, async () => {
      await assertMissing(retiredPath);
    });
    await assertTransactionStateRemoved(packageRoot);
    assert.deepEqual(
      (await readdir(packageRoot)).filter((entry) =>
        entry.startsWith('.stage-wasi-packages.lock.retired.'),
      ),
      [],
    );
  } finally {
    await rm(root, { force: true, recursive: true });
  }
});

test('package lock preserves an aged incomplete preparation owned by a live process', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-live-incomplete-lock-'));
  const packageRoot = path.join(root, 'npm');
  const candidate = spawnIncompleteLockCandidate(packageRoot);
  let activeOwners = 0;
  let maximumActiveOwners = 0;

  try {
    const { preparationPath } = await waitForMessage(candidate.child, 'preparation-created');
    assert.match(
      path.basename(preparationPath),
      new RegExp(
        `^\\.stage-wasi-packages\\.lock\\.candidate-preparing\\.v2\\.${candidate.child.pid}\\.` +
          String.raw`(?:unavailable|[A-Za-z0-9_-]{22})\.(?:unavailable|[A-Za-z0-9_-]{22})\.(?:unavailable|[A-Za-z0-9_-]{22})\.(?:unavailable|[A-Za-z0-9_-]{22})\.`,
      ),
    );
    await assertMissing(path.join(preparationPath, 'owner.json'));
    const oldTime = new Date(Date.now() - 10_000);
    await utimes(preparationPath, oldTime, oldTime);

    await withStageWasiPackageLock(packageRoot, async () => {
      activeOwners++;
      maximumActiveOwners = Math.max(maximumActiveOwners, activeOwners);
      await access(preparationPath);
      activeOwners--;
    });
    await access(preparationPath);

    const childEntered = waitForMessage(candidate.child, 'entered');
    candidate.child.send({ type: 'continue' });
    await childEntered;
    activeOwners++;
    maximumActiveOwners = Math.max(maximumActiveOwners, activeOwners);

    const childDone = waitForMessage(candidate.child, 'done');
    candidate.child.send({ type: 'release' });
    await childDone;
    activeOwners--;
    await assertChildSucceeded(candidate);

    assert.equal(maximumActiveOwners, 1);
    await assertTransactionStateRemoved(packageRoot);
    assert.deepEqual(
      (await readdir(packageRoot)).filter((entry) =>
        entry.startsWith('.stage-wasi-packages.lock.candidate.'),
      ),
      [],
    );
  } finally {
    if (candidate.child.exitCode === null && candidate.child.signalCode === null) {
      await abruptlyTerminateChild(candidate);
    }
    await rm(root, { force: true, recursive: true });
  }
});

test('package lock reclaims a terminated preparation before owner initialization', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-lock-preparation-terminated-'));
  const packageRoot = path.join(root, 'npm');
  const interrupted = spawnIncompleteLockCandidate(packageRoot);

  try {
    const { preparationPath } = await waitForMessage(interrupted.child, 'preparation-created');
    await assertMissing(path.join(preparationPath, 'owner.json'));
    await abruptlyTerminateChild(interrupted);

    await withStageWasiPackageLock(packageRoot, async () => {
      await assertMissing(preparationPath);
      assert.deepEqual(
        (await readdir(packageRoot)).filter((entry) =>
          entry.startsWith('.stage-wasi-packages.lock.candidate-preparing.'),
        ),
        [],
      );
    });

    await assertTransactionStateRemoved(packageRoot);
  } finally {
    if (interrupted.child.exitCode === null && interrupted.child.signalCode === null) {
      await abruptlyTerminateChild(interrupted);
    }
    await rm(root, { force: true, recursive: true });
  }
});

test('package lock retries if its preparation disappears before owner initialization', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-missing-lock-candidate-'));
  const packageRoot = path.join(root, 'npm');
  let attempts = 0;

  try {
    await withStageWasiPackageLock(packageRoot, () => {}, {
      async afterLockCandidatePreparationCreate(preparationPath) {
        attempts++;
        if (attempts === 1) {
          await rm(preparationPath, { force: true, recursive: true });
        }
      },
    });

    assert.equal(attempts, 2);
    await assertTransactionStateRemoved(packageRoot);
    assert.deepEqual(
      (await readdir(packageRoot)).filter((entry) =>
        entry.startsWith('.stage-wasi-packages.lock.candidate.'),
      ),
      [],
    );
  } finally {
    await rm(root, { force: true, recursive: true });
  }
});

test('package lock reclaims an abruptly terminated candidate before retry', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-lock-candidate-terminated-'));
  const packageRoot = path.join(root, 'npm');
  const interrupted = spawnPausedLockCandidate(packageRoot);

  try {
    await waitForMessage(interrupted.child, 'paused');
    const candidateNames = (await readdir(packageRoot)).filter((entry) =>
      entry.startsWith('.stage-wasi-packages.lock.candidate.'),
    );
    assert.equal(candidateNames.length, 1);
    const owner = JSON.parse(
      await readFile(path.join(packageRoot, candidateNames[0], 'owner.json'), 'utf8'),
    );
    assert.equal(owner.pid, interrupted.child.pid);

    await abruptlyTerminateChild(interrupted);

    await withStageWasiPackageLock(packageRoot, async () => {
      assert.deepEqual(
        (await readdir(packageRoot)).filter((entry) =>
          entry.startsWith('.stage-wasi-packages.lock.candidate.'),
        ),
        [],
      );
    });

    await assertTransactionStateRemoved(packageRoot);
    assert.deepEqual(
      (await readdir(packageRoot)).filter((entry) =>
        entry.startsWith('.stage-wasi-packages.lock.candidate.'),
      ),
      [],
    );
  } finally {
    if (interrupted.child.exitCode === null && interrupted.child.signalCode === null) {
      await abruptlyTerminateChild(interrupted);
    }
    await rm(root, { force: true, recursive: true });
  }
});

test(
  'package lock rejects a transaction-root symlink',
  { skip: process.platform === 'win32' },
  async () => {
    const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-root-symlink-'));
    const target = path.join(root, 'target');
    const packageRoot = path.join(root, 'npm');
    await mkdir(target);
    await symlink(target, packageRoot, 'dir');

    try {
      await assert.rejects(
        withStageWasiPackageLock(packageRoot, () => {
          assert.fail('symlinked transaction root must not run the operation');
        }),
        /WASI package transaction root is not a directory/,
      );
      await assertMissing(path.join(target, '.stage-wasi-packages.lock'));
    } finally {
      await rm(root, { force: true, recursive: true });
    }
  },
);

test('package lock rejects missing, malformed, special, oversized, and symlinked owners', async (t) => {
  for (const fixture of [
    {
      name: 'missing',
      pattern: /WASI package lock owner is missing/,
      setup() {},
    },
    {
      name: 'malformed',
      pattern: /WASI package lock owner contains invalid JSON/,
      setup(ownerPath) {
        return writeFile(ownerPath, '{');
      },
    },
    {
      name: 'invalid-schema',
      pattern: /Invalid WASI package lock owner/,
      setup(ownerPath) {
        return writeFile(ownerPath, '{}\n');
      },
    },
    {
      name: 'directory',
      pattern: /WASI package lock owner is not a regular file/,
      setup(ownerPath) {
        return mkdir(ownerPath);
      },
    },
    {
      name: 'fifo',
      pattern: /WASI package lock owner is not a regular file/,
      setup(ownerPath) {
        return createFifo(ownerPath);
      },
      skip: process.platform === 'win32',
    },
    {
      name: 'oversized',
      pattern: /WASI package lock owner exceeds 16384 bytes/,
      setup(ownerPath) {
        return writeFile(ownerPath, ' '.repeat(16_385));
      },
    },
    {
      name: 'symlink',
      pattern: /WASI package lock owner is not a regular file/,
      async setup(ownerPath, root, currentOwner) {
        const externalOwner = path.join(root, 'external-owner.json');
        await writeFile(externalOwner, `${JSON.stringify(currentOwner)}\n`);
        await symlink(externalOwner, ownerPath);
      },
      skip: process.platform === 'win32',
    },
  ]) {
    await t.test(fixture.name, { skip: fixture.skip }, async () => {
      const root = await mkdtemp(path.join(tmpdir(), `stage-wasi-hostile-owner-${fixture.name}-`));
      const packageRoot = path.join(root, 'npm');
      const lockPath = path.join(packageRoot, '.stage-wasi-packages.lock');
      const ownerPath = path.join(lockPath, 'owner.json');

      try {
        const currentOwner = await readCurrentProcessLockOwner(packageRoot);
        await mkdir(lockPath);
        await fixture.setup(ownerPath, root, currentOwner);
        let operationRan = false;
        await assert.rejects(
          withStageWasiPackageLock(packageRoot, () => {
            operationRan = true;
          }),
          fixture.pattern,
        );

        assert.equal(operationRan, false);
        await access(lockPath);
        assert.deepEqual(
          (await readdir(packageRoot)).filter((entry) =>
            entry.startsWith('.stage-wasi-packages.lock.candidate.'),
          ),
          [],
        );
      } finally {
        await rm(root, { force: true, recursive: true });
      }
    });
  }
});

test('transaction recovery rejects malformed, special, oversized, and symlinked state', async (t) => {
  for (const fixture of [
    {
      name: 'malformed',
      pattern: /WASI package transaction state contains invalid JSON/,
      setup(statePath) {
        return writeFile(statePath, '{');
      },
    },
    {
      name: 'directory',
      pattern: /WASI package transaction state is not a regular file/,
      setup(statePath) {
        return mkdir(statePath);
      },
    },
    {
      name: 'fifo',
      pattern: /WASI package transaction state is not a regular file/,
      setup(statePath) {
        return createFifo(statePath);
      },
      skip: process.platform === 'win32',
    },
    {
      name: 'oversized',
      pattern: /WASI package transaction state exceeds 1048576 bytes/,
      setup(statePath) {
        return writeFile(statePath, ' '.repeat(1_048_577));
      },
    },
    {
      name: 'symlink',
      pattern: /WASI package transaction state is not a regular file/,
      async setup(statePath, root) {
        const externalState = path.join(root, 'external-state.json');
        await writeFile(externalState, '{}\n');
        await symlink(externalState, statePath);
      },
      skip: process.platform === 'win32',
    },
  ]) {
    await t.test(fixture.name, { skip: fixture.skip }, async () => {
      const root = await mkdtemp(path.join(tmpdir(), `stage-wasi-hostile-state-${fixture.name}-`));
      const packageRoot = path.join(root, 'npm');
      const journalRoot = path.join(packageRoot, '.stage-wasi-packages.transaction');
      const statePath = path.join(journalRoot, 'state.json');

      try {
        await mkdir(path.join(journalRoot, 'backups'), { recursive: true });
        await fixture.setup(statePath, root);
        let operationRan = false;
        await assert.rejects(
          withStageWasiPackageLock(packageRoot, () => {
            operationRan = true;
          }),
          fixture.pattern,
        );

        assert.equal(operationRan, false);
        await access(journalRoot);
        await assertMissing(path.join(packageRoot, '.stage-wasi-packages.lock'));
      } finally {
        await rm(root, { force: true, recursive: true });
      }
    });
  }
});

test('package bootstrap creates only missing WASI package directories', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-bootstrap-'));
  const packageRoot = path.join(root, 'npm');
  const existingPackage = path.join(packageRoot, 'wasm32-wasi');
  const missingPackage = path.join(packageRoot, 'wasm32-wasip1');
  await writeMarker(existingPackage, 'existing');

  try {
    await ensureWasiPackageDirectories({
      packageNames: ['wasm32-wasi', 'wasm32-wasip1'],
      packageRoot,
      rolldownRoot: root,
      async createNpmDirs(bootstrapRoot) {
        await Promise.all([
          writeMarker(path.join(bootstrapRoot, 'wasm32-wasi'), 'generated-threaded'),
          writeMarker(path.join(bootstrapRoot, 'wasm32-wasip1'), 'generated-threadless'),
        ]);
      },
    });

    assert.equal(await readMarker(existingPackage), 'existing');
    assert.equal(await readMarker(missingPackage), 'generated-threadless');
    assert.deepEqual((await readdir(packageRoot)).sort(), ['wasm32-wasi', 'wasm32-wasip1']);
  } finally {
    await rm(root, { force: true, recursive: true });
  }
});

test('package bootstrap uses the real NapiCli generator without leaving transaction residue', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-real-bootstrap-'));
  const packageRoot = path.join(root, 'npm');
  const rolldownRoot = fileURLToPath(new URL('../../packages/rolldown/', import.meta.url));
  const originalFetch = globalThis.fetch;
  const originalRegistry = process.env.npm_config_registry;
  const fetchRequests = [];
  await mkdir(packageRoot);

  process.env.npm_config_registry = 'https://registry.example.invalid/';
  globalThis.fetch = async (input) => {
    fetchRequests.push(String(input));
    return new Response(JSON.stringify({ 'dist-tags': { latest: '9.9.9' } }), {
      headers: { 'content-type': 'application/json' },
      status: 200,
    });
  };

  try {
    await ensureWasiPackageDirectories({
      packageNames: ['wasm32-wasi', 'wasm32-wasip1'],
      packageRoot,
      rolldownRoot,
    });

    const threadedManifest = JSON.parse(
      await readFile(path.join(packageRoot, 'wasm32-wasi/package.json'), 'utf8'),
    );
    const threadlessManifest = JSON.parse(
      await readFile(path.join(packageRoot, 'wasm32-wasip1/package.json'), 'utf8'),
    );
    assert.equal(threadedManifest.name, '@rolldown/binding-wasm32-wasi');
    assert.equal(threadlessManifest.name, '@rolldown/binding-wasm32-wasip1');
    assert.equal(threadedManifest.dependencies['@napi-rs/wasm-runtime'], '^9.9.9');
    assert.equal(threadlessManifest.dependencies['@napi-rs/wasm-runtime'], '^9.9.9');
    assert.deepEqual(fetchRequests, ['https://registry.example.invalid/@napi-rs/wasm-runtime']);
    assert.deepEqual((await readdir(packageRoot)).sort(), ['wasm32-wasi', 'wasm32-wasip1']);
  } finally {
    globalThis.fetch = originalFetch;
    if (originalRegistry === undefined) {
      delete process.env.npm_config_registry;
    } else {
      process.env.npm_config_registry = originalRegistry;
    }
    await rm(root, { force: true, recursive: true });
  }
});

test('staging preserves artifact Wasm and repairs an existing skeleton without Wasm', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-wasm-source-'));
  const artifactPackage = path.join(root, 'artifact-package');
  const artifactStage = path.join(root, 'artifact-stage');
  const bootstrapPackage = path.join(root, 'bootstrap-package');
  const bootstrapStage = path.join(root, 'bootstrap-stage');
  const missingSrcWasm = path.join(root, 'missing-src', 'binding.wasm');
  const srcWasm = path.join(root, 'src', 'binding.wasm');
  await Promise.all([
    writePackageSkeleton(artifactPackage),
    writePackageSkeleton(bootstrapPackage),
    mkdir(path.dirname(srcWasm)),
  ]);
  await Promise.all([
    writeFile(path.join(artifactPackage, 'binding.wasm'), 'artifact-wasm'),
    writeFile(srcWasm, 'src-wasm'),
  ]);

  try {
    await copyWasiPackageForStaging({
      packageDir: artifactPackage,
      stagedPackageDir: artifactStage,
      wasm: missingSrcWasm,
    });
    await copyWasiPackageForStaging({
      packageDir: bootstrapPackage,
      stagedPackageDir: bootstrapStage,
      wasm: srcWasm,
    });

    assert.equal(await readFile(path.join(artifactStage, 'binding.wasm'), 'utf8'), 'artifact-wasm');
    assert.equal(await readFile(path.join(bootstrapStage, 'binding.wasm'), 'utf8'), 'src-wasm');
  } finally {
    await rm(root, { force: true, recursive: true });
  }
});

test(
  'staging rejects package symlinks without modifying their external target',
  { skip: process.platform === 'win32' },
  async () => {
    const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-package-symlink-'));
    const packageDir = path.join(root, 'package');
    const stagedPackageDir = path.join(root, 'staged');
    const externalManifest = path.join(root, 'external-package.json');
    const wasm = path.join(root, 'binding.wasm');
    await mkdir(packageDir);
    await Promise.all([
      writeFile(path.join(packageDir, 'README.md'), 'package fixture\n'),
      writeFile(externalManifest, 'external sentinel\n'),
      writeFile(wasm, 'src-wasm'),
    ]);
    await symlink(externalManifest, path.join(packageDir, 'package.json'));

    try {
      await assert.rejects(
        copyWasiPackageForStaging({ packageDir, stagedPackageDir, wasm }),
        /Staged WASI package entry must not be a symlink/,
      );
      assert.equal(await readFile(externalManifest, 'utf8'), 'external sentinel\n');
    } finally {
      await rm(root, { force: true, recursive: true });
    }
  },
);

test('directory transaction recovers an abruptly terminated owner before the next commit', async () => {
  const { root, packageRoot, destinations } =
    await createTransactionFixture('stage-wasi-terminated-');
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
    await abruptlyTerminateChild(interrupted);

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
});
