import { createHash } from 'node:crypto';
import {
  BASELINE_POOL_ENVIRONMENT,
  LIFECYCLE_BASELINE,
  REQUIRED_NODE_VERSION,
} from './projects.mjs';

export const PERFORMANCE_PROTOCOL = 'scale-crossover-protocol-amendment-4';
export const SCREEN_VARIANTS = Object.freeze([
  'ordinary',
  'worker-1',
  'worker-2',
  'worker-3',
  'worker-4',
  'worker-5',
  'worker-6',
  'worker-7',
  'worker-8',
]);
export const PROJECT_BANDS = Object.freeze([
  Object.freeze({ projectId: 'floating-vue', band: 'small', expectedReachedSfcCount: 4 }),
  Object.freeze({ projectId: 'cabinet-icon', band: 'medium', expectedReachedSfcCount: 166 }),
  Object.freeze({
    projectId: 'directus-amendment-candidate',
    band: 'large',
    expectedReachedSfcCount: 546,
  }),
]);
export const RESOURCE_LIMITS = Object.freeze({
  medianWallSpeedupAtLeast: 1.1,
  speedupBootstrapLowerAtLeast: 1.05,
  medianTotalProcessCpuRatioAtMost: 2,
  medianPeakRssRatioAtMost: 2,
  absolutePeakRssBelowBytes: 27 * 1024 ** 3,
});
export const PARENT_WALL_SANITY = Object.freeze({
  timeRealPrecisionToleranceMs: 10,
  maximumParentWallOverheadMs: 250,
});
const BOOTSTRAP_RESAMPLES = 100_000;
const BOOTSTRAP_SEED = 0x20260712;

const sha256 = (value) => createHash('sha256').update(value).digest('hex');

export function assertNoInheritedNodeOptions(environment) {
  if (environment.NODE_OPTIONS) {
    throw new Error('formal independent Vue runner refuses inherited NODE_OPTIONS');
  }
}

export function assertFormalPerformanceAuthorization(collectPerformance, protocol) {
  if (collectPerformance && protocol !== PERFORMANCE_PROTOCOL) {
    throw new Error(
      'run-case.mjs refuses collectPerformance outside the Amendment 4 host-gated orchestrator',
    );
  }
}

export function assertPerformanceCorrectnessBinding(run, correctnessEvidence, lane) {
  const expected = correctnessEvidence?.projectCanonicalEvidenceSha256?.[run.projectId];
  if (!/^[a-f0-9]{64}$/.test(expected ?? '')) {
    throw new Error(`${run.projectId} has no committed canonical correctness reference`);
  }
  if (!/^[a-f0-9]{64}$/.test(run.canonicalEvidenceSha256 ?? '')) {
    throw new Error(`${run.projectId}/${run.variant} has no canonical performance evidence`);
  }
  if (run.canonicalEvidenceSha256 !== expected) {
    throw new Error(
      `${lane ?? 'performance'} ${run.projectId}/${run.variant} differs from committed canonical correctness evidence`,
    );
  }
  return true;
}

export function parseMacOsTimeOutput(stderr) {
  const times = stderr.match(/^\s*([0-9.]+)\s+real\s+([0-9.]+)\s+user\s+([0-9.]+)\s+sys$/m);
  const rss = stderr.match(/^\s*(\d+)\s+maximum resident set size$/m);
  if (!times || !rss) throw new Error('failed to parse /usr/bin/time -l output');
  const realToken = times[1];
  const realDecimalPlaces = realToken.includes('.') ? realToken.split('.')[1].length : 0;
  const realPrintedResolutionMs = 1000 / 10 ** realDecimalPlaces;
  return {
    realToken,
    realDecimalPlaces,
    realPrintedResolutionMs,
    realSeconds: Number(realToken),
    userSeconds: Number(times[2]),
    systemSeconds: Number(times[3]),
    peakRssBytes: Number(rss[1]),
  };
}

export function assertParentWallSanity(run) {
  if (
    typeof run.parentWallNs !== 'string' ||
    !/^\d+$/.test(run.parentWallNs) ||
    !Number.isFinite(run.timeRealMs) ||
    run.timeRealMs <= 0 ||
    !Number.isFinite(run.timeRealPrecisionMs) ||
    run.timeRealPrecisionMs <= 0 ||
    run.timeRealPrecisionMs > PARENT_WALL_SANITY.timeRealPrecisionToleranceMs ||
    typeof run.timeRealToken !== 'string' ||
    Number(run.timeRealToken) * 1000 !== run.timeRealMs ||
    1000 / 10 ** (run.timeRealToken.split('.')[1]?.length ?? 0) !== run.timeRealPrecisionMs ||
    !Number.isFinite(run.childWallMs) ||
    run.childWallMs <= 0
  ) {
    throw new Error('formal parent wall provenance is incomplete');
  }
  const parentWallFromNanoseconds = Number(BigInt(run.parentWallNs)) / 1e6;
  if (parentWallFromNanoseconds !== run.childWallMs) {
    throw new Error('parent childWallMs does not match parentWallNs');
  }
  const expectedOverhead = run.childWallMs - run.timeRealMs;
  if (!Object.is(run.parentWallOverheadMs, expectedOverhead)) {
    throw new Error('parentWallOverheadMs arithmetic mismatch');
  }
  if (run.childWallMs + run.timeRealPrecisionMs < run.timeRealMs) {
    throw new Error('parent childWallMs does not cover canonical timeRealMs');
  }
  if (run.parentWallOverheadMs > PARENT_WALL_SANITY.maximumParentWallOverheadMs) {
    throw new Error('parent wall overhead exceeds the frozen sanity bound');
  }
  return true;
}

function expectedRuntimePin() {
  return {
    kind: LIFECYCLE_BASELINE.kind,
    sourceCommit: LIFECYCLE_BASELINE.sourceCommit,
    nativeBindingSha256: LIFECYCLE_BASELINE.nativeBindingSha256,
    distributionSha256: LIFECYCLE_BASELINE.distributionSha256,
  };
}

export function validatePerformanceMatrix(matrix) {
  if (matrix.schema !== 1) throw new Error('performance matrix schema must be 1');
  if (!['independent-vue-wall-screen', 'independent-vue-wall-confirm'].includes(matrix.lane)) {
    throw new Error('invalid independent Vue performance lane');
  }
  if (
    matrix.protocol !== PERFORMANCE_PROTOCOL ||
    matrix.node !== REQUIRED_NODE_VERSION ||
    matrix.directRolldown !== true ||
    matrix.instrumentation !== false ||
    matrix.freshProcessPerVariant !== true ||
    matrix.correctnessEvidenceRequired !== true ||
    JSON.stringify(matrix.runtimePin) !== JSON.stringify(expectedRuntimePin()) ||
    JSON.stringify(matrix.configuredPools) !==
      JSON.stringify({
        tokio: Number(BASELINE_POOL_ENVIRONMENT.ROLLDOWN_WORKER_THREADS),
        rayon: Number(BASELINE_POOL_ENVIRONMENT.RAYON_NUM_THREADS),
        blocking: Number(BASELINE_POOL_ENVIRONMENT.ROLLDOWN_MAX_BLOCKING_THREADS),
      })
  ) {
    throw new Error('performance matrix provenance differs from Amendment 4');
  }
  if (!Array.isArray(matrix.cases) || matrix.cases.length !== PROJECT_BANDS.length) {
    throw new Error('performance matrix must contain the frozen three projects');
  }
  for (const [index, frozen] of PROJECT_BANDS.entries()) {
    const definition = matrix.cases[index];
    if (
      definition.projectId !== frozen.projectId ||
      definition.band !== frozen.band ||
      definition.expectedReachedSfcCount !== frozen.expectedReachedSfcCount ||
      definition.rotationOffset !== index ||
      !Array.isArray(definition.variants) ||
      new Set(definition.variants).size !== definition.variants.length ||
      definition.variants[0] !== 'ordinary' ||
      definition.variants.slice(1).some((variant) => !/^worker-[1-8]$/.test(variant))
    ) {
      throw new Error(`invalid frozen performance case: ${frozen.projectId}`);
    }
    if (matrix.lane === 'independent-vue-wall-screen') {
      if (
        definition.repeats !== 1 ||
        JSON.stringify(definition.variants) !== JSON.stringify(SCREEN_VARIANTS)
      ) {
        throw new Error(
          `screen must cover ordinary and worker one through eight: ${frozen.projectId}`,
        );
      }
    } else {
      if (
        !matrix.generatedFrom ||
        !/^[a-f0-9]{64}$/.test(matrix.generatedFrom.screenRawSha256) ||
        !Number.isInteger(definition.selectedScreenWorkerCount) ||
        definition.selectedScreenWorkerCount < 1 ||
        definition.selectedScreenWorkerCount > 8 ||
        !['resource-envelope-eligible', 'no-resource-envelope-worker'].includes(
          definition.screenSelectionStatus,
        ) ||
        ![10, 15].includes(definition.repeats) ||
        typeof definition.screenBelowTwoSeconds !== 'boolean' ||
        definition.repeats !== (definition.screenBelowTwoSeconds ? 15 : 10)
      ) {
        throw new Error(`invalid confirmation provenance: ${frozen.projectId}`);
      }
      const count = definition.selectedScreenWorkerCount;
      const expectedVariants = confirmationWorkerCounts(count).map((value) => `worker-${value}`);
      if (
        JSON.stringify(definition.variants) !== JSON.stringify(['ordinary', ...expectedVariants])
      ) {
        throw new Error(
          `confirmation must use best, adjacent, fixed-four, and fixed-eight counts: ${frozen.projectId}`,
        );
      }
    }
  }
}

function workerCount(variant) {
  return Number(variant.slice('worker-'.length));
}

function confirmationWorkerCounts(selectedCount) {
  return [...new Set([selectedCount - 1, selectedCount, selectedCount + 1, 4, 8])]
    .filter((value) => value >= 1 && value <= 8)
    .sort((left, right) => left - right);
}

function assertCompleteScreenRuns(report, definition) {
  const runs = report.runs.filter(({ projectId }) => projectId === definition.projectId);
  if (
    runs.length !== SCREEN_VARIANTS.length ||
    JSON.stringify(runs.map(({ variant }) => variant).sort()) !==
      JSON.stringify([...SCREEN_VARIANTS].sort())
  ) {
    throw new Error(`incomplete screen for ${definition.projectId}`);
  }
  return runs;
}

function resourceEnvelopeEligible(run, ordinary) {
  return (
    run.totalCpuMs / ordinary.totalCpuMs <= RESOURCE_LIMITS.medianTotalProcessCpuRatioAtMost &&
    run.peakRssBytes / ordinary.peakRssBytes <= RESOURCE_LIMITS.medianPeakRssRatioAtMost &&
    run.peakRssBytes < RESOURCE_LIMITS.absolutePeakRssBelowBytes &&
    run.pagingDelta.pageouts === 0 &&
    run.pagingDelta.swapouts === 0 &&
    run.canonicalEvidenceSha256 === ordinary.canonicalEvidenceSha256
  );
}

export function createConfirmationMatrixFromScreen(report, screenRawSha256) {
  if (
    report.measurementClass !== 'formal local wall evidence subject to host gates' ||
    report.matrix?.lane !== 'independent-vue-wall-screen' ||
    report.admitted !== true
  ) {
    throw new Error('confirmation requires an admitted formal independent Vue screen');
  }
  assertExactRunSet(report);
  const cases = PROJECT_BANDS.map((frozen, index) => {
    const runs = assertCompleteScreenRuns(report, frozen);
    const ordinary = runs.find(({ variant }) => variant === 'ordinary');
    const eligible = runs.filter(
      (run) => run.variant !== 'ordinary' && resourceEnvelopeEligible(run, ordinary),
    );
    const allWorkers = runs.filter((run) => run.variant !== 'ordinary');
    const screenSelectionStatus =
      eligible.length === 0 ? 'no-resource-envelope-worker' : 'resource-envelope-eligible';
    const selectionPool = eligible.length === 0 ? allWorkers : eligible;
    const best = [...selectionPool].sort(
      (left, right) =>
        left.timeRealMs - right.timeRealMs ||
        workerCount(left.variant) - workerCount(right.variant),
    )[0];
    const count = workerCount(best.variant);
    const variants = confirmationWorkerCounts(count).map((value) => `worker-${value}`);
    const belowTwoSeconds = Math.max(ordinary.timeRealMs, best.timeRealMs) < 2000;
    return {
      projectId: frozen.projectId,
      band: frozen.band,
      expectedReachedSfcCount: frozen.expectedReachedSfcCount,
      variants: ['ordinary', ...variants],
      repeats: belowTwoSeconds ? 15 : 10,
      rotationOffset: index,
      selectedScreenWorkerCount: count,
      screenSelectionStatus,
      screenBelowTwoSeconds: belowTwoSeconds,
    };
  });
  const matrix = {
    schema: 1,
    lane: 'independent-vue-wall-confirm',
    protocol: PERFORMANCE_PROTOCOL,
    node: REQUIRED_NODE_VERSION,
    directRolldown: true,
    instrumentation: false,
    freshProcessPerVariant: true,
    correctnessEvidenceRequired: true,
    runtimePin: expectedRuntimePin(),
    configuredPools: {
      tokio: Number(BASELINE_POOL_ENVIRONMENT.ROLLDOWN_WORKER_THREADS),
      rayon: Number(BASELINE_POOL_ENVIRONMENT.RAYON_NUM_THREADS),
      blocking: Number(BASELINE_POOL_ENVIRONMENT.ROLLDOWN_MAX_BLOCKING_THREADS),
    },
    generatedFrom: {
      screenRawSha256,
      screenStartedAt: report.startedAt,
      screenFinishedAt: report.finishedAt,
    },
    cases,
  };
  validatePerformanceMatrix(matrix);
  return matrix;
}

export function createPerformanceCompactSummary(report, rawArtifactSha256) {
  validatePerformanceMatrix(report.matrix);
  assertExactRunSet(report);
  const base = {
    schema: 1,
    measurementClass: report.measurementClass,
    lane: report.matrix.lane,
    protocol: PERFORMANCE_PROTOCOL,
    rawArtifactSha256,
    runtimePin: report.runtime.profile,
    harness: report.harness,
    adapterToolchain: report.adapterToolchain,
    projectAdapterProvenance: report.projectAdapterProvenance,
    correctnessEvidence: compactEvidence(report.correctnessEvidence),
    screenEvidence: compactScreenEvidence(report.screenEvidence),
    matrixSha256: report.matrixSha256,
    admitted: report.admitted,
    parentWallSanity: PARENT_WALL_SANITY,
    durableEligible:
      report.admitted === true &&
      report.harness?.clean === true &&
      report.runtime?.clean === true &&
      Boolean(report.correctnessEvidence),
  };
  const projectSummaries =
    report.matrix.lane === 'independent-vue-wall-screen'
      ? summarizeScreens(report)
      : summarizeConfirmations(report);
  const classifications = {
    mechanicalPerformanceCrossover: {
      status: 'not-inferred-from-independent-projects',
      reason:
        'Floating Vue, cabinet-fe/icon, and Directus are different non-nested project families; Amendment 4 forbids joining them into a synthetic crossover curve.',
    },
    resourceAcceptablePerformanceCrossover: {
      status: 'not-inferred-from-independent-projects',
      reason: 'The controlled schema-2 Vue corpus is the source of the nested Vue crossover.',
    },
    productCrossover: {
      status: 'not-established',
      reason:
        'The transform-only adapter has no transform source-map correctness or paired diagnostic/failure-semantics gate.',
    },
  };
  const canonical = { ...base, projectSummaries, classifications };
  return {
    ...canonical,
    canonicalSummarySha256: sha256(JSON.stringify(canonical)),
    statisticsProtocol:
      report.matrix.lane === 'independent-vue-wall-confirm'
        ? {
            pairedBy: 'projectId and rotated block index',
            canonicalWall: '/usr/bin/time -l real, recorded as timeRealMs',
            parentWallSanityField: 'childWallMs',
            estimator: 'paired median ordinary timeRealMs / worker timeRealMs',
            bootstrapResamples: BOOTSTRAP_RESAMPLES,
            bootstrapSeed: `0x${BOOTSTRAP_SEED.toString(16)}`,
            interval: 'deterministic percentile bootstrap 95%',
            resourceLimits: RESOURCE_LIMITS,
          }
        : { status: 'one-shot-screen-only; no optimum or crossover claim' },
  };
}

function assertExactRunSet(report) {
  const expected = new Set();
  for (const definition of report.matrix.cases) {
    for (let blockIndex = 0; blockIndex < definition.repeats; blockIndex++) {
      for (const variant of definition.variants) {
        expected.add(`${definition.projectId}\0${variant}\0${blockIndex}`);
      }
    }
  }
  if (report.runs.length !== expected.size) throw new Error('performance run set is incomplete');
  for (const run of report.runs) {
    const key = `${run.projectId}\0${run.variant}\0${run.blockIndex}`;
    if (!expected.delete(key)) throw new Error(`unexpected or duplicate performance run: ${key}`);
    assertParentWallSanity(run);
    assertPerformanceCorrectnessBinding(run, report.correctnessEvidence, report.matrix.lane);
    if (
      !Number.isFinite(run.timeRealMs) ||
      run.timeRealMs <= 0 ||
      !Number.isFinite(run.childWallMs) ||
      run.childWallMs <= 0 ||
      !Number.isFinite(run.totalCpuMs) ||
      run.totalCpuMs < 0 ||
      !Number.isFinite(run.peakRssBytes) ||
      run.peakRssBytes <= 0 ||
      run.pagingDelta?.pageouts !== 0 ||
      run.pagingDelta?.swapouts !== 0 ||
      run.hostAdmission?.phase !== 'before-child' ||
      run.postHostAdmission?.phase !== 'after-child' ||
      !/^[a-f0-9]{64}$/.test(run.canonicalEvidenceSha256)
    ) {
      throw new Error(`ineligible formal performance run: ${key}`);
    }
  }
  if (expected.size !== 0) throw new Error('performance run set is incomplete');
}

function compactEvidence(evidence) {
  if (!evidence) return evidence;
  return {
    manifest: evidence.manifest
      ? {
          bytes: evidence.manifest.bytes,
          sha256: evidence.manifest.sha256,
          repository: evidence.manifest.repository,
          repositoryHead: evidence.manifest.repositoryHead,
          contentSha256: evidence.manifest.contentSha256,
        }
      : undefined,
    artifacts: evidence.artifacts?.map((artifact) => ({
      raw: { bytes: artifact.raw.bytes, sha256: artifact.raw.sha256 },
      summary: { bytes: artifact.summary.bytes, sha256: artifact.summary.sha256 },
      matrixSha256: artifact.matrixSha256,
      goldenSha256: artifact.goldenSha256,
      canonicalSummarySha256: artifact.canonicalSummarySha256,
    })),
    admittedProjects: evidence.admittedProjects,
    projectCanonicalEvidenceSha256: evidence.projectCanonicalEvidenceSha256,
    projectAdapterProvenance: evidence.projectAdapterProvenance,
  };
}

function compactScreenEvidence(evidence) {
  if (!evidence) return evidence;
  return {
    raw: { bytes: evidence.raw.bytes, sha256: evidence.raw.sha256 },
    summary: {
      bytes: evidence.summary.bytes,
      sha256: evidence.summary.sha256,
      canonicalSummarySha256: evidence.summary.canonicalSummarySha256,
    },
  };
}

function summarizeScreens(report) {
  return PROJECT_BANDS.map((frozen) => {
    const runs = assertCompleteScreenRuns(report, frozen);
    const ordinary = runs.find(({ variant }) => variant === 'ordinary');
    const workers = runs
      .filter(({ variant }) => variant !== 'ordinary')
      .map((run) => ({
        variant: run.variant,
        workerCount: workerCount(run.variant),
        timeRealMs: run.timeRealMs,
        parentChildWallMs: run.childWallMs,
        speedup: ordinary.timeRealMs / run.timeRealMs,
        cpuRatio: run.totalCpuMs / ordinary.totalCpuMs,
        rssRatio: run.peakRssBytes / ordinary.peakRssBytes,
        resourceEnvelopeEligible: resourceEnvelopeEligible(run, ordinary),
      }));
    const eligible = workers.filter(({ resourceEnvelopeEligible }) => resourceEnvelopeEligible);
    const best = [...eligible].sort(
      (left, right) => left.timeRealMs - right.timeRealMs || left.workerCount - right.workerCount,
    )[0];
    return {
      projectId: frozen.projectId,
      band: frozen.band,
      reachedSfcCount: frozen.expectedReachedSfcCount,
      ordinaryTimeRealMs: ordinary.timeRealMs,
      ordinaryParentChildWallMs: ordinary.childWallMs,
      screenSelectionStatus:
        eligible.length === 0 ? 'no-resource-envelope-worker' : 'resource-envelope-eligible',
      selectedScreenWorker:
        best?.variant ??
        [...workers].sort(
          (left, right) => left.timeRealMs - right.timeRealMs || left.workerCount - right.workerCount,
        )[0]?.variant,
      bestResourceEnvelopeWorker: best?.variant ?? null,
      screenOnly: true,
      workers,
    };
  });
}

function summarizeConfirmations(report) {
  return PROJECT_BANDS.map((frozen) => {
    const runs = report.runs.filter(({ projectId }) => projectId === frozen.projectId);
    const definition = report.matrix.cases.find(({ projectId }) => projectId === frozen.projectId);
    const ordinaryRuns = runs.filter(({ variant }) => variant === 'ordinary');
    const ordinaryByBlock = new Map(ordinaryRuns.map((run) => [run.blockIndex, run]));
    if (ordinaryByBlock.size !== definition.repeats) {
      throw new Error(`ordinary confirmation blocks are incomplete: ${frozen.projectId}`);
    }
    const workers = definition.variants.slice(1).map((variant) => {
      const variantRuns = runs.filter((run) => run.variant === variant);
      if (variantRuns.length !== definition.repeats) {
        throw new Error(
          `worker confirmation blocks are incomplete: ${frozen.projectId}/${variant}`,
        );
      }
      const paired = variantRuns.map((run) => {
        const ordinary = ordinaryByBlock.get(run.blockIndex);
        return {
          speedup: ordinary.timeRealMs / run.timeRealMs,
          workerToOrdinaryWallRatio: run.timeRealMs / ordinary.timeRealMs,
          cpuRatio: run.totalCpuMs / ordinary.totalCpuMs,
          rssRatio: run.peakRssBytes / ordinary.peakRssBytes,
        };
      });
      const canonicalWalls = variantRuns.map(({ timeRealMs }) => timeRealMs);
      const parentWalls = variantRuns.map(({ childWallMs }) => childWallMs);
      const speedups = paired.map(({ speedup }) => speedup);
      const workerToOrdinaryWallRatios = paired.map(
        ({ workerToOrdinaryWallRatio }) => workerToOrdinaryWallRatio,
      );
      const summary = {
        variant,
        workerCount: workerCount(variant),
        samples: variantRuns.length,
        timeRealMs: statistics(canonicalWalls),
        timeRealMedianBootstrap95: bootstrapMedianInterval(
          canonicalWalls,
          `${frozen.projectId}/${variant}/time-real`,
        ),
        parentChildWallMs: statistics(parentWalls),
        pairedSpeedup: statistics(speedups),
        pairedSpeedupBootstrap95: bootstrapMedianInterval(
          speedups,
          `${frozen.projectId}/${variant}/speedup`,
        ),
        pairedWorkerToOrdinaryWallRatio: statistics(workerToOrdinaryWallRatios),
        pairedWorkerToOrdinaryWallRatioBootstrap95: bootstrapMedianInterval(
          workerToOrdinaryWallRatios,
          `${frozen.projectId}/${variant}/worker-to-ordinary-wall-ratio`,
        ),
        pairedCpuRatio: statistics(paired.map(({ cpuRatio }) => cpuRatio)),
        pairedRssRatio: statistics(paired.map(({ rssRatio }) => rssRatio)),
        totalCpuMs: statistics(variantRuns.map(({ totalCpuMs }) => totalCpuMs)),
        peakRssBytes: statistics(variantRuns.map(({ peakRssBytes }) => peakRssBytes)),
      };
      summary.mechanicalGain = summary.pairedSpeedupBootstrap95.lower > 1;
      summary.resourceEligible =
        summary.pairedSpeedup.median >= RESOURCE_LIMITS.medianWallSpeedupAtLeast &&
        summary.pairedSpeedupBootstrap95.lower >= RESOURCE_LIMITS.speedupBootstrapLowerAtLeast &&
        summary.pairedCpuRatio.median <= RESOURCE_LIMITS.medianTotalProcessCpuRatioAtMost &&
        summary.pairedRssRatio.median <= RESOURCE_LIMITS.medianPeakRssRatioAtMost &&
        summary.peakRssBytes.max < RESOURCE_LIMITS.absolutePeakRssBelowBytes &&
        variantRuns.every(
          ({ pagingDelta }) => pagingDelta.pageouts === 0 && pagingDelta.swapouts === 0,
        );
      summary.productEligible = false;
      return summary;
    });
    const fastest = selectWorkerWithTieRule(workers);
    const resource = workers.filter(({ resourceEligible }) => resourceEligible);
    const fixedFour = workers.find(({ workerCount }) => workerCount === 4);
    const fixedEight = workers.find(({ workerCount }) => workerCount === 8);
    if (!fixedFour || !fixedEight) {
      throw new Error(`fixed policy candidates are incomplete: ${frozen.projectId}`);
    }
    const policyEvidence = createPolicyEvidence(
      ordinaryRuns,
      workers,
      resource.length === 0 ? 0 : selectWorkerWithTieRule(resource).workerCount,
    );
    return {
      projectId: frozen.projectId,
      band: frozen.band,
      reachedSfcCount: frozen.expectedReachedSfcCount,
      screenSelectionStatus: definition.screenSelectionStatus,
      selectedScreenWorkerCount: definition.selectedScreenWorkerCount,
      selectedRepeatedWorker: fastest.variant,
      selectedResourceWorker:
        resource.length === 0 ? null : selectWorkerWithTieRule(resource).variant,
      mechanicalGain: fastest.mechanicalGain,
      resourceEligible: resource.length !== 0,
      productEligible: false,
      productGaps: ['transform-source-map-correctness', 'diagnostic-and-failure-semantics'],
      policyEvidence,
      fixedPolicyCandidates: {
        conservativeFixedFour: {
          workerCount: 4,
          ...policyEvidence.variants['worker-4'],
        },
        hardwareCapFixedEight: {
          workerCount: 8,
          ...policyEvidence.variants['worker-8'],
        },
      },
      workers,
    };
  });
}

function createPolicyEvidence(ordinaryRuns, workers, selectedOracleWorkerCount) {
  const variants = {
    ordinary: {
      wallMedianMs: statistics(ordinaryRuns.map(({ timeRealMs }) => timeRealMs)).median,
      cpuMedianMs: statistics(ordinaryRuns.map(({ totalCpuMs }) => totalCpuMs)).median,
      peakRssMedianBytes: statistics(ordinaryRuns.map(({ peakRssBytes }) => peakRssBytes)).median,
      resourceEligible: true,
      pairedWallRatioBootstrap95Upper: 1,
    },
  };
  for (const worker of workers) {
    variants[worker.variant] = {
      wallMedianMs: worker.timeRealMs.median,
      cpuMedianMs: worker.totalCpuMs.median,
      peakRssMedianBytes: worker.peakRssBytes.median,
      resourceEligible: worker.resourceEligible,
      pairedWallRatioBootstrap95Upper: worker.pairedWorkerToOrdinaryWallRatioBootstrap95.upper,
    };
  }
  return {
    schema: 1,
    canonicalWallField: 'timeRealMs',
    pairedWallRatioDirection: 'worker/ordinary',
    selectedOracleWorkerCount,
    variants,
  };
}

function selectWorkerWithTieRule(workers) {
  const fastest = [...workers].sort(
    (left, right) =>
      left.timeRealMs.median - right.timeRealMs.median || left.workerCount - right.workerCount,
  )[0];
  const lowerEligible = workers
    .filter((candidate) => candidate.workerCount < fastest.workerCount)
    .filter((candidate) => {
      const difference =
        Math.abs(candidate.timeRealMs.median - fastest.timeRealMs.median) /
        fastest.timeRealMs.median;
      const overlap =
        candidate.timeRealMedianBootstrap95.lower <= fastest.timeRealMedianBootstrap95.upper &&
        fastest.timeRealMedianBootstrap95.lower <= candidate.timeRealMedianBootstrap95.upper;
      return difference < 0.02 && overlap;
    })
    .sort((left, right) => left.workerCount - right.workerCount);
  return lowerEligible[0] ?? fastest;
}

function bootstrapMedianInterval(values, label) {
  const random = xorshift32(BOOTSTRAP_SEED ^ hashLabel(label));
  const medians = new Float64Array(BOOTSTRAP_RESAMPLES);
  const sample = Array.from({ length: values.length });
  for (let iteration = 0; iteration < BOOTSTRAP_RESAMPLES; iteration++) {
    for (let index = 0; index < values.length; index++) {
      sample[index] = values[Math.floor(random() * values.length)];
    }
    medians[iteration] = median(sample);
  }
  medians.sort();
  return { lower: quantile(medians, 0.025), upper: quantile(medians, 0.975) };
}

function statistics(values) {
  const sorted = [...values].sort((left, right) => left - right);
  return {
    n: values.length,
    mean: values.reduce((total, value) => total + value, 0) / values.length,
    median: quantile(sorted, 0.5),
    min: sorted[0],
    max: sorted.at(-1),
  };
}

function median(values) {
  return quantile(
    [...values].sort((left, right) => left - right),
    0.5,
  );
}

function quantile(sorted, probability) {
  if (sorted.length === 1) return sorted[0];
  const position = (sorted.length - 1) * probability;
  const lower = Math.floor(position);
  const fraction = position - lower;
  return (
    sorted[lower] + (sorted[Math.min(lower + 1, sorted.length - 1)] - sorted[lower]) * fraction
  );
}

function xorshift32(seed) {
  let state = seed >>> 0 || 0x6d2b79f5;
  return () => {
    state ^= state << 13;
    state ^= state >>> 17;
    state ^= state << 5;
    return (state >>> 0) / 2 ** 32;
  };
}

function hashLabel(value) {
  let hash = 0x811c9dc5;
  for (const byte of Buffer.from(value)) {
    hash ^= byte;
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
}
