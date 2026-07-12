import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import { lstat, readFile, readlink, readdir, realpath, stat } from 'node:fs/promises';
import nodePath from 'node:path';

export const ATTRIBUTION_RUNTIME = Object.freeze({
  sourceCommit: '76a971de8ce66e031b7d19637d13742fe4662594',
  bindingSha256: '6d6fc6e94b30b7b39b4c6d116b38bbecca2907ecc183c99a25a1a67e1cce1fce',
  distributionSha256: '3e4b174ad36807430da1b5b7db3f294a47909962511531b370f421fe00d83fbd',
  distributionBytes: 17_240_063,
  packageEntrySha256: 'ecbce9a6cfc187db4d2c818d2500f52372b15b66022358f69c8e578c1dcbb2bc',
  packageEntryBytes: 1_642,
});

export const ATTRIBUTION_PACKAGE_ENVIRONMENT = Object.freeze({
  projectFiles: Object.freeze({
    'package.json': 'a56f43a5c1df0cf52223d60e288a265354889dc01fa7126b9a80a56893b21fc7',
    'pnpm-lock.yaml': '2cc3710e028dd31f108d5ac993aca4205bddf126099b6d474a2321f8c2ba170a',
    'pnpm-workspace.yaml': 'a003e5867870bfdb4bcf76331db32d01ecc1a0153d97ee1703dd401b48cc1540',
    'packages/rolldown/package.json':
      '889ad6608781cc1a66ecd094218656d8be76bdade2ccf93971d46288690c6573',
    'node_modules/.modules.yaml':
      '1538c49b1b7fbe8d08d1c661fb12da398f9698e9a0ee1f3755e0e8814c23ed51',
  }),
  staticExternalPackages: Object.freeze([
    Object.freeze({
      name: '@rolldown/pluginutils',
      version: '1.0.1',
      files: 8,
      bytes: 25_693,
      treeSha256: 'beff8f8a8d561dc0dd84cdf24093335d8a25ddeb302693a04378cc3d006331ce',
    }),
  ]),
});

const repositoryRoot = nodePath.resolve(import.meta.dirname, '../../../..');
const ignoredNames = new Set(['.results']);
const compareUtf8 = (left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right));
const sha256 = (value) => createHash('sha256').update(value).digest('hex');

export async function captureInitializationHarnessProvenance({ requireClean }) {
  const worktree = inspectGit(repositoryRoot);
  if (requireClean && worktree.status !== '') {
    throw new Error('formal initialization attribution requires a clean harness worktree');
  }
  const roots = [
    nodePath.join(repositoryRoot, 'examples/par-plugin/cases/runtime-initialization'),
    nodePath.join(repositoryRoot, 'examples/par-plugin/package.json'),
    nodePath.join(repositoryRoot, 'pnpm-lock.yaml'),
  ];
  const entries = [];
  for (const root of roots) await walk(root, entries);
  entries.sort((left, right) => compareUtf8(left.path, right.path));
  return {
    worktree,
    sourceManifest: aggregateEntries(entries),
  };
}

export async function inspectAttributionRuntime(packageRoot, expected = ATTRIBUTION_RUNTIME) {
  const runtimeRepositoryRoot = nodePath.resolve(packageRoot, '../..');
  const worktree = inspectGit(runtimeRepositoryRoot);
  if (worktree.commit !== expected.sourceCommit) {
    throw new Error(
      `initialization attribution runtime commit mismatch: ${worktree.commit} != ${expected.sourceCommit}`,
    );
  }
  if (worktree.status !== '') {
    throw new Error('initialization attribution runtime worktree must be clean');
  }

  const distributionRoot = nodePath.join(packageRoot, 'dist');
  const distributionEntries = [];
  await walk(distributionRoot, distributionEntries, distributionRoot);
  distributionEntries.sort((left, right) => compareUtf8(left.path, right.path));
  const distributionAggregate = createHash('sha256');
  let distributionBytes = 0;
  for (const entry of distributionEntries) {
    const content = await readFile(nodePath.join(distributionRoot, entry.path));
    distributionAggregate.update(entry.path);
    distributionAggregate.update('\0');
    distributionAggregate.update(content);
    distributionAggregate.update('\0');
    distributionBytes += content.byteLength;
  }
  const distribution = {
    algorithm: 'SHA-256 over UTF-8-sorted relative path + NUL + raw content + NUL records',
    files: distributionEntries.length,
    bytes: distributionBytes,
    aggregateSha256: distributionAggregate.digest('hex'),
    entries: distributionEntries,
  };
  if (
    distribution.aggregateSha256 !== expected.distributionSha256 ||
    distribution.bytes !== expected.distributionBytes
  ) {
    throw new Error(
      `initialization attribution distribution mismatch: ${distribution.aggregateSha256}/${distribution.bytes} != ${expected.distributionSha256}/${expected.distributionBytes}`,
    );
  }

  const bindingNames = (await readdir(distributionRoot)).filter((name) =>
    /^rolldown-binding\..+\.node$/.test(name),
  );
  if (bindingNames.length !== 1) {
    throw new Error(`expected one attribution binding, got ${bindingNames.length}`);
  }
  const bindingPath = nodePath.join(distributionRoot, bindingNames[0]);
  const packageEntryPath = nodePath.join(distributionRoot, 'index.mjs');
  const [
    bindingContent,
    packageEntryContent,
    bindingStat,
    packageEntryStat,
    nodeContent,
    nodeStat,
  ] = await Promise.all([
    readFile(bindingPath),
    readFile(packageEntryPath),
    stat(bindingPath),
    stat(packageEntryPath),
    readFile(process.execPath),
    stat(process.execPath),
  ]);
  const bindingSha256 = sha256(bindingContent);
  const packageEntrySha256 = sha256(packageEntryContent);
  if (bindingSha256 !== expected.bindingSha256) {
    throw new Error(
      `initialization attribution binding mismatch: ${bindingSha256} != ${expected.bindingSha256}`,
    );
  }
  if (
    packageEntrySha256 !== expected.packageEntrySha256 ||
    packageEntryStat.size !== expected.packageEntryBytes
  ) {
    throw new Error(
      `initialization attribution package entry mismatch: ${packageEntrySha256}/${packageEntryStat.size} != ${expected.packageEntrySha256}/${expected.packageEntryBytes}`,
    );
  }
  const packageEnvironment = await capturePackageEnvironment(runtimeRepositoryRoot, packageRoot);
  if (JSON.stringify(packageEnvironment) !== JSON.stringify(ATTRIBUTION_PACKAGE_ENVIRONMENT)) {
    throw new Error('initialization attribution package-import environment differs from its pin');
  }
  return {
    runtimeRepositoryRoot,
    packageRoot,
    worktree,
    node: {
      version: process.version,
      path: process.execPath,
      bytes: nodeStat.size,
      sha256: sha256(nodeContent),
    },
    binding: { path: bindingPath, bytes: bindingStat.size, sha256: bindingSha256 },
    packageEntry: {
      path: packageEntryPath,
      bytes: packageEntryStat.size,
      sha256: packageEntrySha256,
    },
    packageEnvironment,
    distribution,
  };
}

async function capturePackageEnvironment(repositoryRoot, packageRoot) {
  const projectFiles = {};
  for (const relativePath of Object.keys(ATTRIBUTION_PACKAGE_ENVIRONMENT.projectFiles)) {
    projectFiles[relativePath] = sha256(
      await readFile(nodePath.join(repositoryRoot, relativePath)),
    );
  }
  const resolver = createRequire(nodePath.join(packageRoot, 'package.json'));
  const staticExternalPackages = [];
  for (const expected of ATTRIBUTION_PACKAGE_ENVIRONMENT.staticExternalPackages) {
    const manifestPath = resolver.resolve(`${expected.name}/package.json`);
    const packageDirectory = await realpath(nodePath.dirname(manifestPath));
    const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
    const tree = await hashPackageTree(packageDirectory);
    staticExternalPackages.push({ name: manifest.name, version: manifest.version, ...tree });
  }
  return { projectFiles, staticExternalPackages };
}

async function hashPackageTree(directory) {
  const files = [];
  await collectPackageFiles(directory, directory, files);
  files.sort((left, right) => compareUtf8(left.relativePath, right.relativePath));
  const aggregate = createHash('sha256');
  let bytes = 0;
  for (const file of files) {
    const content = await readFile(file.path);
    aggregate.update(file.relativePath);
    aggregate.update('\0');
    aggregate.update(content);
    aggregate.update('\0');
    bytes += content.byteLength;
  }
  return { files: files.length, bytes, treeSha256: aggregate.digest('hex') };
}

async function collectPackageFiles(root, directory, files) {
  const children = await readdir(directory, { withFileTypes: true });
  for (const child of children) {
    if (child.name === 'node_modules') continue;
    const path = nodePath.join(directory, child.name);
    if (child.isDirectory()) await collectPackageFiles(root, path, files);
    else if (child.isFile()) {
      files.push({
        path,
        relativePath: nodePath.relative(root, path).split(nodePath.sep).join('/'),
      });
    }
  }
}

export function verifyCurrentHarnessProvenance(initial, current) {
  if (
    initial.worktree.commit !== current.worktree.commit ||
    initial.worktree.status !== current.worktree.status ||
    initial.sourceManifest.aggregateSha256 !== current.sourceManifest.aggregateSha256
  ) {
    throw new Error('initialization harness changed during the matrix');
  }
}

function inspectGit(root) {
  const run = (arguments_) => {
    const result = spawnSync('git', ['-C', root, ...arguments_], { encoding: 'utf8' });
    if (result.status !== 0) {
      throw new Error(`git ${arguments_.join(' ')} failed in ${root}: ${result.stderr}`);
    }
    return result.stdout.trim();
  };
  return { commit: run(['rev-parse', 'HEAD']), status: run(['status', '--short']) };
}

async function walk(path, entries, displayRoot = repositoryRoot) {
  const pathStat = await lstat(path);
  if (!pathStat.isDirectory()) {
    await append(path, entries, displayRoot);
    return;
  }
  const children = await readdir(path, { withFileTypes: true });
  children.sort((left, right) => compareUtf8(left.name, right.name));
  for (const child of children) {
    if (child.isDirectory() && ignoredNames.has(child.name)) continue;
    await walk(nodePath.join(path, child.name), entries, displayRoot);
  }
}

async function append(path, entries, displayRoot) {
  const pathStat = await lstat(path);
  const kind = pathStat.isSymbolicLink() ? 'symlink' : 'file';
  const content = kind === 'symlink' ? Buffer.from(await readlink(path)) : await readFile(path);
  entries.push({
    path: nodePath.relative(displayRoot, path).split(nodePath.sep).join('/'),
    kind,
    bytes: content.byteLength,
    sha256: sha256(content),
  });
}

function aggregateEntries(entries) {
  const aggregate = createHash('sha256');
  for (const entry of entries) {
    aggregate.update(entry.path);
    aggregate.update('\0');
    aggregate.update(entry.kind);
    aggregate.update('\0');
    aggregate.update(String(entry.bytes));
    aggregate.update('\0');
    aggregate.update(entry.sha256);
    aggregate.update('\n');
  }
  return {
    algorithm:
      'SHA-256 over UTF-8-sorted path + NUL + kind + NUL + bytes + NUL + content SHA-256 + LF records',
    files: entries.length,
    bytes: entries.reduce((total, entry) => total + entry.bytes, 0),
    aggregateSha256: aggregate.digest('hex'),
    entries,
  };
}
