import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFile, readdir, stat } from 'node:fs/promises';
import nodePath from 'node:path';

export const REQUIRED_NODE_VERSION = 'v24.18.0';
export const RUNTIME_SOURCE_COMMIT = '0aa600b5721b852cdc4095c7122a929a8cb4a798';
export const EXPECTED_NATIVE_BINDING_SHA256 =
  'deec0b2cb7a12e507ff223e12535c3280ab5fe8371f2fcc92f9db206163f1c5d';
export const EXPECTED_DISTRIBUTION_SHA256 =
  'e30311e764bae7fba9afe27665db741d556a7c3728eb67cfbe7ce0fed3135ebc';
export const LIFECYCLE_BASELINE_SOURCE_COMMIT = 'b144106882fe244b19b738fc0acf3ffa07c7c9f3';
export const LIFECYCLE_BASELINE_NATIVE_BINDING_SHA256 =
  '7b8863bb28aefd2e2eb7409f8be6dae57a252fe4a2688383007be7ea2f847bf7';
export const LIFECYCLE_BASELINE_DISTRIBUTION_SHA256 =
  '1efffd0b63483e77cd2854fe716941000ae9548768691d7b5a64dceb011f3c45';
export const ATTRIBUTION_SOURCE_COMMIT = '76a971de8ce66e031b7d19637d13742fe4662594';
export const ATTRIBUTION_NATIVE_BINDING_SHA256 =
  '6d6fc6e94b30b7b39b4c6d116b38bbecca2907ecc183c99a25a1a67e1cce1fce';
export const ATTRIBUTION_NATIVE_BINDING_BYTES = 16_360_800;
export const ATTRIBUTION_DISTRIBUTION_SHA256 =
  '3e4b174ad36807430da1b5b7db3f294a47909962511531b370f421fe00d83fbd';
export const ATTRIBUTION_DISTRIBUTION_FILES = 49;
export const ATTRIBUTION_DISTRIBUTION_BYTES = 17_240_063;
export const ATTRIBUTION_PACKAGE_ENTRY_SHA256 =
  'ecbce9a6cfc187db4d2c818d2500f52372b15b66022358f69c8e578c1dcbb2bc';
export const ATTRIBUTION_PACKAGE_ENTRY_BYTES = 1_642;
export const BASELINE_POOL_ENVIRONMENT = {
  ROLLDOWN_WORKER_THREADS: '18',
  RAYON_NUM_THREADS: '12',
  ROLLDOWN_MAX_BLOCKING_THREADS: '4',
};

const CI_MARKERS = [
  'CI',
  'CONTINUOUS_INTEGRATION',
  'BUILD_NUMBER',
  'RUN_ID',
  'GITHUB_ACTIONS',
  'GITLAB_CI',
  'BUILDKITE',
  'CIRCLECI',
  'JENKINS_URL',
  'TEAMCITY_VERSION',
  'TF_BUILD',
  'TRAVIS',
];

const isActiveMarker = (value) =>
  typeof value === 'string' &&
  value.length !== 0 &&
  value.toLowerCase() !== 'false' &&
  value !== '0';

export function assertLocalExecution() {
  const activeMarkers = CI_MARKERS.filter((name) => isActiveMarker(process.env[name]));
  if (activeMarkers.length !== 0) {
    throw new Error(`Vue scale runners refuse active CI environments: ${activeMarkers.join(', ')}`);
  }
  if (process.version !== REQUIRED_NODE_VERSION) {
    throw new Error(
      `Vue scale runners require Node.js ${REQUIRED_NODE_VERSION}, got ${process.version}`,
    );
  }
  if (typeof process.env.NODE_OPTIONS === 'string' && process.env.NODE_OPTIONS.trim() !== '') {
    throw new Error(
      `Vue scale runners require an empty inherited NODE_OPTIONS, got ${JSON.stringify(process.env.NODE_OPTIONS)}`,
    );
  }
}

const sha256 = (value) => createHash('sha256').update(value).digest('hex');

async function walk(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const paths = await Promise.all(
    entries.map((entry) => {
      const path = nodePath.join(directory, entry.name);
      return entry.isDirectory() ? walk(path) : path;
    }),
  );
  return paths.flat();
}

export async function hashRolldownDistribution(packageRoot) {
  const distributionDirectory = nodePath.join(packageRoot, 'dist');
  const paths = (await walk(distributionDirectory)).sort((left, right) =>
    Buffer.compare(Buffer.from(left), Buffer.from(right)),
  );
  const aggregate = createHash('sha256');
  let bytes = 0;
  for (const path of paths) {
    const content = await readFile(path);
    const relativePath = nodePath
      .relative(distributionDirectory, path)
      .split(nodePath.sep)
      .join('/');
    bytes += content.byteLength;
    aggregate.update(relativePath);
    aggregate.update('\0');
    aggregate.update(content);
    aggregate.update('\0');
  }
  return {
    directory: 'packages/rolldown/dist',
    files: paths.length,
    bytes,
    aggregateSha256: aggregate.digest('hex'),
  };
}

const runGit = (repositoryRoot, arguments_) => {
  const result = spawnSync('git', ['-C', repositoryRoot, ...arguments_], { encoding: 'utf8' });
  if (result.status !== 0) throw new Error(`git ${arguments_.join(' ')} failed`);
  return result.stdout.trim();
};

export async function inspectRuntimeProvenance(
  repositoryRoot,
  packageRoot,
  { requireClean, expectedPin },
) {
  const repositoryCommit = runGit(repositoryRoot, ['rev-parse', 'HEAD']);
  const worktreeStatus = runGit(repositoryRoot, ['status', '--short']);
  if (requireClean && worktreeStatus) {
    throw new Error('formal Vue scale timing requires a clean worktree');
  }

  const bindingDirectory = nodePath.join(packageRoot, 'dist');
  const bindingNames = (await readdir(bindingDirectory)).filter((name) =>
    /^rolldown-binding\..+\.node$/.test(name),
  );
  if (bindingNames.length !== 1) {
    throw new Error(`expected one local native binding, got ${bindingNames.length}`);
  }
  const bindingPath = nodePath.join(bindingDirectory, bindingNames[0]);
  const bindingContent = await readFile(bindingPath);
  const bindingStat = await stat(bindingPath);
  const bindingSha256 = sha256(bindingContent);
  if (
    bindingSha256 !== expectedPin.nativeBindingSha256 ||
    (expectedPin.nativeBindingBytes !== undefined &&
      bindingStat.size !== expectedPin.nativeBindingBytes)
  ) {
    throw new Error(
      `native binding bytes or hash differ from ${expectedPin.kind} pin ${expectedPin.sourceCommit}: ${bindingStat.size}/${bindingSha256}`,
    );
  }

  const distribution = await hashRolldownDistribution(packageRoot);
  if (
    distribution.aggregateSha256 !== expectedPin.distributionSha256 ||
    (expectedPin.distributionFiles !== undefined &&
      distribution.files !== expectedPin.distributionFiles) ||
    (expectedPin.distributionBytes !== undefined &&
      distribution.bytes !== expectedPin.distributionBytes)
  ) {
    throw new Error(
      `Rolldown distribution bytes, files, or hash differ from ${expectedPin.kind} pin ${expectedPin.sourceCommit}: ${distribution.files}/${distribution.bytes}/${distribution.aggregateSha256}`,
    );
  }
  const packageEntryPath = nodePath.join(packageRoot, 'dist/index.mjs');
  const packageEntryContent = await readFile(packageEntryPath);
  const packageEntry = {
    path: 'packages/rolldown/dist/index.mjs',
    bytes: packageEntryContent.byteLength,
    sha256: sha256(packageEntryContent),
  };
  if (
    (expectedPin.packageEntrySha256 !== undefined &&
      packageEntry.sha256 !== expectedPin.packageEntrySha256) ||
    (expectedPin.packageEntryBytes !== undefined &&
      packageEntry.bytes !== expectedPin.packageEntryBytes)
  ) {
    throw new Error(
      `Rolldown package entry bytes or hash differ from ${expectedPin.kind} pin ${expectedPin.sourceCommit}: ${packageEntry.bytes}/${packageEntry.sha256}`,
    );
  }
  if (repositoryCommit !== expectedPin.sourceCommit) {
    throw new Error(
      `Rolldown repository commit differs from ${expectedPin.kind} pin: ${repositoryCommit}`,
    );
  }
  const nodeBinaryContent = await readFile(process.execPath);
  const nodeBinaryStat = await stat(process.execPath);
  return {
    repositoryCommit,
    worktreeStatus,
    packageRoot,
    runtimePin: expectedPin,
    node: process.version,
    nodeBinary: process.execPath,
    nodeArtifact: {
      bytes: nodeBinaryStat.size,
      sha256: sha256(nodeBinaryContent),
    },
    nativeBinding: {
      path: nodePath.relative(repositoryRoot, bindingPath),
      bytes: bindingStat.size,
      sha256: bindingSha256,
      sourceCommit: expectedPin.sourceCommit,
      profileClaim: 'release',
      profileVerification:
        'The expected byte hash pins the unchanged release artifact; the file does not encode its Cargo profile.',
    },
    rolldownDistribution: distribution,
    packageEntry,
    configuredPools: {
      tokio: {
        environmentVariable: 'ROLLDOWN_WORKER_THREADS',
        configuredCapacity: Number(BASELINE_POOL_ENVIRONMENT.ROLLDOWN_WORKER_THREADS),
      },
      rayon: {
        environmentVariable: 'RAYON_NUM_THREADS',
        configuredCapacity: Number(BASELINE_POOL_ENVIRONMENT.RAYON_NUM_THREADS),
      },
      blocking: {
        environmentVariable: 'ROLLDOWN_MAX_BLOCKING_THREADS',
        configuredCapacity: Number(BASELINE_POOL_ENVIRONMENT.ROLLDOWN_MAX_BLOCKING_THREADS),
      },
      interpretation:
        'Tokio, Rayon, blocking, and JavaScript worker capacities are separate configured limits, not observed active CPU counts, and must not be summed as simultaneous CPU demand.',
    },
  };
}

export async function assertRuntimeStable(repositoryRoot, packageRoot, initial) {
  const currentCommit = runGit(repositoryRoot, ['rev-parse', 'HEAD']);
  const currentStatus = runGit(repositoryRoot, ['status', '--short']);
  if (
    currentCommit !== initial.repositoryCommit ||
    currentStatus !== initial.worktreeStatus
  ) {
    throw new Error('runtime commit or worktree changed during Vue scale matrix');
  }
  const distribution = await hashRolldownDistribution(packageRoot);
  if (JSON.stringify(distribution) !== JSON.stringify(initial.rolldownDistribution)) {
    throw new Error('Rolldown distribution changed during Vue scale matrix');
  }
  const bindingPath = nodePath.join(repositoryRoot, initial.nativeBinding.path);
  const bindingContent = await readFile(bindingPath);
  const bindingStat = await stat(bindingPath);
  if (
    bindingStat.size !== initial.nativeBinding.bytes ||
    sha256(bindingContent) !== initial.nativeBinding.sha256
  ) {
    throw new Error('Rolldown native binding changed during Vue scale matrix');
  }
  const packageEntryContent = await readFile(nodePath.join(packageRoot, 'dist/index.mjs'));
  if (
    packageEntryContent.byteLength !== initial.packageEntry.bytes ||
    sha256(packageEntryContent) !== initial.packageEntry.sha256
  ) {
    throw new Error('Rolldown package entry changed during Vue scale matrix');
  }
}
