import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { lstat, readFile, realpath } from 'node:fs/promises';
import nodePath from 'node:path';

export const CORRECTNESS_ARTIFACT_REPOSITORY = 'github.com/hyf0/rolldown-parallel-js-plugin';
export const CORRECTNESS_ARTIFACT_ROOT_PREFIX = 'research/artifacts/correctness/sha256';

const sha256 = (value) => createHash('sha256').update(value).digest('hex');
const hashPattern = /^[a-f0-9]{64}$/;

function gitText(cwd, arguments_) {
  const result = spawnSync('git', ['-C', cwd, ...arguments_], {
    encoding: 'utf8',
    maxBuffer: 16 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(`artifact store git ${arguments_.join(' ')} failed: ${result.stderr}`);
  }
  return result.stdout.trim();
}

function gitBytes(cwd, arguments_) {
  const result = spawnSync('git', ['-C', cwd, ...arguments_], {
    encoding: null,
    maxBuffer: 256 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(`artifact store git ${arguments_.join(' ')} failed`);
  }
  return result.stdout;
}

function normalizedRepository(value) {
  return value
    .trim()
    .replace(/^https?:\/\//, '')
    .replace(/^ssh:\/\/git@/, '')
    .replace(/^git@([^:]+):/, '$1/')
    .replace(/\.git$/, '');
}

export function correctnessArtifactSetAddress(artifacts) {
  if (!Array.isArray(artifacts) || artifacts.length < 1) {
    throw new Error('content-addressed correctness store needs artifacts');
  }
  const pairs = artifacts.map(({ rawSha256, summarySha256 }) => {
    if (!hashPattern.test(rawSha256) || !hashPattern.test(summarySha256)) {
      throw new Error('correctness artifact content address requires SHA-256 pairs');
    }
    return `${rawSha256}\0${summarySha256}\n`;
  });
  pairs.sort((left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right)));
  if (new Set(pairs).size !== pairs.length) {
    throw new Error('correctness artifact content address contains duplicate pairs');
  }
  return sha256(pairs.join(''));
}

function resolveInside(root, relativePath, label) {
  if (typeof relativePath !== 'string' || relativePath.length === 0) {
    throw new Error(`${label} path is required`);
  }
  const path = nodePath.resolve(root, relativePath);
  const relative = nodePath.relative(root, path);
  if (relative.startsWith('..') || nodePath.isAbsolute(relative)) {
    throw new Error(`${label} escapes the content-addressed artifact root`);
  }
  return path;
}

async function readTrackedHeadFile(repositoryRoot, path, label) {
  const relative = nodePath.relative(repositoryRoot, path).split(nodePath.sep).join('/');
  const pathStat = await lstat(path);
  if (!pathStat.isFile()) throw new Error(`${label} must be a regular tracked file`);
  gitText(repositoryRoot, ['ls-files', '--error-unmatch', '--', relative]);
  const [workingBytes, headBytes] = await Promise.all([
    readFile(path),
    Promise.resolve(gitBytes(repositoryRoot, ['show', `HEAD:${relative}`])),
  ]);
  if (!workingBytes.equals(headBytes)) {
    throw new Error(`${label} differs from the file committed at artifact-store HEAD`);
  }
  return { path, relative, bytes: workingBytes };
}

export async function validateCommittedCorrectnessArtifactStore(manifestPath) {
  const absoluteManifestPath = await realpath(nodePath.resolve(manifestPath));
  const repositoryRoot = gitText(nodePath.dirname(absoluteManifestPath), [
    'rev-parse',
    '--show-toplevel',
  ]);
  const repository = normalizedRepository(gitText(repositoryRoot, ['remote', 'get-url', 'origin']));
  if (repository !== CORRECTNESS_ARTIFACT_REPOSITORY) {
    throw new Error(`correctness artifacts are not in ${CORRECTNESS_ARTIFACT_REPOSITORY}`);
  }
  const manifestFile = await readTrackedHeadFile(
    repositoryRoot,
    absoluteManifestPath,
    'correctness evidence manifest',
  );
  const manifest = JSON.parse(manifestFile.bytes);
  if (
    manifest.schema !== 2 ||
    manifest.artifactStore?.kind !== 'git-head-content-addressed' ||
    manifest.artifactStore?.repository !== CORRECTNESS_ARTIFACT_REPOSITORY ||
    !hashPattern.test(manifest.artifactStore?.contentSha256) ||
    !Array.isArray(manifest.artifacts) ||
    manifest.artifacts.length < 1
  ) {
    throw new Error('correctness evidence manifest has an invalid committed-store contract');
  }
  const expectedRootRelative = `${CORRECTNESS_ARTIFACT_ROOT_PREFIX}/${manifest.artifactStore.contentSha256}`;
  if (manifest.artifactStore.root !== expectedRootRelative) {
    throw new Error('correctness artifact root is not the declared content address');
  }
  const artifactRoot = nodePath.resolve(repositoryRoot, expectedRootRelative);
  if (absoluteManifestPath !== nodePath.join(artifactRoot, 'manifest.json')) {
    throw new Error('correctness evidence manifest is outside its canonical artifact root');
  }
  const artifacts = [];
  for (const [index, entry] of manifest.artifacts.entries()) {
    if (
      !hashPattern.test(entry.rawSha256) ||
      !hashPattern.test(entry.summarySha256) ||
      entry.raw !== `raw/${entry.rawSha256}.json` ||
      entry.summary !== `summary/${entry.summarySha256}.json`
    ) {
      throw new Error(`correctness artifact ${index} has noncanonical paths or hashes`);
    }
    const rawFile = await readTrackedHeadFile(
      repositoryRoot,
      resolveInside(artifactRoot, entry.raw, `correctness raw ${index}`),
      `correctness raw ${index}`,
    );
    const summaryFile = await readTrackedHeadFile(
      repositoryRoot,
      resolveInside(artifactRoot, entry.summary, `correctness summary ${index}`),
      `correctness summary ${index}`,
    );
    if (
      sha256(rawFile.bytes) !== entry.rawSha256 ||
      sha256(summaryFile.bytes) !== entry.summarySha256
    ) {
      throw new Error(`correctness artifact ${index} content hash mismatch`);
    }
    artifacts.push({ entry, rawFile, summaryFile });
  }
  const contentSha256 = correctnessArtifactSetAddress(manifest.artifacts);
  if (contentSha256 !== manifest.artifactStore.contentSha256) {
    throw new Error('correctness artifact-set content address mismatch');
  }
  return {
    repository: CORRECTNESS_ARTIFACT_REPOSITORY,
    repositoryRoot,
    repositoryHead: gitText(repositoryRoot, ['rev-parse', 'HEAD']),
    artifactRoot,
    contentSha256,
    manifest,
    manifestFile,
    artifacts,
  };
}
