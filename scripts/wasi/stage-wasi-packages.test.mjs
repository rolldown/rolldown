import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import {
  access,
  mkdtemp,
  mkdir,
  readFile,
  readdir,
  rm,
  symlink,
  writeFile,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  copyWasiPackageForStaging,
  ensureWasiPackageDirectories,
  replaceDirectoriesTransactionally,
} from './stage-wasi-packages.mjs';

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
