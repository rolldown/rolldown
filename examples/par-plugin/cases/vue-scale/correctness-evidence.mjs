import nodePath from 'node:path';
import {
  FROZEN_SELECTIONS,
  selectManifestEntries,
  summarizeSelection,
  summarizeSelectionInput,
} from './corpus.mjs';
import { hashBytes } from './harness-provenance.mjs';
import {
  LIFECYCLE_BASELINE_DISTRIBUTION_SHA256,
  LIFECYCLE_BASELINE_NATIVE_BINDING_SHA256,
  LIFECYCLE_BASELINE_SOURCE_COMMIT,
} from './provenance.mjs';

export const OUTPUT_GOLDEN_FIELDS = [
  'outputRawCodeHash',
  'outputCodeHash',
  'outputRawMapHash',
  'outputMapHash',
  'outputCodeBytes',
  'outputMapBytes',
  'outputChunkCount',
  'outputAssetCount',
  'totalExports',
];
export const PORTABLE_OUTPUT_GOLDEN_FIELDS = OUTPUT_GOLDEN_FIELDS.filter(
  (field) => field !== 'outputRawCodeHash' && field !== 'outputRawMapHash',
);

const forbiddenRunKeys = [
  'totalElapsedMs',
  'pluginSetupElapsedMs',
  'rolldownApiElapsedMs',
  'generateElapsedMs',
  'closeElapsedMs',
  'cpuUserMs',
  'cpuSystemMs',
  'finalRssBytes',
  'peakRssBytes',
  'pagingDelta',
  'hostAdmission',
  'postHostAdmission',
];

export async function validateCorrectnessReport(
  report,
  { harnessSourceManifest, vueToolchain, manifest, corpusDirectory },
) {
  if (
    report?.schema !== 1 ||
    report.matrix?.lane !== 'correctness-smoke' ||
    report.measurementClass !== 'untimed correctness; not performance evidence' ||
    report.admitted !== true ||
    report.admissionFailures?.length !== 0 ||
    report.runtime?.runtimePin?.sourceCommit !== LIFECYCLE_BASELINE_SOURCE_COMMIT ||
    report.runtime.runtimePin.nativeBindingSha256 !== LIFECYCLE_BASELINE_NATIVE_BINDING_SHA256 ||
    report.runtime.runtimePin.distributionSha256 !== LIFECYCLE_BASELINE_DISTRIBUTION_SHA256 ||
    report.runtime.worktreeStatus !== '' ||
    report.fixture?.worktreeStatus !== '' ||
    report.harnessSourceManifest?.aggregateSha256 !== harnessSourceManifest.aggregateSha256 ||
    JSON.stringify(report.vueToolchain) !== JSON.stringify(vueToolchain) ||
    report.executionEnvironment?.inheritedNodeOptions !== null
  ) {
    throw new Error('raw Vue correctness report is not current clean lifecycle evidence');
  }
  const scales = Object.keys(FROZEN_SELECTIONS).map(Number);
  if (
    JSON.stringify(report.caseSelections?.map(({ componentCount }) => componentCount)) !==
    JSON.stringify(scales)
  ) {
    throw new Error('Vue correctness report does not cover every frozen scale once');
  }
  const goldens = {};
  for (const componentCount of scales) {
    const selectedEntries = selectManifestEntries(manifest, componentCount);
    const expectedSelection = summarizeSelection(selectedEntries);
    const expectedInput = await summarizeSelectionInput(selectedEntries, corpusDirectory);
    const selection = report.caseSelections.find(
      (candidate) => candidate.componentCount === componentCount,
    );
    if (
      !selection ||
      !sameFields(selection, expectedSelection) ||
      JSON.stringify(selection.input) !== JSON.stringify(expectedInput)
    ) {
      throw new Error(`Vue correctness selection differs at ${componentCount}`);
    }
    const runs = report.runs.filter((run) => run.componentCount === componentCount);
    const expectedVariants =
      componentCount === scales.at(-1)
        ? ['ordinary', 'worker-1', 'worker-4', 'worker-8']
        : ['ordinary'];
    if (JSON.stringify(runs.map(({ variant }) => variant)) !== JSON.stringify(expectedVariants)) {
      throw new Error(`Vue correctness variants are incomplete at ${componentCount}`);
    }
    for (const run of runs) {
      validateCorrectnessRun(run, expectedInput);
    }
    for (const field of OUTPUT_GOLDEN_FIELDS) {
      if (new Set(runs.map((run) => run[field])).size !== 1) {
        throw new Error(`Vue correctness ${field} differs at ${componentCount}`);
      }
    }
    goldens[componentCount] = {
      selection: expectedSelection,
      input: expectedInput,
      output: Object.fromEntries(OUTPUT_GOLDEN_FIELDS.map((field) => [field, runs[0][field]])),
    };
  }
  return goldens;
}

function validateCorrectnessRun(run, expectedInput) {
  const workerCount = run.variant === 'ordinary' ? 0 : Number(run.variant.slice('worker-'.length));
  if (
    forbiddenRunKeys.some((key) => Object.hasOwn(run, key)) ||
    run.measurementClass !== 'correctness-only' ||
    run.instrumentation !== true ||
    run.auditSources !== true ||
    run.selectedSourceBytes !== expectedInput.bytes ||
    run.sourceAudit?.distinctIds !== run.componentCount ||
    run.sourceAudit.calls !== run.componentCount ||
    run.sourceAudit.inputBytes !== expectedInput.bytes ||
    run.sourceAudit.inputAggregateSha256 !== expectedInput.aggregateSha256 ||
    run.sourceAudit.exactOnceSha256 !== expectedInput.exactOnceSha256 ||
    run.jsMetrics?.handlerCalls !== run.componentCount ||
    run.jsMetrics.handlerInputCodeBytes !== expectedInput.bytes ||
    run.jsMetrics.perWorkerCalls.length !== Math.max(1, workerCount) ||
    run.jsMetrics.perWorkerCalls.some((calls) => calls < 1) ||
    run.transformTimeline?.records?.length !== run.componentCount ||
    run.totalExports !== run.componentCount ||
    !Number.isSafeInteger(run.outputCodeBytes) ||
    run.outputCodeBytes < 1 ||
    !Number.isSafeInteger(run.outputMapBytes) ||
    run.outputMapBytes < 1 ||
    !Number.isSafeInteger(run.outputChunkCount) ||
    run.outputChunkCount < 1 ||
    !Number.isSafeInteger(run.outputAssetCount) ||
    run.outputAssetCount < 0 ||
    ![run.outputRawCodeHash, run.outputCodeHash, run.outputRawMapHash, run.outputMapHash].every(
      (value) => /^[a-f0-9]{64}$/.test(value),
    )
  ) {
    throw new Error(`correctness run ${run.variant} failed independent source/output validation`);
  }
  if (workerCount === 0) {
    if (run.rustMetrics || run.initializationMetrics || run.terminationMetrics) {
      throw new Error('ordinary correctness run unexpectedly emitted worker metrics');
    }
    return;
  }
  const rust = run.rustMetrics;
  if (
    rust?.workerCount !== workerCount ||
    rust.wrapperCalls !== run.componentCount + 3 ||
    rust.permitAcquiredCalls !== rust.wrapperCalls ||
    rust.completedWrapperCalls !== rust.wrapperCalls ||
    rust.valueResults !== run.componentCount ||
    rust.nullResults !== 3 ||
    rust.errorResults !== 0 ||
    rust.cancelledBeforeAcquire !== 0 ||
    rust.cancelledDuringService !== 0 ||
    rust.permitQueuePending?.current !== 0 ||
    rust.wrapperOutstanding?.current !== 0 ||
    rust.permitInFlight?.current !== 0 ||
    rust.permitInFlight.max > workerCount ||
    run.initializationMetrics?.kind !== 'rolldown_parallel_plugin_init_metrics' ||
    run.initializationMetrics.version !== 1 ||
    run.initializationMetrics?.workerCount !== workerCount ||
    run.initializationMetrics.pluginCount !== 1 ||
    run.initializationMetrics.workers?.length !== workerCount ||
    run.terminationMetrics?.kind !== 'rolldown_parallel_plugin_termination_metrics' ||
    run.terminationMetrics.version !== 1 ||
    run.terminationMetrics.workerCount !== workerCount
  ) {
    throw new Error(`correctness run ${run.variant} failed Rust queue/error/lifecycle validation`);
  }
}

export function createCompactCorrectnessEvidence(
  rawContent,
  report,
  rawPath,
  compactPath,
  goldens,
) {
  return {
    schema: 2,
    kind: 'vue-scale-correctness-evidence-pointer',
    passed: true,
    measurementClass: report.measurementClass,
    raw: {
      path: relativeEvidencePath(compactPath, rawPath),
      bytes: rawContent.byteLength,
      sha256: hashBytes(rawContent),
    },
    harnessSourceManifest: {
      files: report.harnessSourceManifest.files,
      bytes: report.harnessSourceManifest.bytes,
      aggregateSha256: report.harnessSourceManifest.aggregateSha256,
    },
    vueToolchain: report.vueToolchain,
    fixtureCommit: report.fixture.commit,
    runtimePin: report.runtime.runtimePin,
    corpusAggregateSha256: report.corpus.summary.aggregateSha256,
    goldens,
  };
}

function relativeEvidencePath(compactPath, rawPath) {
  return nodePath
    .relative(nodePath.dirname(nodePath.resolve(compactPath)), nodePath.resolve(rawPath))
    .split(nodePath.sep)
    .join('/');
}

function sameFields(actual, expected) {
  return Object.entries(expected).every(
    ([key, value]) => JSON.stringify(actual[key]) === JSON.stringify(value),
  );
}
