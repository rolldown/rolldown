import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFile, readdir } from 'node:fs/promises';
import nodePath from 'node:path';
import {
  assertExactAdapterProvenance,
  assertFrozenProjectAdapterProvenance,
} from './adapter-provenance.mjs';
import { validateCommittedCorrectnessArtifactStore } from './correctness-artifact-store.mjs';
import {
  BASELINE_POOL_ENVIRONMENT,
  LIFECYCLE_BASELINE,
  REQUIRED_NODE_VERSION,
  REPOSITORY_ROOT,
} from './projects.mjs';
import { canonicalEvidenceSha256, createCompactSummary, verifyGolden } from './verification.mjs';

const sha256 = (value) => createHash('sha256').update(value).digest('hex');
const byteSort = (left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right));
const hashPattern = /^[a-f0-9]{64}$/;

export function deriveProjectCorrectnessReferences(projectRuns, requiredProjects) {
  if (!Array.isArray(projectRuns) || !Array.isArray(requiredProjects)) {
    throw new Error('correctness reference derivation requires runs and projects');
  }
  return Object.fromEntries(
    requiredProjects.map((projectId) => {
      const accepted = projectRuns.filter(
        (run) =>
          run.projectId === projectId &&
          run.executionStatus === 'completed' &&
          run.admissionStatus === 'accepted',
      );
      const ordinaryReference = accepted.find(
        ({ variant, repeat }) => variant === 'ordinary' && repeat === 0,
      );
      if (!ordinaryReference || !hashPattern.test(ordinaryReference.canonicalEvidenceSha256)) {
        throw new Error(`${projectId} has no canonical ordinary correctness reference`);
      }
      for (const run of accepted) {
        if (
          !hashPattern.test(run.canonicalEvidenceSha256) ||
          run.canonicalEvidenceSha256 !== ordinaryReference.canonicalEvidenceSha256
        ) {
          throw new Error(
            `${projectId} admitted correctness run ${run.variant}/${run.repeat} differs from ordinary canonical evidence`,
          );
        }
      }
      return [projectId, ordinaryReference.canonicalEvidenceSha256];
    }),
  );
}

function git(arguments_) {
  const result = spawnSync('git', ['-C', REPOSITORY_ROOT, ...arguments_], { encoding: 'utf8' });
  if (result.status !== 0) throw new Error(`git ${arguments_.join(' ')} failed: ${result.stderr}`);
  return result.stdout.trim();
}

async function walk(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  return (
    await Promise.all(
      entries.map((entry) => {
        const path = nodePath.join(directory, entry.name);
        return entry.isDirectory() ? walk(path) : [path];
      }),
    )
  ).flat();
}

export async function captureHarnessProvenance({ requireClean = false } = {}) {
  const status = git(['status', '--short']);
  if (requireClean && status)
    throw new Error('formal independent Vue evidence requires a clean worktree');
  const roots = [
    nodePath.join(REPOSITORY_ROOT, 'examples/par-plugin/cases/vue-projects'),
    nodePath.join(REPOSITORY_ROOT, 'examples/par-plugin/parallel-vue-plugin'),
  ];
  const files = (await Promise.all(roots.map((root) => walk(root))))
    .flat()
    .sort((left, right) =>
      byteSort(nodePath.relative(REPOSITORY_ROOT, left), nodePath.relative(REPOSITORY_ROOT, right)),
    );
  const hash = createHash('sha256');
  for (const path of files) {
    const relative = nodePath.relative(REPOSITORY_ROOT, path).split(nodePath.sep).join('/');
    hash.update(relative);
    hash.update('\0');
    hash.update(await readFile(path));
    hash.update('\0');
  }
  return {
    commit: git(['rev-parse', 'HEAD']),
    clean: status.length === 0,
    statusSha256: sha256(status),
    sourceFileCount: files.length,
    sourceManifestSha256: hash.digest('hex'),
  };
}

export async function inspectLifecycleRuntime(runtimePackageRoot, { requireClean = false } = {}) {
  const packageRoot = nodePath.resolve(runtimePackageRoot);
  const repositoryRoot = nodePath.resolve(packageRoot, '../..');
  const runGit = (arguments_) => {
    const result = spawnSync('git', ['-C', repositoryRoot, ...arguments_], { encoding: 'utf8' });
    if (result.status !== 0) throw new Error(`runtime git ${arguments_.join(' ')} failed`);
    return result.stdout.trim();
  };
  const commit = runGit(['rev-parse', 'HEAD']);
  if (commit !== LIFECYCLE_BASELINE.sourceCommit) {
    throw new Error(`runtime commit is not the lifecycle-corrected baseline: ${commit}`);
  }
  const status = runGit(['status', '--short']);
  if (requireClean && status) throw new Error(`lifecycle runtime worktree is dirty:\n${status}`);
  const distributionDirectory = nodePath.join(packageRoot, 'dist');
  const files = (await walk(distributionDirectory)).sort((left, right) =>
    byteSort(
      nodePath.relative(distributionDirectory, left),
      nodePath.relative(distributionDirectory, right),
    ),
  );
  const distributionHash = createHash('sha256');
  let distributionBytes = 0;
  let binding;
  for (const path of files) {
    const content = await readFile(path);
    const relative = nodePath.relative(distributionDirectory, path).split(nodePath.sep).join('/');
    distributionHash.update(relative);
    distributionHash.update('\0');
    distributionHash.update(content);
    distributionHash.update('\0');
    distributionBytes += content.byteLength;
    if (/^rolldown-binding\..+\.node$/.test(relative)) {
      if (binding) throw new Error('runtime distribution has multiple native bindings');
      binding = { path: relative, bytes: content.byteLength, sha256: sha256(content) };
    }
  }
  if (!binding) throw new Error('runtime distribution has no native binding');
  const distributionSha256 = distributionHash.digest('hex');
  if (binding.sha256 !== LIFECYCLE_BASELINE.nativeBindingSha256) {
    throw new Error(`lifecycle binding hash mismatch: ${binding.sha256}`);
  }
  if (distributionSha256 !== LIFECYCLE_BASELINE.distributionSha256) {
    throw new Error(`lifecycle distribution hash mismatch: ${distributionSha256}`);
  }
  return {
    profile: LIFECYCLE_BASELINE,
    repositoryRoot,
    packageRoot,
    commit,
    clean: status.length === 0,
    binding,
    distribution: {
      files: files.length,
      bytes: distributionBytes,
      sha256: distributionSha256,
    },
  };
}

export async function validateCorrectnessEvidenceSet({
  manifestPath,
  currentHarness,
  currentRuntime,
  currentAdapterToolchain,
  goldenBytes,
}) {
  const store = await validateCommittedCorrectnessArtifactStore(manifestPath);
  const manifestBytes = store.manifestFile.bytes;
  const manifest = store.manifest;
  const currentGoldenSha256 = sha256(goldenBytes);
  const goldens = JSON.parse(goldenBytes);
  const currentMatrices = new Map();
  for (const name of ['correctness-matrix.json', 'amendment-candidate-matrix.json']) {
    const bytes = await readFile(nodePath.join(import.meta.dirname, name));
    currentMatrices.set(sha256(bytes), JSON.parse(bytes));
  }
  const artifacts = [];
  const projectRuns = new Map();
  const projectAdapterProvenance = new Map();
  for (const stored of store.artifacts) {
    const { entry } = stored;
    const rawPath = stored.rawFile.path;
    const summaryPath = stored.summaryFile.path;
    const rawBytes = stored.rawFile.bytes;
    const summaryBytes = stored.summaryFile.bytes;
    const raw = JSON.parse(rawBytes);
    const summary = JSON.parse(summaryBytes);
    const artifactAdapterProvenance = new Map();
    if (
      raw.schema !== 1 ||
      raw.measurementClass !== 'correctness-only' ||
      raw.timingEligible !== false ||
      raw.node !== REQUIRED_NODE_VERSION ||
      raw.executionEnvironment?.inheritedNodeOptions !== null ||
      raw.executionEnvironment?.childNodeOptions !== null ||
      JSON.stringify(raw.harness) !== JSON.stringify(currentHarness) ||
      JSON.stringify(raw.adapterToolchain) !== JSON.stringify(currentAdapterToolchain) ||
      JSON.stringify(raw.runtime?.profile) !== JSON.stringify(currentRuntime.profile) ||
      raw.runtime.commit !== currentRuntime.commit ||
      raw.runtime.binding?.sha256 !== currentRuntime.binding.sha256 ||
      raw.runtime.distribution?.sha256 !== currentRuntime.distribution.sha256 ||
      raw.runtime.distribution?.bytes !== currentRuntime.distribution.bytes ||
      raw.goldenSha256 !== currentGoldenSha256 ||
      !currentMatrices.has(raw.matrixSha256) ||
      JSON.stringify(raw.matrix) !== JSON.stringify(currentMatrices.get(raw.matrixSha256)) ||
      JSON.stringify(raw.configuredPools) !== JSON.stringify(BASELINE_POOL_ENVIRONMENT)
    ) {
      throw new Error(`correctness artifact is stale or ineligible: ${rawPath}`);
    }
    assertExactAdapterProvenance(
      raw.adapterToolchain,
      currentAdapterToolchain,
      'correctness adapter toolchain',
    );
    const expectedSummary = createCompactSummary(raw, sha256(rawBytes), raw.harness);
    if (JSON.stringify(summary) !== JSON.stringify(expectedSummary)) {
      throw new Error(`correctness compact summary does not match raw evidence: ${summaryPath}`);
    }
    if (!summary.durableEligible || summary.rawArtifactSha256 !== sha256(rawBytes)) {
      throw new Error(`correctness summary is not durable: ${summaryPath}`);
    }
    for (const result of raw.results) {
      if (result.skipped) continue;
      if (
        result.childStatus !== 0 ||
        result.childSignal !== null ||
        result.report?.measurementClass !== 'correctness-only' ||
        result.report.performance !== undefined
      ) {
        throw new Error(`correctness child is not cleanly admitted: ${result.projectId}`);
      }
      verifyGolden(result.projectId, result.report, goldens);
      const adapterProvenance = assertFrozenProjectAdapterProvenance(
        result.report.adapterProvenance,
        result.projectId,
      );
      const existingAdapter = artifactAdapterProvenance.get(result.projectId);
      if (existingAdapter) {
        assertExactAdapterProvenance(
          adapterProvenance,
          existingAdapter,
          `${result.projectId} correctness compiler provenance`,
        );
      } else {
        artifactAdapterProvenance.set(result.projectId, adapterProvenance);
      }
      const key = `${result.projectId}\0${result.variant}\0${result.repeat}`;
      if (projectRuns.has(key)) throw new Error(`duplicate correctness evidence run: ${key}`);
      projectRuns.set(key, {
        projectId: result.projectId,
        variant: result.variant,
        repeat: result.repeat,
        executionStatus: result.report.executionStatus,
        admissionStatus: result.report.admissionStatus,
        canonicalEvidenceSha256: canonicalEvidenceSha256(result.report),
      });
    }
    if (
      JSON.stringify(raw.projectAdapterProvenance) !==
      JSON.stringify(Object.fromEntries(artifactAdapterProvenance))
    ) {
      throw new Error(`correctness artifact compiler map is incomplete: ${rawPath}`);
    }
    for (const [projectId, adapterProvenance] of artifactAdapterProvenance) {
      const existingAdapter = projectAdapterProvenance.get(projectId);
      if (existingAdapter) {
        assertExactAdapterProvenance(
          adapterProvenance,
          existingAdapter,
          `${projectId} cross-artifact compiler provenance`,
        );
      } else {
        projectAdapterProvenance.set(projectId, adapterProvenance);
      }
    }
    artifacts.push({
      raw: {
        repositoryRelativePath: stored.rawFile.relative,
        bytes: rawBytes.byteLength,
        sha256: entry.rawSha256,
      },
      summary: {
        repositoryRelativePath: stored.summaryFile.relative,
        bytes: summaryBytes.byteLength,
        sha256: entry.summarySha256,
      },
      matrixSha256: raw.matrixSha256,
      goldenSha256: raw.goldenSha256,
      canonicalSummarySha256: summary.canonicalSummarySha256,
    });
  }
  const requiredProjects = ['floating-vue', 'cabinet-icon', 'directus-amendment-candidate'];
  for (const projectId of requiredProjects) {
    for (const [variant, repeats] of [
      ['ordinary', [0, 1]],
      ['worker-1', [0]],
      ['worker-4', [0]],
    ]) {
      for (const repeat of repeats) {
        const evidence = projectRuns.get(`${projectId}\0${variant}\0${repeat}`);
        if (evidence?.executionStatus !== 'completed' || evidence.admissionStatus !== 'accepted') {
          throw new Error(
            `missing accepted correctness evidence: ${projectId}/${variant}/${repeat}`,
          );
        }
      }
    }
  }
  const projectCanonicalEvidenceSha256 = deriveProjectCorrectnessReferences(
    [...projectRuns.values()],
    requiredProjects,
  );
  return {
    manifest: {
      repositoryRelativePath: store.manifestFile.relative,
      bytes: manifestBytes.byteLength,
      sha256: sha256(manifestBytes),
      repository: store.repository,
      repositoryHead: store.repositoryHead,
      artifactRoot: manifest.artifactStore.root,
      contentSha256: store.contentSha256,
    },
    artifacts,
    admittedProjects: requiredProjects,
    projectCanonicalEvidenceSha256,
    projectAdapterProvenance: Object.fromEntries(
      requiredProjects.map((projectId) => [projectId, projectAdapterProvenance.get(projectId)]),
    ),
  };
}
