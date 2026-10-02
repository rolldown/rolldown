import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, readdir, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { copyWasiPackageForStaging, ensureWasiPackageDirectories } from './stage-wasi-packages.mjs';

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
