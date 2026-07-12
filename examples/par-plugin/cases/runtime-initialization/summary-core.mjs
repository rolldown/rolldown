import { validateFormalInitializationReport } from './admission.mjs';

export { validateFormalInitializationReport };

const compareNumber = (left, right) => left - right;
export const INITIALIZATION_BOOTSTRAP_RESAMPLES = 100_000;
export const INITIALIZATION_BOOTSTRAP_SEED = 0x20260712;

export function deriveInitializationRun(run) {
  const timeline = run.child.timeline;
  const resources = run.child.resources;
  const constructors = timeline.constructors;
  const ready = timeline.ready;
  const poolRequestedEpochMs = Math.min(
    ...constructors.map(({ constructorStartedAt }) => constructorStartedAt.epochMs),
  );
  const constructorReturnedEpochMs = constructors.map(
    ({ constructorReturnedAt }) => constructorReturnedAt.epochMs,
  );
  const onlineEpochMs = Object.values(timeline.online).map(({ epochMs }) => epochMs);
  const entryEpochMs = ready.map(({ timeline: workerTimeline }) => workerTimeline.entryAt.epochMs);
  const importStartedEpochMs = ready.map(
    ({ timeline: workerTimeline }) => workerTimeline.importStartedAt.epochMs,
  );
  const importFinishedEpochMs = ready.map(
    ({ timeline: workerTimeline }) => workerTimeline.importFinishedAt.epochMs,
  );
  const readyEpochMs = ready.map(({ receivedAt }) => receivedAt.epochMs);
  const importDurationsMs = ready.map(
    ({ timeline: workerTimeline }) =>
      workerTimeline.importFinishedAt.epochMs - workerTimeline.importStartedAt.epochMs,
  );
  const workerHeapAfterAllReadyBytes = resources.workerResourcesAfterAllReady.reduce(
    (total, value) => total + value.heapStatistics.used_heap_size,
    0,
  );
  const workerHeapPostGcBytes = resources.workerLocalSnapshots.reduce(
    (total, value) => total + value.heapStatistics.used_heap_size,
    0,
  );
  const preloadProcessCpuMs =
    cpuDifference(
      resources.processBeforeWorkers.cpuUsageMicros,
      resources.processBeforePreload.cpuUsageMicros,
    ) / 1000;
  const poolProcessCpuMs =
    cpuDifference(
      resources.processAtAllReady.cpuUsageMicros,
      resources.processBeforeWorkers.cpuUsageMicros,
    ) / 1000;
  const poolMainCpuMs =
    cpuDifference(
      resources.processAtAllReady.mainThreadCpuUsageMicros,
      resources.processBeforeWorkers.mainThreadCpuUsageMicros,
    ) / 1000;
  const workerResourceCaptureProcessCpuMs =
    cpuDifference(
      resources.processAfterWorkerResourceCapture.cpuUsageMicros,
      resources.processAtAllReady.cpuUsageMicros,
    ) / 1000;
  const preTerminationSnapshotProcessCpuMs =
    cpuDifference(
      resources.processBeforeTermination.cpuUsageMicros,
      resources.processAfterWorkerResourceCapture.cpuUsageMicros,
    ) / 1000;
  const terminationProcessCpuMs =
    cpuDifference(
      resources.processAfterTerminationBeforePostGc.cpuUsageMicros,
      resources.processBeforeTermination.cpuUsageMicros,
    ) / 1000;
  const postGcProcessCpuMs =
    cpuDifference(
      resources.processAfterPostGc.cpuUsageMicros,
      resources.processAfterTerminationBeforePostGc.cpuUsageMicros,
    ) / 1000;
  return {
    sequence: run.sequence,
    block: run.block,
    name: run.name,
    workerCount: run.workerCount,
    preloadWallMs: timeline.preload
      ? timeline.preload.finishedAt.epochMs - timeline.preload.startedAt.epochMs
      : 0,
    constructorDispatchMs: Math.max(...constructorReturnedEpochMs) - poolRequestedEpochMs,
    firstOnlineMs: Math.min(...onlineEpochMs) - poolRequestedEpochMs,
    allOnlineMs: Math.max(...onlineEpochMs) - poolRequestedEpochMs,
    firstEntryMs: Math.min(...entryEpochMs) - poolRequestedEpochMs,
    allEntryMs: Math.max(...entryEpochMs) - poolRequestedEpochMs,
    firstReadyMs: Math.min(...readyEpochMs) - poolRequestedEpochMs,
    allReadyMs: Math.max(...readyEpochMs) - poolRequestedEpochMs,
    readySkewMs: Math.max(...readyEpochMs) - Math.min(...readyEpochMs),
    importCriticalPathMs: Math.max(...importFinishedEpochMs) - Math.min(...importStartedEpochMs),
    importPerWorkerMedianMs: quantile(importDurationsMs, 0.5),
    importPerWorkerP95Ms: quantile(importDurationsMs, 0.95),
    importAggregateWorkerMs: importDurationsMs.reduce((total, value) => total + value, 0),
    terminationWallMs:
      timeline.terminationFinishedAt.epochMs - timeline.terminationStartedAt.epochMs,
    processCpuMs: sumCpu(resources.processCpuMicros) / 1000,
    mainCpuMs: sumCpu(resources.mainCpuMicros) / 1000,
    workerCpuAfterAllReadyMs: sumCpu(resources.workerCpuAfterAllReadyMicros) / 1000,
    workerCpuBeforeTerminationMs: sumCpu(resources.workerCpuBeforeTerminationMicros) / 1000,
    residualCpuMs: resources.residualCpuMicros / 1000,
    preloadProcessCpuMs,
    poolProcessCpuMs,
    poolMainCpuMs,
    workerResourceCaptureProcessCpuMs,
    preTerminationSnapshotProcessCpuMs,
    terminationProcessCpuMs,
    postGcProcessCpuMs,
    preloadRssDeltaBytes:
      resources.processBeforeWorkers.memoryUsageBytes.rss -
      resources.processBeforePreload.memoryUsageBytes.rss,
    readyRssDeltaBytes:
      resources.processAtAllReady.memoryUsageBytes.rss -
      resources.processBeforeWorkers.memoryUsageBytes.rss,
    retainedPostGcRssDeltaBytes:
      resources.processAfterPostGc.memoryUsageBytes.rss -
      resources.processBeforeWorkers.memoryUsageBytes.rss,
    peakRssDeltaBytes: run.peakRssBytes - resources.processBeforeWorkers.memoryUsageBytes.rss,
    workerHeapAfterAllReadyBytes,
    workerHeapPostGcBytes,
    moduleInit: run.moduleInit,
  };
}

export function summarizeInitializationReport(report, { rawArtifact } = {}) {
  validateFormalInitializationReport(report);
  if (
    typeof rawArtifact?.path !== 'string' ||
    !Number.isSafeInteger(rawArtifact.bytes) ||
    rawArtifact.bytes < 1 ||
    !/^[a-f0-9]{64}$/.test(rawArtifact.sha256 ?? '')
  ) {
    throw new Error('initialization summary requires the exact raw artifact path, bytes, and hash');
  }
  const derivedRuns = report.runs.map(deriveInitializationRun);
  const groups = Map.groupBy(derivedRuns, (run) => `${run.name}\0${run.workerCount}`);
  const cases = [...groups.entries()]
    .map(([key, runs]) => {
      const [name, workerCount] = key.split('\0');
      const metrics = {};
      for (const metric of metricNames) {
        const values = runs.map((run) => run[metric]);
        metrics[metric] = {
          median: quantile(values, 0.5),
          p95: quantile(values, 0.95),
          minimum: Math.min(...values),
          maximum: Math.max(...values),
        };
      }
      return { name, workerCount: Number(workerCount), samples: runs.length, metrics };
    })
    .sort(
      (left, right) => left.workerCount - right.workerCount || left.name.localeCompare(right.name),
    );
  const counts = [...new Set(cases.map(({ workerCount }) => workerCount))].sort(compareNumber);
  const contrasts = counts.map((workerCount) => ({
    workerCount,
    bindingImportOverRetainedRuntime: pairedContrast(
      groups.get(`binding-worker-import\0${workerCount}`),
      groups.get(`retained-runtime-worker\0${workerCount}`),
      `worker-${workerCount}/binding-import-over-retained-runtime`,
    ),
    packageImportOverRetainedPackage: pairedContrast(
      groups.get(`package-worker-import\0${workerCount}`),
      groups.get(`retained-package-worker\0${workerCount}`),
      `worker-${workerCount}/package-import-over-retained-package`,
    ),
    packageLayerOverBindingImport: pairedDifferenceInDifferences(
      groups.get(`package-worker-import\0${workerCount}`),
      groups.get(`retained-package-worker\0${workerCount}`),
      groups.get(`binding-worker-import\0${workerCount}`),
      groups.get(`retained-runtime-worker\0${workerCount}`),
      `worker-${workerCount}/package-layer-over-binding-import`,
    ),
  }));
  return {
    schemaVersion: 2,
    kind: 'rolldown-runtime-initialization-summary',
    measurementClass:
      'formal local initialization attribution; instrumented elapsed values are not wall benchmark evidence',
    source: {
      harnessManifestSha256: report.harnessProvenance.sourceManifest.aggregateSha256,
      runtimeCommit: report.runtimeProvenance.worktree.commit,
      bindingSha256: report.runtimeProvenance.binding.sha256,
      distributionSha256: report.runtimeProvenance.distribution.aggregateSha256,
      packageEntrySha256: report.runtimeProvenance.packageEntry.sha256,
      nodeArtifact: {
        version: report.runtimeProvenance.node.version,
        bytes: report.runtimeProvenance.node.bytes,
        sha256: report.runtimeProvenance.node.sha256,
      },
      packageEnvironment: report.runtimeProvenance.packageEnvironment,
      rawArtifact,
      rawRuns: report.runs.length,
      repeats: report.matrix.repeats,
    },
    stageDefinitions: {
      preloadWallMs: 'parent binding or package import before any Worker constructor',
      constructorDispatchMs: 'first Worker constructor start through last constructor return',
      firstOnlineMs: 'first Worker constructor start through first online event',
      allOnlineMs: 'first Worker constructor start through last online event',
      firstEntryMs: 'first Worker constructor start through first worker-entry statement',
      allEntryMs: 'first Worker constructor start through last worker-entry statement',
      firstReadyMs: 'first Worker constructor start through first ready message received',
      allReadyMs: 'first Worker constructor start through last ready message received',
      importCriticalPathMs: 'earliest per-worker dynamic import start through latest import finish',
      importAggregateWorkerMs:
        'sum of overlapping per-worker dynamic import elapsed; not critical-path wall or CPU',
      residualCpuMs:
        'process CPU minus measured Node main and worker CPU; includes native/runtime/helper work and error',
      poolProcessCpuMs:
        'aggregate process CPU from Worker construction through the immediate all-ready snapshot',
      workerCpuAfterAllReadyMs:
        'sum of worker CPU queried immediately after all-ready; query completion is later than the all-ready process snapshot',
      workerCpuBeforeTerminationMs:
        'sum of worker-local CPU after explicit worker GC and before termination',
      retainedPostGcRssDeltaBytes:
        'whole-process post-termination/post-GC RSS minus RSS immediately before Worker construction',
    },
    cases,
    contrasts,
    moduleInitialization: summarizeModuleInitialization(derivedRuns),
    interpretationLimits: [
      'The empty/binding/package controls do not measure a plugin factory, plugin configuration, lifecycle hooks, bindingification, native registration, or first transforms.',
      'Whole-process RSS contrasts do not assign retained pages to a specific isolate or native subsystem.',
      'Worker imports overlap; aggregate per-worker elapsed is not CPU and is never added to critical-path wall.',
      'All-ready process CPU is exact at its boundary; per-worker CPU is queried just after that boundary and is reported separately rather than subtracted from it.',
      'Module-initialization thread callbacks are immediate construction/registration scheduling snapshots, not a quiesced retained-thread count.',
      'Current Vue and MDX raw traces supply worker implementation import, factory, bindingification, registration, readiness, and per-transform service timestamps; they do not yet supply matching ordinary registration or the real worker static-binding boundary.',
      'Per-transform traces can derive observed first-1/2/4/8/16/32 and later service, but source complexity prevents naming a cold-to-steady difference as JIT without additional evidence.',
      'This control does not observe GC events, page faults, or I/O by initialization stage.',
    ],
  };
}

const metricNames = [
  'preloadWallMs',
  'constructorDispatchMs',
  'firstOnlineMs',
  'allOnlineMs',
  'firstEntryMs',
  'allEntryMs',
  'firstReadyMs',
  'allReadyMs',
  'readySkewMs',
  'importCriticalPathMs',
  'importPerWorkerMedianMs',
  'importPerWorkerP95Ms',
  'importAggregateWorkerMs',
  'terminationWallMs',
  'processCpuMs',
  'mainCpuMs',
  'workerCpuAfterAllReadyMs',
  'workerCpuBeforeTerminationMs',
  'residualCpuMs',
  'preloadProcessCpuMs',
  'poolProcessCpuMs',
  'poolMainCpuMs',
  'workerResourceCaptureProcessCpuMs',
  'preTerminationSnapshotProcessCpuMs',
  'terminationProcessCpuMs',
  'postGcProcessCpuMs',
  'preloadRssDeltaBytes',
  'readyRssDeltaBytes',
  'retainedPostGcRssDeltaBytes',
  'peakRssDeltaBytes',
  'workerHeapAfterAllReadyBytes',
  'workerHeapPostGcBytes',
];

function pairedContrast(upper, lower, label) {
  if (!upper || !lower) return null;
  const lowerByBlock = new Map(lower.map((run) => [run.block, run]));
  if (lowerByBlock.size !== lower.length || upper.length !== lower.length) {
    throw new Error('initialization contrast blocks are incomplete');
  }
  const differences = upper.map((run) => {
    const baseline = lowerByBlock.get(run.block);
    if (!baseline) throw new Error(`initialization contrast lacks block ${run.block}`);
    return Object.fromEntries(
      metricNames.map((metric) => [metric, run[metric] - baseline[metric]]),
    );
  });
  return {
    pairing: 'same rotated block',
    estimator: 'upper minus lower',
    samples: differences.length,
    metrics: summarizeDifferenceMetrics(differences, label),
  };
}

function pairedDifferenceInDifferences(
  packageImport,
  retainedPackage,
  bindingImport,
  retainedRuntime,
  label,
) {
  const groups = [packageImport, retainedPackage, bindingImport, retainedRuntime];
  if (groups.some((group) => !group)) return null;
  const byBlock = groups.map((group) => new Map(group.map((run) => [run.block, run])));
  if (byBlock.some((group, index) => group.size !== groups[index].length)) {
    throw new Error('initialization difference-in-differences blocks are incomplete');
  }
  const differences = packageImport.map((packageRun) => {
    const block = packageRun.block;
    const retainedPackageRun = byBlock[1].get(block);
    const bindingRun = byBlock[2].get(block);
    const retainedRuntimeRun = byBlock[3].get(block);
    if (!retainedPackageRun || !bindingRun || !retainedRuntimeRun) {
      throw new Error(`initialization difference-in-differences lacks block ${block}`);
    }
    return Object.fromEntries(
      metricNames.map((metric) => [
        metric,
        packageRun[metric] -
          retainedPackageRun[metric] -
          (bindingRun[metric] - retainedRuntimeRun[metric]),
      ]),
    );
  });
  return {
    pairing: 'same order-balanced rotated block',
    estimator:
      '(package worker import - retained package worker) - (binding worker import - retained runtime worker)',
    samples: differences.length,
    metrics: summarizeDifferenceMetrics(differences, label),
  };
}

function summarizeDifferenceMetrics(differences, label) {
  return Object.fromEntries(
    metricNames.map((metric) => {
      const values = differences.map((value) => value[metric]);
      return [
        metric,
        {
          median: quantile(values, 0.5),
          p95: quantile(values, 0.95),
          minimum: Math.min(...values),
          maximum: Math.max(...values),
          medianBootstrap95: bootstrapMedianInterval(values, `${label}/${metric}`),
        },
      ];
    }),
  );
}

function summarizeModuleInitialization(runs) {
  const groups = Map.groupBy(runs, (run) => `${run.name}\0${run.workerCount}`);
  return [...groups.entries()]
    .map(([key, groupedRuns]) => {
      const [name, workerCount] = key.split('\0');
      const records = groupedRuns.flatMap(({ moduleInit }) => moduleInit);
      return {
        name,
        workerCount: Number(workerCount),
        observation:
          'immediate runtime-build and custom-runtime-registration callback snapshots; not a quiesced retained-thread count',
        runs: groupedRuns.length,
        records: records.length,
        recordsPerRun: [...new Set(groupedRuns.map(({ moduleInit }) => moduleInit.length))].sort(
          compareNumber,
        ),
        configuredTokioWorkerThreads: [
          ...new Set(
            records.map(({ configuredTokioWorkerThreads }) => configuredTokioWorkerThreads),
          ),
        ].sort(compareNumber),
        configuredTokioMaxBlockingThreads: [
          ...new Set(
            records.map(
              ({ configuredTokioMaxBlockingThreads }) => configuredTokioMaxBlockingThreads,
            ),
          ),
        ].sort(compareNumber),
        threadsStartedAfterBuild: [
          ...new Set(records.map(({ threadsStartedAfterBuild }) => threadsStartedAfterBuild)),
        ].sort(compareNumber),
        threadsStartedAfterRegistration: [
          ...new Set(
            records.map(({ threadsStartedAfterRegistration }) => threadsStartedAfterRegistration),
          ),
        ].sort(compareNumber),
        threadsStoppedAfterBuild: [
          ...new Set(records.map(({ threadsStoppedAfterBuild }) => threadsStoppedAfterBuild)),
        ].sort(compareNumber),
        threadsStoppedAfterRegistration: [
          ...new Set(
            records.map(({ threadsStoppedAfterRegistration }) => threadsStoppedAfterRegistration),
          ),
        ].sort(compareNumber),
        durations:
          records.length === 0
            ? null
            : Object.fromEntries(
                ['runtimeBuildMs', 'customRuntimeRegistrationMs', 'totalMs'].map((metric) => [
                  metric,
                  statistics(records.map((record) => record[metric])),
                ]),
              ),
      };
    })
    .sort(
      (left, right) => left.workerCount - right.workerCount || left.name.localeCompare(right.name),
    );
}

export function bootstrapMedianInterval(values, label) {
  if (!Array.isArray(values) || values.length === 0) {
    throw new Error('initialization bootstrap requires values');
  }
  const random = xorshift32(INITIALIZATION_BOOTSTRAP_SEED ^ hashLabel(label));
  const medians = new Float64Array(INITIALIZATION_BOOTSTRAP_RESAMPLES);
  const sample = Array.from({ length: values.length });
  for (let iteration = 0; iteration < INITIALIZATION_BOOTSTRAP_RESAMPLES; iteration++) {
    for (let index = 0; index < values.length; index++) {
      sample[index] = values[Math.floor(random() * values.length)];
    }
    medians[iteration] = quantile(sample, 0.5);
  }
  medians.sort();
  const sortedMedians = [...medians];
  return {
    lower: quantile(sortedMedians, 0.025),
    upper: quantile(sortedMedians, 0.975),
  };
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

function statistics(values) {
  return {
    samples: values.length,
    median: quantile(values, 0.5),
    p95: quantile(values, 0.95),
    minimum: Math.min(...values),
    maximum: Math.max(...values),
  };
}

function sumCpu(value) {
  if (!Number.isFinite(value?.user) || !Number.isFinite(value?.system)) {
    throw new Error('CPU value is incomplete');
  }
  return value.user + value.system;
}

function cpuDifference(after, before) {
  return sumCpu(after) - sumCpu(before);
}

export function quantile(values, probability) {
  if (!Array.isArray(values) || values.length === 0) throw new Error('quantile needs values');
  const sorted = [...values].sort(compareNumber);
  const index = (sorted.length - 1) * probability;
  const lower = Math.floor(index);
  const upper = Math.ceil(index);
  if (lower === upper) return sorted[lower];
  return sorted[lower] + (sorted[upper] - sorted[lower]) * (index - lower);
}
