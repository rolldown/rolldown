import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import nodePath from 'node:path';
import {
  CORRECTNESS_ARTIFACT_REPOSITORY,
  CORRECTNESS_ARTIFACT_ROOT_PREFIX,
  correctnessArtifactSetAddress,
  validateCommittedCorrectnessArtifactStore,
} from './correctness-artifact-store.mjs';
import { deriveProjectCorrectnessReferences } from './performance-provenance.mjs';
import {
  assertNoPagingDelta,
  immediateHostFailures,
  transientHostFailures,
} from './performance-host-policy.mjs';
import {
  PROJECT_BANDS,
  SCREEN_VARIANTS,
  assertFormalPerformanceAuthorization,
  assertNoInheritedNodeOptions,
  assertParentWallSanity,
  createConfirmationMatrixFromScreen,
  createPerformanceCompactSummary,
  parseMacOsTimeOutput,
  validatePerformanceMatrix,
} from './performance-policy.mjs';

const canonicalA = 'a'.repeat(64);
const canonicalB = 'b'.repeat(64);

assert.deepEqual(
  deriveProjectCorrectnessReferences(
    [
      {
        projectId: 'fixture',
        variant: 'ordinary',
        repeat: 0,
        executionStatus: 'completed',
        admissionStatus: 'accepted',
        canonicalEvidenceSha256: canonicalA,
      },
      {
        projectId: 'fixture',
        variant: 'worker-4',
        repeat: 0,
        executionStatus: 'completed',
        admissionStatus: 'accepted',
        canonicalEvidenceSha256: canonicalA,
      },
    ],
    ['fixture'],
  ),
  { fixture: canonicalA },
);
assert.throws(
  () =>
    deriveProjectCorrectnessReferences(
      [
        {
          projectId: 'fixture',
          variant: 'ordinary',
          repeat: 0,
          executionStatus: 'completed',
          admissionStatus: 'accepted',
          canonicalEvidenceSha256: canonicalA,
        },
        {
          projectId: 'fixture',
          variant: 'worker-4',
          repeat: 0,
          executionStatus: 'completed',
          admissionStatus: 'accepted',
          canonicalEvidenceSha256: canonicalB,
        },
      ],
      ['fixture'],
    ),
  /differs from ordinary canonical evidence/,
);

const screenMatrix = JSON.parse(
  await readFile(new URL('./performance-wall-screen-matrix.json', import.meta.url), 'utf8'),
);
validatePerformanceMatrix(screenMatrix);
assertNoInheritedNodeOptions({});
assertFormalPerformanceAuthorization(false, undefined);
assertFormalPerformanceAuthorization(true, 'scale-crossover-protocol-amendment-4');
assert.throws(
  () => assertNoInheritedNodeOptions({ NODE_OPTIONS: '--no-warnings' }),
  /NODE_OPTIONS/,
);

const artifactStoreRoot = await mkdtemp(nodePath.join(tmpdir(), 'vue-correctness-store-'));
const runGit = (root, arguments_) => {
  const result = spawnSync('git', ['-C', root, ...arguments_], { encoding: 'utf8' });
  if (result.status !== 0) throw new Error(result.stderr);
};
try {
  runGit(artifactStoreRoot, ['init', '--quiet']);
  runGit(artifactStoreRoot, ['config', 'user.name', 'Verifier']);
  runGit(artifactStoreRoot, ['config', 'user.email', 'verifier@example.com']);
  runGit(artifactStoreRoot, [
    'remote',
    'add',
    'origin',
    `https://${CORRECTNESS_ARTIFACT_REPOSITORY}.git`,
  ]);
  const rawBytes = Buffer.from('{"raw":true}\n');
  const summaryBytes = Buffer.from('{"summary":true}\n');
  const rawSha256 = createHash('sha256').update(rawBytes).digest('hex');
  const summarySha256 = createHash('sha256').update(summaryBytes).digest('hex');
  const artifacts = [{ rawSha256, summarySha256 }];
  const contentSha256 = correctnessArtifactSetAddress(artifacts);
  const rootRelative = `${CORRECTNESS_ARTIFACT_ROOT_PREFIX}/${contentSha256}`;
  const root = nodePath.join(artifactStoreRoot, rootRelative);
  const rawRelative = `raw/${rawSha256}.json`;
  const summaryRelative = `summary/${summarySha256}.json`;
  await mkdir(nodePath.join(root, 'raw'), { recursive: true });
  await mkdir(nodePath.join(root, 'summary'), { recursive: true });
  await writeFile(nodePath.join(root, rawRelative), rawBytes);
  await writeFile(nodePath.join(root, summaryRelative), summaryBytes);
  const manifestPath = nodePath.join(root, 'manifest.json');
  const manifest = {
    schema: 2,
    artifactStore: {
      kind: 'git-head-content-addressed',
      repository: CORRECTNESS_ARTIFACT_REPOSITORY,
      root: rootRelative,
      contentSha256,
    },
    artifacts: [{ raw: rawRelative, summary: summaryRelative, rawSha256, summarySha256 }],
  };
  await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  runGit(artifactStoreRoot, ['add', rootRelative]);
  runGit(artifactStoreRoot, ['commit', '--quiet', '-m', 'add evidence']);
  const acceptedStore = await validateCommittedCorrectnessArtifactStore(manifestPath);
  assert.equal(acceptedStore.contentSha256, contentSha256);

  const untrackedManifestPath = nodePath.join(root, 'untracked-manifest.json');
  await writeFile(untrackedManifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  await assert.rejects(
    validateCommittedCorrectnessArtifactStore(untrackedManifestPath),
    /ls-files --error-unmatch -- .* failed/,
  );

  const cloneRoot = `${artifactStoreRoot}-clone`;
  const clone = spawnSync('git', ['clone', '--quiet', artifactStoreRoot, cloneRoot], {
    encoding: 'utf8',
  });
  if (clone.status !== 0) throw new Error(clone.stderr);
  try {
    runGit(cloneRoot, [
      'remote',
      'set-url',
      'origin',
      `https://${CORRECTNESS_ARTIFACT_REPOSITORY}.git`,
    ]);
    const relocated = await validateCommittedCorrectnessArtifactStore(
      nodePath.join(cloneRoot, rootRelative, 'manifest.json'),
    );
    assert.equal(relocated.contentSha256, contentSha256);
  } finally {
    await rm(cloneRoot, { recursive: true, force: true });
  }

  await writeFile(nodePath.join(root, rawRelative), Buffer.from('{"raw":"drift"}\n'));
  await assert.rejects(
    validateCommittedCorrectnessArtifactStore(manifestPath),
    /differs from the file committed/,
  );
  await writeFile(nodePath.join(root, rawRelative), rawBytes);
  await writeFile(manifestPath, `${JSON.stringify({ ...manifest, schema: 1 }, null, 2)}\n`);
  runGit(artifactStoreRoot, ['add', rootRelative]);
  runGit(artifactStoreRoot, ['commit', '--quiet', '-m', 'add obsolete schema fixture']);
  await assert.rejects(
    validateCommittedCorrectnessArtifactStore(manifestPath),
    /invalid committed-store contract/,
  );
} finally {
  await rm(artifactStoreRoot, { recursive: true, force: true });
}
assert.deepEqual(
  parseMacOsTimeOutput(
    '        1.23 real         2.34 user         0.45 sys\n             1234567  maximum resident set size\n',
  ),
  {
    realToken: '1.23',
    realDecimalPlaces: 2,
    realPrintedResolutionMs: 10,
    realSeconds: 1.23,
    userSeconds: 2.34,
    systemSeconds: 0.45,
    peakRssBytes: 1234567,
  },
);
assert.equal(
  parseMacOsTimeOutput(
    '        1.234 real         2.34 user         0.45 sys\n             1234567  maximum resident set size\n',
  ).realPrintedResolutionMs,
  1,
);
assert.throws(() => parseMacOsTimeOutput('not time output'), /failed to parse/);
assert.throws(
  () => assertFormalPerformanceAuthorization(true, 'wrong-protocol'),
  /outside the Amendment 4 host-gated orchestrator/,
);

const timingFields = (timeRealMs, requestedChildWallMs, timeRealPrecisionMs = 10) => {
  const decimals = Math.log10(1000 / timeRealPrecisionMs);
  const parentWallNs = BigInt(Math.round(requestedChildWallMs * 1e6));
  const childWallMs = Number(parentWallNs) / 1e6;
  return {
    parentWallNs: String(parentWallNs),
    childWallMs,
    timeRealToken: (timeRealMs / 1000).toFixed(decimals),
    timeRealPrecisionMs,
    timeRealMs,
    parentWallOverheadMs: childWallMs - timeRealMs,
  };
};

assert.equal(assertParentWallSanity(timingFields(1000, 990, 10)), true);
assert.throws(
  () => assertParentWallSanity(timingFields(1000, 989, 10)),
  /does not cover canonical/,
);
assert.throws(
  () => assertParentWallSanity(timingFields(1000, 1251, 10)),
  /exceeds the frozen sanity bound/,
);
assert.throws(
  () =>
    assertParentWallSanity({
      ...timingFields(1000, 1010, 10),
      parentWallOverheadMs: 9,
    }),
  /arithmetic mismatch/,
);
assert.throws(
  () => assertParentWallSanity({ ...timingFields(1000, 1010, 10), parentWallNs: '1' }),
  /does not match parentWallNs/,
);
assert.throws(
  () => assertParentWallSanity({ ...timingFields(1000, 1010, 10), timeRealPrecisionMs: undefined }),
  /provenance is incomplete/,
);

assert.throws(
  () => validatePerformanceMatrix({ ...screenMatrix, runtimePin: { sourceCommit: 'drift' } }),
  /provenance differs/,
);
assert.throws(
  () =>
    validatePerformanceMatrix({
      ...screenMatrix,
      cases: screenMatrix.cases.slice(0, 2),
    }),
  /frozen three projects/,
);
assert.throws(
  () =>
    validatePerformanceMatrix({
      ...screenMatrix,
      cases: screenMatrix.cases.map((value, index) =>
        index === 0 ? { ...value, variants: value.variants.slice(0, -1) } : value,
      ),
    }),
  /worker one through eight/,
);

const runs = PROJECT_BANDS.flatMap((project) =>
  SCREEN_VARIANTS.map((variant) => {
    const count = variant === 'ordinary' ? 0 : Number(variant.slice('worker-'.length));
    const timeRealMs =
      count === 0 ? 1990 : count === 4 ? 500 : count < 4 ? 800 - count * 50 : 600 + count * 20;
    const childWallMs =
      count === 0 ? 2050 : count === 3 ? 660 : count === 4 ? 700 : timeRealMs + 10;
    return {
      projectId: project.projectId,
      variant,
      ...timingFields(timeRealMs, childWallMs),
      totalCpuMs: count === 8 ? 250 : 100 + count * 10,
      peakRssBytes: 1000 + count * 50,
      pagingDelta: { pageouts: 0, swapouts: 0 },
      hostAdmission: { phase: 'before-child' },
      postHostAdmission: { phase: 'after-child' },
      canonicalEvidenceSha256: canonicalA,
      blockIndex: 0,
    };
  }),
);
const screenReport = {
  measurementClass: 'formal local wall evidence subject to host gates',
  admitted: true,
  startedAt: 'start',
  finishedAt: 'finish',
  matrix: screenMatrix,
  runtime: { profile: screenMatrix.runtimePin },
  harness: { clean: true },
  correctnessEvidence: {
    admittedProjects: PROJECT_BANDS.map(({ projectId }) => projectId),
    projectCanonicalEvidenceSha256: Object.fromEntries(
      PROJECT_BANDS.map(({ projectId }) => [projectId, canonicalA]),
    ),
  },
  matrixSha256: 'matrix',
  runs,
};
const confirmation = createConfirmationMatrixFromScreen(screenReport, 'a'.repeat(64));
validatePerformanceMatrix(confirmation);
for (const definition of confirmation.cases) {
  assert.equal(definition.selectedScreenWorkerCount, 4);
  assert.deepEqual(definition.variants, [
    'ordinary',
    'worker-3',
    'worker-4',
    'worker-5',
    'worker-8',
  ]);
  assert.equal(definition.repeats, 15);
}
for (const [selectedScreenWorkerCount, variants] of [
  [1, ['ordinary', 'worker-1', 'worker-2', 'worker-4', 'worker-8']],
  [8, ['ordinary', 'worker-4', 'worker-7', 'worker-8']],
]) {
  validatePerformanceMatrix({
    ...confirmation,
    cases: confirmation.cases.map((definition) => ({
      ...definition,
      selectedScreenWorkerCount,
      variants,
    })),
  });
}
assert.throws(
  () =>
    validatePerformanceMatrix({
      ...confirmation,
      cases: confirmation.cases.map((value, index) =>
        index === 2 ? { ...value, variants: ['ordinary', 'worker-4'] } : value,
      ),
    }),
  /best, adjacent, fixed-four, and fixed-eight counts/,
);

const compact = createPerformanceCompactSummary(screenReport, 'raw');
assert.equal(
  compact.classifications.mechanicalPerformanceCrossover.status,
  'not-inferred-from-independent-projects',
);
assert.equal(compact.classifications.productCrossover.status, 'not-established');
assert.equal(compact.projectSummaries[0].bestResourceEnvelopeWorker, 'worker-4');
assert.throws(
  () => createPerformanceCompactSummary({ ...screenReport, runs: runs.slice(1) }, 'raw'),
  /run set is incomplete/,
);
assert.throws(
  () =>
    createPerformanceCompactSummary(
      {
        ...screenReport,
        runs: runs.map((run, index) => (index === 0 ? { ...run, timeRealMs: undefined } : run)),
      },
      'raw',
    ),
  /parent wall provenance is incomplete/,
);
assert.throws(
  () =>
    createPerformanceCompactSummary(
      {
        ...screenReport,
        runs: runs.map((run, index) =>
          index === 0 ? { ...run, canonicalEvidenceSha256: canonicalB } : run,
        ),
      },
      'forged-screen-raw',
    ),
  /differs from committed canonical correctness evidence/,
);

const confirmationRuns = confirmation.cases.flatMap((definition) =>
  Array.from({ length: definition.repeats }, (_, blockIndex) =>
    definition.variants.map((variant) => {
      const count = variant === 'ordinary' ? 0 : Number(variant.slice('worker-'.length));
      const baseTimeReal =
        count === 0 ? 1000 : count === 4 ? 500 : count === 3 ? 550 : count === 8 ? 650 : 530;
      const offset = (blockIndex % 3) * 10;
      const timeRealMs = baseTimeReal + offset;
      const baseParentWall = count === 3 ? 560 : count === 4 ? 700 : baseTimeReal + 20;
      return {
        projectId: definition.projectId,
        variant,
        blockIndex,
        ...timingFields(timeRealMs, baseParentWall + offset),
        totalCpuMs: count === 0 ? 100 : 140,
        peakRssBytes: count === 0 ? 1000 : 1400,
        pagingDelta: { pageouts: 0, swapouts: 0 },
        hostAdmission: { phase: 'before-child' },
        postHostAdmission: { phase: 'after-child' },
        canonicalEvidenceSha256: canonicalA,
      };
    }),
  ).flat(),
);
const confirmationCompact = createPerformanceCompactSummary(
  {
    ...screenReport,
    matrix: confirmation,
    runtime: { profile: confirmation.runtimePin, clean: true },
    runs: confirmationRuns,
  },
  'confirm-raw',
);
assert.equal(confirmationCompact.projectSummaries[0].selectedRepeatedWorker, 'worker-4');
assert.equal(confirmationCompact.projectSummaries[0].mechanicalGain, true);
assert.equal(confirmationCompact.projectSummaries[0].resourceEligible, true);
assert.equal(confirmationCompact.projectSummaries[0].productEligible, false);
assert.equal(confirmationCompact.projectSummaries[0].policyEvidence.selectedOracleWorkerCount, 4);
assert.deepEqual(Object.keys(confirmationCompact.projectSummaries[0].policyEvidence.variants), [
  'ordinary',
  'worker-3',
  'worker-4',
  'worker-5',
  'worker-8',
]);
assert.equal(
  confirmationCompact.projectSummaries[0].policyEvidence.variants['worker-4'].wallMedianMs,
  510,
);
assert.equal(
  confirmationCompact.projectSummaries[0].policyEvidence.variants['worker-4'].cpuMedianMs,
  140,
);
assert.equal(
  confirmationCompact.projectSummaries[0].policyEvidence.variants['worker-4'].peakRssMedianBytes,
  1400,
);
assert.equal(
  confirmationCompact.projectSummaries[0].policyEvidence.variants['worker-4'].resourceEligible,
  true,
);
assert.ok(
  confirmationCompact.projectSummaries[0].policyEvidence.variants['worker-4']
    .pairedWallRatioBootstrap95Upper < 0.53,
);
assert.equal(
  confirmationCompact.projectSummaries[0].policyEvidence.variants.ordinary
    .pairedWallRatioBootstrap95Upper,
  1,
);
assert.equal(
  confirmationCompact.projectSummaries[0].fixedPolicyCandidates.conservativeFixedFour.workerCount,
  4,
);
assert.equal(
  confirmationCompact.projectSummaries[0].fixedPolicyCandidates.hardwareCapFixedEight.workerCount,
  8,
);
assert.throws(
  () =>
    createPerformanceCompactSummary(
      {
        ...screenReport,
        matrix: confirmation,
        runtime: { profile: confirmation.runtimePin, clean: true },
        runs: confirmationRuns.map((run, index) =>
          index === 0 ? { ...run, canonicalEvidenceSha256: canonicalB } : run,
        ),
      },
      'forged-confirmation-raw',
    ),
  /differs from committed canonical correctness evidence/,
);

const noResourceCompact = createPerformanceCompactSummary(
  {
    ...screenReport,
    matrix: confirmation,
    runtime: { profile: confirmation.runtimePin, clean: true },
    runs: confirmationRuns.map((run) => ({
      ...run,
      totalCpuMs: run.variant === 'ordinary' ? run.totalCpuMs : 300,
    })),
  },
  'no-resource-confirm-raw',
);
assert.equal(noResourceCompact.projectSummaries[0].selectedRepeatedWorker, 'worker-4');
assert.equal(noResourceCompact.projectSummaries[0].selectedResourceWorker, null);
assert.equal(noResourceCompact.projectSummaries[0].policyEvidence.selectedOracleWorkerCount, 0);

assert.deepEqual(
  immediateHostFailures({
    acPower: false,
    lowPowerMode: 1,
    noRecordedThermalWarning: false,
    noRecordedPerformanceWarning: false,
    uptimeSeconds: 90_000,
    swapUsedBytes: 600 * 1024 ** 2,
  }),
  [
    'AC power is required',
    'low-power mode must be off',
    'thermal warning is recorded',
    'performance warning is recorded',
    'host uptime exceeds 24 hours',
    'swap exceeds 512 MiB',
  ],
);
assert.deepEqual(
  transientHostFailures({
    oneMinuteLoadAverage: 3,
    summedProcessCpuPercentage: 200,
    memoryFreePercentage: 40,
  }),
  [
    'one-minute load average exceeds 2.0',
    'summed process CPU exceeds 150%',
    'free memory percentage is below 50%',
  ],
);
assert.deepEqual(
  assertNoPagingDelta({ pageouts: 10, swapouts: 20 }, { pageouts: 10, swapouts: 20 }),
  { pageouts: 0, swapouts: 0 },
);
assert.throws(
  () => assertNoPagingDelta({ pageouts: 10, swapouts: 20 }, { pageouts: 11, swapouts: 20 }),
  /paged or swapped/,
);

console.log('independent Vue performance verifier negative tests passed');
