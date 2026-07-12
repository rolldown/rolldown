import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFile, readdir, writeFile } from 'node:fs/promises';
import { cpus, platform, release, totalmem } from 'node:os';
import nodePath from 'node:path';
import {
  assertExactAdapterProvenance,
  captureAdapterBaseProvenance,
  captureProjectAdapterProvenance,
} from './adapter-provenance.mjs';
import { ensurePreparedProject } from './prepare-projects.mjs';
import {
  BASELINE_POOL_ENVIRONMENT,
  LIFECYCLE_BASELINE,
  REPOSITORY_ROOT,
  assertLocalNode,
  projectDefinition,
} from './projects.mjs';
import {
  comparableEvidence,
  createCompactSummary,
  stablePrepared,
  validateMatrix,
  verifyGolden,
  verifyRunOutcome,
} from './verification.mjs';

assertLocalNode();
const matrixPath = process.argv[2];
const outputPath = process.argv[3];
const runtimePackageRoot = process.argv[4];
if (!matrixPath || !outputPath || !runtimePackageRoot) {
  throw new Error('usage: node run-matrix.mjs MATRIX OUTPUT ROLLDOWN_PACKAGE_ROOT');
}
const relativeOutput = nodePath.relative(REPOSITORY_ROOT, nodePath.resolve(outputPath));
if (!relativeOutput.startsWith('..') && !nodePath.isAbsolute(relativeOutput)) {
  throw new Error('correctness artifacts must be written outside the Rolldown worktree');
}
const matrix = JSON.parse(await readFile(matrixPath, 'utf8'));
validateMatrix(matrix);
const matrixBytes = await readFile(matrixPath);
const goldenPath = nodePath.resolve(nodePath.dirname(matrixPath), matrix.goldenFile);
const goldenBytes = await readFile(goldenPath);
const goldens = JSON.parse(goldenBytes);

const sha256 = (value) => createHash('sha256').update(value).digest('hex');
const byteSort = (left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right));

function runGit(root, arguments_) {
  const result = spawnSync('git', ['-C', root, ...arguments_], { encoding: 'utf8' });
  if (result.status !== 0) throw new Error(`git ${arguments_.join(' ')} failed`);
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

async function inspectRuntime() {
  const packageRoot = nodePath.resolve(runtimePackageRoot);
  const repositoryRoot = nodePath.resolve(packageRoot, '../..');
  const commit = runGit(repositoryRoot, ['rev-parse', 'HEAD']);
  if (commit !== LIFECYCLE_BASELINE.sourceCommit) {
    throw new Error(`runtime commit is not lifecycle-corrected baseline: ${commit}`);
  }
  const status = runGit(repositoryRoot, ['status', '--short']);
  if (status) throw new Error(`lifecycle-corrected baseline worktree is dirty:\n${status}`);
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
    throw new Error(`lifecycle baseline binding hash mismatch: ${binding.sha256}`);
  }
  if (distributionSha256 !== LIFECYCLE_BASELINE.distributionSha256) {
    throw new Error(`lifecycle baseline distribution hash mismatch: ${distributionSha256}`);
  }
  return {
    profile: LIFECYCLE_BASELINE,
    repositoryRoot,
    packageRoot,
    commit,
    clean: true,
    binding,
    distribution: {
      files: files.length,
      bytes: distributionBytes,
      sha256: distributionSha256,
    },
  };
}

async function inspectHarness() {
  const commit = runGit(REPOSITORY_ROOT, ['rev-parse', 'HEAD']);
  const status = runGit(REPOSITORY_ROOT, ['status', '--short']);
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
    commit,
    clean: status.length === 0,
    statusSha256: sha256(status),
    sourceFileCount: files.length,
    sourceManifestSha256: hash.digest('hex'),
  };
}

const runtime = await inspectRuntime();
const harness = await inspectHarness();
const adapterToolchain = await captureAdapterBaseProvenance();
const startedAt = new Date().toISOString();
const results = [];
const projectAdmissions = new Map();
const projectAdapterProvenance = new Map();
const preparedProjects = new Map();

function normalizeFailure(value, projectRoot) {
  return value
    .replaceAll(projectRoot, '<project-root>')
    .replaceAll(REPOSITORY_ROOT, '<rolldown-research-root>')
    .replaceAll(runtime.repositoryRoot, '<rolldown-runtime-root>');
}

function execute(definition, variant, repeat, adapterProvenance) {
  const { projectId } = definition;
  const projectRoot = nodePath.join(REPOSITORY_ROOT, 'tmp/bench/vue-projects', projectId);
  const environment = {
    ...process.env,
    ...BASELINE_POOL_ENVIRONMENT,
    NODE_ENV: 'production',
    ROLLDOWN_RESEARCH_PACKAGE_ROOT: runtime.packageRoot,
  };
  delete environment.NODE_OPTIONS;
  delete environment.ROLLDOWN_PARALLEL_PLUGIN_WORKERS;
  delete environment.ROLLDOWN_PARALLEL_PLUGIN_METRICS;
  const match = /^worker-(\d+)$/.exec(variant);
  if (match) environment.ROLLDOWN_PARALLEL_PLUGIN_WORKERS = match[1];
  const child = spawnSync(
    process.execPath,
    [
      '--expose-gc',
      '--import',
      nodePath.join(import.meta.dirname, 'register-loader.mjs'),
      nodePath.join(import.meta.dirname, 'run-case.mjs'),
      JSON.stringify({
        projectId,
        variant,
        collectPerformance: false,
        frozenAdapterProvenance: adapterProvenance,
      }),
    ],
    { encoding: 'utf8', env: environment, maxBuffer: 64 * 1024 * 1024 },
  );
  const stdoutLines = child.stdout.trim().split('\n').filter(Boolean);
  let report;
  try {
    report = JSON.parse(stdoutLines.at(-1));
  } catch {
    report = undefined;
  }
  const stderr = normalizeFailure(child.stderr, projectRoot);
  const stdout = normalizeFailure(child.stdout, projectRoot);
  const result = {
    projectId,
    variant,
    repeat,
    childStatus: child.status,
    childSignal: child.signal,
    stderrBytes: Buffer.byteLength(stderr),
    stderrSha256: sha256(stderr),
    stderr,
    stdoutSha256: sha256(stdout),
    report,
  };
  results.push(result);
  if (!report) throw new Error(`${projectId}/${variant} did not emit a JSON report`);
  verifyRunOutcome(definition, result);
  verifyGolden(projectId, report, goldens);
  assertExactAdapterProvenance(
    report.adapterProvenance,
    adapterProvenance,
    `${projectId}/${variant} child adapter provenance`,
  );
  return result;
}

for (const definition of matrix.cases) {
  const { projectId, ordinaryRepeats = 2, workerVariants } = definition;
  const project = projectDefinition(projectId);
  if (project.fallbackFor && projectAdmissions.get(project.fallbackFor) === 'accepted') {
    results.push({
      projectId,
      skipped: true,
      reason: `${project.fallbackFor} passed ordinary admission; frozen fallback must not run`,
    });
    continue;
  }
  const prepared = await ensurePreparedProject(projectId);
  preparedProjects.set(projectId, prepared);
  const adapterProvenance = await captureProjectAdapterProvenance(
    projectId,
    prepared.root,
    adapterToolchain,
  );
  projectAdapterProvenance.set(projectId, adapterProvenance);
  const ordinary = [];
  for (let repeat = 0; repeat < ordinaryRepeats; repeat++) {
    ordinary.push(execute(definition, 'ordinary', repeat, adapterProvenance));
  }
  const first = ordinary[0].report;
  projectAdmissions.set(projectId, first.admissionStatus);
  const reference = JSON.stringify(comparableEvidence(first));
  for (const run of ordinary.slice(1)) {
    if (JSON.stringify(comparableEvidence(run.report)) !== reference) {
      throw new Error(`${projectId} ordinary correctness is not deterministic`);
    }
  }
  for (const variant of workerVariants) {
    const worker = execute(definition, variant, 0, adapterProvenance);
    if (JSON.stringify(comparableEvidence(worker.report)) !== reference) {
      throw new Error(`${projectId}/${variant} differs from ordinary correctness reference`);
    }
  }
}

for (const [projectId, initial] of preparedProjects) {
  const current = await ensurePreparedProject(projectId);
  if (JSON.stringify(stablePrepared(current)) !== JSON.stringify(stablePrepared(initial))) {
    throw new Error(`${projectId} preparation snapshot changed during correctness matrix`);
  }
}

const [finalRuntime, finalHarness, finalAdapterToolchain, finalMatrixBytes, finalGoldenBytes] =
  await Promise.all([
    inspectRuntime(),
    inspectHarness(),
    captureAdapterBaseProvenance(),
    readFile(matrixPath),
    readFile(goldenPath),
  ]);
if (JSON.stringify(finalRuntime) !== JSON.stringify(runtime)) {
  throw new Error('lifecycle runtime provenance changed during correctness matrix');
}
if (JSON.stringify(finalHarness) !== JSON.stringify(harness)) {
  throw new Error('independent Vue harness changed during correctness matrix');
}
assertExactAdapterProvenance(
  finalAdapterToolchain,
  adapterToolchain,
  'adapter toolchain changed during correctness matrix',
);
if (
  sha256(finalMatrixBytes) !== sha256(matrixBytes) ||
  sha256(finalGoldenBytes) !== sha256(goldenBytes)
) {
  throw new Error('correctness matrix or golden changed during execution');
}
for (const [projectId, initial] of projectAdapterProvenance) {
  const current = await captureProjectAdapterProvenance(
    projectId,
    nodePath.join(REPOSITORY_ROOT, 'tmp/bench/vue-projects', projectId),
    finalAdapterToolchain,
  );
  assertExactAdapterProvenance(current, initial, `${projectId} compiler changed during matrix`);
}
assertLocalNode();

const report = {
  schema: 1,
  measurementClass: 'correctness-only',
  timingEligible: false,
  startedAt,
  finishedAt: new Date().toISOString(),
  node: process.version,
  nodeBinary: process.execPath,
  executionEnvironment: {
    inheritedNodeOptions: null,
    childNodeOptions: null,
    childLoaderArgument: `--import ${nodePath.join(import.meta.dirname, 'register-loader.mjs')}`,
  },
  runtime,
  harness,
  adapterToolchain,
  projectAdapterProvenance: Object.fromEntries(projectAdapterProvenance),
  matrixSha256: sha256(matrixBytes),
  goldenSha256: sha256(goldenBytes),
  configuredPools: BASELINE_POOL_ENVIRONMENT,
  host: {
    platform: platform(),
    release: release(),
    architecture: process.arch,
    cpuModel: cpus()[0]?.model,
    logicalCpuCount: cpus().length,
    totalMemoryBytes: totalmem(),
  },
  matrix,
  projectAdmissions: Object.fromEntries(projectAdmissions),
  results,
};
const rawBytes = Buffer.from(`${JSON.stringify(report, null, 2)}\n`);
await writeFile(outputPath, rawBytes);
const summary = createCompactSummary(report, sha256(rawBytes), harness);
const summaryPath = outputPath.endsWith('.json')
  ? `${outputPath.slice(0, -'.json'.length)}.summary.json`
  : `${outputPath}.summary.json`;
await writeFile(summaryPath, `${JSON.stringify(summary, null, 2)}\n`);
console.log(
  JSON.stringify({
    outputPath,
    summaryPath,
    durableEligible: harness.clean,
    projectAdmissions: report.projectAdmissions,
    runs: results.length,
  }),
);
