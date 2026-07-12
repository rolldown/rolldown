import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { cpus, platform, release, totalmem } from 'node:os';
import nodePath from 'node:path';
import {
  assertExactAdapterProvenance,
  captureAdapterBaseProvenance,
  captureProjectAdapterProvenance,
} from './adapter-provenance.mjs';
import {
  admitFormalHostAfterChild,
  admitFormalHostBeforeChild,
  assertNoPagingDelta,
  virtualMemoryCounters,
} from './performance-host-policy.mjs';
import {
  assertNoInheritedNodeOptions,
  assertParentWallSanity,
  assertPerformanceCorrectnessBinding,
  createPerformanceCompactSummary,
  parseMacOsTimeOutput,
  validatePerformanceMatrix,
} from './performance-policy.mjs';
import {
  captureHarnessProvenance,
  inspectLifecycleRuntime,
  validateCorrectnessEvidenceSet,
} from './performance-provenance.mjs';
import { BASELINE_POOL_ENVIRONMENT, REPOSITORY_ROOT, assertLocalNode } from './projects.mjs';
import { canonicalEvidenceSha256, verifyGolden } from './verification.mjs';
import { ensurePreparedProject } from './prepare-projects.mjs';

assertLocalNode();
assertNoInheritedNodeOptions(process.env);

const matrixPath = process.argv[2];
const outputPath = process.argv[3];
const runtimePackageRoot = process.argv[4];
const validateOnly = process.argv.includes('--validate-only');
const evidenceIndex = process.argv.indexOf('--correctness-evidence');
const evidenceManifestPath = evidenceIndex === -1 ? undefined : process.argv[evidenceIndex + 1];
const screenEvidenceIndex = process.argv.indexOf('--screen-evidence');
const screenRawPath =
  screenEvidenceIndex === -1 ? undefined : process.argv[screenEvidenceIndex + 1];
const screenSummaryPath =
  screenEvidenceIndex === -1 ? undefined : process.argv[screenEvidenceIndex + 2];
if (!matrixPath || !runtimePackageRoot || (!validateOnly && !outputPath)) {
  throw new Error(
    'usage: node run-performance.mjs MATRIX OUTPUT RUNTIME [--correctness-evidence MANIFEST] [--screen-evidence RAW SUMMARY] [--validate-only]',
  );
}
if (screenEvidenceIndex !== -1 && (!screenRawPath || !screenSummaryPath)) {
  throw new Error('--screen-evidence requires raw and summary paths');
}
if (evidenceIndex !== -1 && !evidenceManifestPath) {
  throw new Error('--correctness-evidence requires a manifest path');
}
if (!validateOnly) {
  const relativeOutput = nodePath.relative(REPOSITORY_ROOT, nodePath.resolve(outputPath));
  if (!relativeOutput.startsWith('..') && !nodePath.isAbsolute(relativeOutput)) {
    throw new Error('formal performance artifacts must be written outside the research worktree');
  }
}

const sha256 = (value) => createHash('sha256').update(value).digest('hex');
const matrixBytes = await readFile(matrixPath);
const matrix = JSON.parse(matrixBytes);
validatePerformanceMatrix(matrix);
const goldenPath = nodePath.join(import.meta.dirname, 'correctness-goldens.json');
const goldenBytes = await readFile(goldenPath);
const goldens = JSON.parse(goldenBytes);
const harness = await captureHarnessProvenance({ requireClean: !validateOnly });
const runtime = await inspectLifecycleRuntime(runtimePackageRoot, { requireClean: !validateOnly });
const adapterToolchain = await captureAdapterBaseProvenance();
let correctnessEvidence;
if (evidenceManifestPath) {
  correctnessEvidence = await validateCorrectnessEvidenceSet({
    manifestPath: evidenceManifestPath,
    currentHarness: harness,
    currentRuntime: runtime,
    currentAdapterToolchain: adapterToolchain,
    goldenBytes,
  });
} else if (!validateOnly) {
  throw new Error('formal wall execution requires --correctness-evidence');
}
let screenEvidence;
if (screenRawPath && screenSummaryPath) {
  screenEvidence = await validateScreenEvidence(screenRawPath, screenSummaryPath);
} else if (!validateOnly && matrix.lane === 'independent-vue-wall-confirm') {
  throw new Error('formal confirmation requires --screen-evidence RAW SUMMARY');
}

if (validateOnly) {
  console.log(
    JSON.stringify({
      validatedOnly: true,
      lane: matrix.lane,
      projects: matrix.cases.map(({ projectId }) => projectId),
      runtimePin: runtime.profile,
      runtimeClean: runtime.clean,
      harnessClean: harness.clean,
      harnessSourceManifestSha256: harness.sourceManifestSha256,
      adapterToolchainManifestSha256: adapterToolchain.installation.manifestSha256,
      correctnessEvidenceValidated: Boolean(correctnessEvidence),
      screenEvidenceValidated: Boolean(screenEvidence),
      performanceReady:
        harness.clean &&
        runtime.clean &&
        Boolean(correctnessEvidence) &&
        (matrix.lane !== 'independent-vue-wall-confirm' || Boolean(screenEvidence)),
      hostGateEvaluated: false,
    }),
  );
  process.exit(0);
}

const preparedProjects = new Map();
const projectAdapterProvenance = new Map();
for (const { projectId } of matrix.cases) {
  const prepared = await ensurePreparedProject(projectId);
  preparedProjects.set(projectId, prepared);
  const currentAdapter = await captureProjectAdapterProvenance(
    projectId,
    prepared.root,
    adapterToolchain,
  );
  assertExactAdapterProvenance(
    currentAdapter,
    correctnessEvidence.projectAdapterProvenance[projectId],
    `${projectId} compiler differs from correctness evidence`,
  );
  projectAdapterProvenance.set(projectId, currentAdapter);
}

const startedAt = new Date().toISOString();
const runs = [];
let sequence = 0;
for (const definition of matrix.cases) {
  for (let blockIndex = 0; blockIndex < definition.repeats; blockIndex++) {
    const offset = (definition.rotationOffset + blockIndex) % definition.variants.length;
    const order = [...definition.variants.slice(offset), ...definition.variants.slice(0, offset)];
    for (const variant of order) {
      runs.push({
        sequence: sequence++,
        ...(await executePerformanceChild(definition, variant, blockIndex)),
      });
    }
  }
  const projectEvidence = new Set(
    runs
      .filter(({ projectId }) => projectId === definition.projectId)
      .map(({ canonicalEvidenceSha256: value }) => value),
  );
  if (projectEvidence.size !== 1) {
    throw new Error(`${definition.projectId} performance variants changed correctness evidence`);
  }
}

for (const { projectId } of matrix.cases) {
  const finalPrepared = await ensurePreparedProject(projectId);
  if (
    JSON.stringify(stablePreparation(finalPrepared)) !==
    JSON.stringify(stablePreparation(preparedProjects.get(projectId)))
  ) {
    throw new Error(`${projectId} preparation snapshot changed during performance matrix`);
  }
}

const finalRuntime = await inspectLifecycleRuntime(runtimePackageRoot, { requireClean: true });
if (JSON.stringify(finalRuntime) !== JSON.stringify(runtime)) {
  throw new Error('lifecycle runtime provenance changed during performance matrix');
}
const finalHarness = await captureHarnessProvenance({ requireClean: true });
if (JSON.stringify(finalHarness) !== JSON.stringify(harness)) {
  throw new Error('independent Vue harness changed during performance matrix');
}
const finalAdapterToolchain = await captureAdapterBaseProvenance();
assertExactAdapterProvenance(
  finalAdapterToolchain,
  adapterToolchain,
  'adapter toolchain changed during performance matrix',
);
for (const [projectId, initialAdapter] of projectAdapterProvenance) {
  const currentAdapter = await captureProjectAdapterProvenance(
    projectId,
    preparedProjects.get(projectId).root,
    finalAdapterToolchain,
  );
  assertExactAdapterProvenance(
    currentAdapter,
    initialAdapter,
    `${projectId} compiler changed during performance matrix`,
  );
}
const finalCorrectnessEvidence = await validateCorrectnessEvidenceSet({
  manifestPath: evidenceManifestPath,
  currentHarness: finalHarness,
  currentRuntime: finalRuntime,
  currentAdapterToolchain: finalAdapterToolchain,
  goldenBytes,
});
if (JSON.stringify(finalCorrectnessEvidence) !== JSON.stringify(correctnessEvidence)) {
  throw new Error('correctness evidence changed during performance matrix');
}
if (screenEvidence) {
  const finalScreenEvidence = await validateScreenEvidence(screenRawPath, screenSummaryPath);
  if (JSON.stringify(finalScreenEvidence) !== JSON.stringify(screenEvidence)) {
    throw new Error('screen evidence changed during performance confirmation');
  }
}

const report = {
  schema: 1,
  measurementClass: 'formal local wall evidence subject to host gates',
  admitted: true,
  startedAt,
  finishedAt: new Date().toISOString(),
  node: process.version,
  nodeBinary: process.execPath,
  runtime,
  harness,
  adapterToolchain,
  projectAdapterProvenance: Object.fromEntries(projectAdapterProvenance),
  correctnessEvidence,
  screenEvidence,
  preparedProjects: Object.fromEntries(
    [...preparedProjects].map(([projectId, value]) => [projectId, stablePreparation(value)]),
  ),
  matrixSha256: sha256(matrixBytes),
  goldenSha256: sha256(goldenBytes),
  matrix,
  configuredPools: BASELINE_POOL_ENVIRONMENT,
  executionEnvironment: {
    inheritedNodeOptions: null,
    childNodeOptions: null,
    childLoaderArgument: `--import ${nodePath.join(import.meta.dirname, 'register-loader.mjs')}`,
  },
  host: {
    platform: platform(),
    release: release(),
    architecture: process.arch,
    cpuModel: cpus()[0]?.model,
    logicalCpuCount: cpus().length,
    totalMemoryBytes: totalmem(),
  },
  runs,
};
const rawBytes = Buffer.from(`${JSON.stringify(report, null, 2)}\n`);
const summary = createPerformanceCompactSummary(report, sha256(rawBytes));
const summaryPath = outputPath.endsWith('.json')
  ? `${outputPath.slice(0, -'.json'.length)}.summary.json`
  : `${outputPath}.summary.json`;
await mkdir(nodePath.dirname(nodePath.resolve(outputPath)), { recursive: true });
await writeFile(outputPath, rawBytes);
await writeFile(summaryPath, `${JSON.stringify(summary, null, 2)}\n`);
console.log(
  JSON.stringify({
    outputPath,
    summaryPath,
    lane: matrix.lane,
    runs: runs.length,
    rawSha256: sha256(rawBytes),
    canonicalSummarySha256: summary.canonicalSummarySha256,
  }),
);

async function executePerformanceChild(definition, variant, blockIndex) {
  const hostAdmission = await admitFormalHostBeforeChild();
  const beforeVm = virtualMemoryCounters();
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
  const arguments_ = [
    process.execPath,
    '--expose-gc',
    '--import',
    nodePath.join(import.meta.dirname, 'register-loader.mjs'),
    nodePath.join(import.meta.dirname, 'run-case.mjs'),
    JSON.stringify({
      projectId: definition.projectId,
      variant,
      collectPerformance: true,
      formalPerformanceProtocol: matrix.protocol,
      formalPreparedProject: preparedProjects.get(definition.projectId),
      frozenAdapterProvenance: projectAdapterProvenance.get(definition.projectId),
    }),
  ];
  const started = process.hrtime.bigint();
  const child = spawnSync('/usr/bin/time', ['-l', ...arguments_], {
    encoding: 'utf8',
    env: environment,
    maxBuffer: 64 * 1024 * 1024,
  });
  const parentWallNs = process.hrtime.bigint() - started;
  const childWallMs = Number(parentWallNs) / 1e6;
  const afterVm = virtualMemoryCounters();
  const postHostAdmission = admitFormalHostAfterChild();
  const pagingDelta = assertNoPagingDelta(beforeVm, afterVm);
  if (child.error) throw new Error(`failed to spawn formal child: ${child.error.message}`);
  if (child.status !== 0 || child.signal !== null) {
    throw new Error(
      `${definition.projectId}/${variant} failed with ${child.status}/${child.signal}:\n${child.stderr}`,
    );
  }
  if (/^\[rolldown-parallel-plugin-/m.test(child.stderr)) {
    throw new Error(`${definition.projectId}/${variant} emitted forbidden wall instrumentation`);
  }
  const time = parseMacOsTimeOutput(child.stderr);
  const timeRealMs = time.realSeconds * 1000;
  const lines = child.stdout.trim().split('\n').filter(Boolean);
  if (lines.length !== 1) {
    throw new Error(`${definition.projectId}/${variant} emitted unexpected stdout lines`);
  }
  const report = JSON.parse(lines[0]);
  if (
    report.projectId !== definition.projectId ||
    report.variant !== variant ||
    report.measurementClass !== 'formal-performance-child' ||
    report.executionStatus !== 'completed' ||
    report.admissionStatus !== 'accepted' ||
    report.transform?.reachedSfcCount !== definition.expectedReachedSfcCount ||
    !report.performance
  ) {
    throw new Error(`${definition.projectId}/${variant} failed strict performance admission`);
  }
  verifyGolden(definition.projectId, { ...report, measurementClass: 'correctness-only' }, goldens);
  const result = {
    projectId: definition.projectId,
    band: definition.band,
    expectedReachedSfcCount: definition.expectedReachedSfcCount,
    variant,
    blockIndex,
    parentWallNs: String(parentWallNs),
    childWallMs,
    timeRealToken: time.realToken,
    timeRealPrecisionMs: time.realPrintedResolutionMs,
    timeRealMs,
    parentWallOverheadMs: childWallMs - timeRealMs,
    timeUserMs: time.userSeconds * 1000,
    timeSystemMs: time.systemSeconds * 1000,
    totalCpuMs: (time.userSeconds + time.systemSeconds) * 1000,
    peakRssBytes: time.peakRssBytes,
    pagingDelta,
    hostAdmission,
    postHostAdmission,
    canonicalEvidenceSha256: canonicalEvidenceSha256(report),
    childStdoutSha256: sha256(child.stdout),
    childStderrSha256: sha256(child.stderr),
    report,
  };
  assertParentWallSanity(result);
  assertPerformanceCorrectnessBinding(result, correctnessEvidence, matrix.lane);
  return result;
}

function stablePreparation(value) {
  if (!value) return value;
  return {
    ...value,
    dependencyPreparation: value.dependencyPreparation
      ? { ...value.dependencyPreparation, installPerformed: undefined }
      : undefined,
  };
}

async function validateScreenEvidence(rawPath, summaryPath) {
  if (matrix.lane !== 'independent-vue-wall-confirm') {
    throw new Error('--screen-evidence is valid only for a confirmation matrix');
  }
  const [rawBytes, summaryBytes] = await Promise.all([readFile(rawPath), readFile(summaryPath)]);
  const raw = JSON.parse(rawBytes);
  const summary = JSON.parse(summaryBytes);
  const rawSha256 = sha256(rawBytes);
  if (
    rawSha256 !== matrix.generatedFrom.screenRawSha256 ||
    raw.measurementClass !== 'formal local wall evidence subject to host gates' ||
    raw.matrix?.lane !== 'independent-vue-wall-screen' ||
    raw.admitted !== true ||
    JSON.stringify(raw.harness) !== JSON.stringify(harness) ||
    JSON.stringify(stableRuntime(raw.runtime)) !== JSON.stringify(stableRuntime(runtime)) ||
    JSON.stringify(raw.adapterToolchain) !== JSON.stringify(adapterToolchain) ||
    JSON.stringify(raw.projectAdapterProvenance) !==
      JSON.stringify(correctnessEvidence?.projectAdapterProvenance) ||
    JSON.stringify(raw.correctnessEvidence) !== JSON.stringify(correctnessEvidence) ||
    raw.goldenSha256 !== sha256(goldenBytes)
  ) {
    throw new Error('screen evidence is stale or does not match the confirmation matrix');
  }
  const expectedSummary = createPerformanceCompactSummary(raw, rawSha256);
  if (JSON.stringify(summary) !== JSON.stringify(expectedSummary)) {
    throw new Error('screen compact summary does not match its raw artifact');
  }
  if (!summary.durableEligible) throw new Error('screen compact summary is not durable');
  return {
    raw: { path: nodePath.resolve(rawPath), bytes: rawBytes.byteLength, sha256: rawSha256 },
    summary: {
      path: nodePath.resolve(summaryPath),
      bytes: summaryBytes.byteLength,
      sha256: sha256(summaryBytes),
      canonicalSummarySha256: summary.canonicalSummarySha256,
    },
  };
}

function stableRuntime(value) {
  return {
    profile: value?.profile,
    commit: value?.commit,
    clean: value?.clean,
    binding: value?.binding,
    distribution: value?.distribution,
  };
}
