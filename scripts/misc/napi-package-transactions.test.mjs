import assert from 'node:assert/strict';
import { chmod, mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { NapiCli } from '../../packages/rolldown/node_modules/@napi-rs/cli/dist/index.js';

const binaryName = 'transaction-binding';
const napi = new NapiCli();
const targets = [
  {
    artifact: `${binaryName}.linux-x64-gnu.node`,
    directory: 'linux-x64-gnu',
    triple: 'x86_64-unknown-linux-gnu',
  },
  {
    artifact: `${binaryName}.linux-arm64-gnu.node`,
    directory: 'linux-arm64-gnu',
    triple: 'aarch64-unknown-linux-gnu',
  },
];

async function createFixture() {
  const root = await mkdtemp(path.join(tmpdir(), 'napi-package-transaction-'));
  const artifacts = path.join(root, 'artifacts');
  await mkdir(artifacts);
  await writeFile(
    path.join(root, 'package.json'),
    `${JSON.stringify(
      {
        name: '@fixture/transaction-binding',
        version: '1.0.0',
        napi: {
          binaryName,
          targets: targets.map(({ triple }) => triple),
        },
      },
      null,
      2,
    )}\n`,
  );
  await napi.createNpmDirs({
    cwd: root,
    npmDir: 'npm',
    packageJsonPath: 'package.json',
  });
  for (const [index, target] of targets.entries()) {
    await writeFile(path.join(artifacts, target.artifact), `artifact-${index}-old`);
  }
  await napi.artifacts({
    cwd: root,
    npmDir: 'npm',
    outputDir: 'artifacts',
    packageJsonPath: 'package.json',
  });
  return { artifacts, root };
}

async function makeDirectoryReadOnly(directory) {
  await chmod(directory, 0o555);
  return async () => chmod(directory, 0o755);
}

test(
  'artifacts restores earlier destinations after a later write fails',
  { skip: process.platform === 'win32' },
  async () => {
    const { artifacts, root } = await createFixture();
    const firstTargetPath = path.join(root, 'npm', targets[0].directory, targets[0].artifact);
    const firstRootPath = path.join(root, targets[0].artifact);
    const beforeTarget = await readFile(firstTargetPath);
    const beforeRoot = await readFile(firstRootPath);
    const restorePermissions = await makeDirectoryReadOnly(
      path.join(root, 'npm', targets[1].directory),
    );
    try {
      for (const [index, target] of targets.entries()) {
        await writeFile(path.join(artifacts, target.artifact), `artifact-${index}-new`);
      }

      await assert.rejects(
        napi.artifacts({
          cwd: root,
          npmDir: 'npm',
          outputDir: 'artifacts',
          packageJsonPath: 'package.json',
        }),
        (error) => error?.code === 'EACCES',
      );

      assert.deepEqual(await readFile(firstTargetPath), beforeTarget);
      assert.deepEqual(await readFile(firstRootPath), beforeRoot);
    } finally {
      await restorePermissions();
      await rm(root, { force: true, recursive: true });
    }
  },
);

test(
  'pre-publish restores earlier manifests after a later write fails',
  { skip: process.platform === 'win32' },
  async () => {
    const { root } = await createFixture();
    const firstManifestPath = path.join(root, 'npm', targets[0].directory, 'package.json');
    const rootManifestPath = path.join(root, 'package.json');
    const firstManifest = JSON.parse(await readFile(firstManifestPath, 'utf8'));
    firstManifest.version = '0.0.0';
    await writeFile(firstManifestPath, `${JSON.stringify(firstManifest, null, 2)}\n`);
    const beforeFirstManifest = await readFile(firstManifestPath);
    const beforeRootManifest = await readFile(rootManifestPath);
    const restorePermissions = await makeDirectoryReadOnly(
      path.join(root, 'npm', targets[1].directory),
    );
    try {
      await assert.rejects(
        napi.prePublish({
          cwd: root,
          ghRelease: false,
          npmDir: 'npm',
          packageJsonPath: 'package.json',
          skipOptionalPublish: true,
        }),
        (error) => error?.code === 'EACCES',
      );

      assert.deepEqual(await readFile(firstManifestPath), beforeFirstManifest);
      assert.deepEqual(await readFile(rootManifestPath), beforeRootManifest);
    } finally {
      await restorePermissions();
      await rm(root, { force: true, recursive: true });
    }
  },
);
