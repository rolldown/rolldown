import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { replaceDirectoriesTransactionally } from './stage-wasi-packages.mjs';

async function writeMarker(directory, marker) {
  await mkdir(directory, { recursive: true });
  await writeFile(path.join(directory, 'marker.txt'), marker);
}

async function readMarker(directory) {
  return readFile(path.join(directory, 'marker.txt'), 'utf8');
}

test('directory transaction restores every package after a mid-commit failure', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stage-wasi-rollback-'));
  try {
    const packageRoot = path.join(root, 'npm');
    const firstPackage = path.join(packageRoot, 'wasm32-wasi');
    const secondPackage = path.join(packageRoot, 'wasm32-wasip1');
    const firstStaged = path.join(root, 'staged', 'wasm32-wasi');
    const secondStaged = path.join(root, 'staged', 'wasm32-wasip1');
    await Promise.all([
      writeMarker(firstPackage, 'old-threaded'),
      writeMarker(secondPackage, 'old-threadless'),
      writeMarker(firstStaged, 'new-threaded'),
      writeMarker(secondStaged, 'new-threadless'),
    ]);

    await assert.rejects(
      replaceDirectoriesTransactionally(
        [
          { destination: firstPackage, staged: firstStaged },
          { destination: secondPackage, staged: secondStaged },
        ],
        {
          afterOperation(phase, index) {
            if (phase === 'backup' && index === 1) {
              throw new Error('injected transaction failure');
            }
          },
        },
      ),
      /injected transaction failure/,
    );

    assert.equal(await readMarker(firstPackage), 'old-threaded');
    assert.equal(await readMarker(secondPackage), 'old-threadless');
    assert.equal(await readMarker(firstStaged), 'new-threaded');
    assert.equal(await readMarker(secondStaged), 'new-threadless');
    assert.deepEqual(
      (await readdir(packageRoot)).filter((entry) => entry.startsWith('.stage-wasi-backup-')),
      [],
    );
  } finally {
    await rm(root, { force: true, recursive: true });
  }
});
