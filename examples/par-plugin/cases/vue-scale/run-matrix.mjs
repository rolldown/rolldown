import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readFile, realpath, rm, writeFile } from 'node:fs/promises';
import { cpus, platform, release, totalmem } from 'node:os';
import nodePath from 'node:path';
import {
  validateAttributionLifecycle as validateAttributionLifecycleStrict,
  validateBindingModuleInit as validateBindingModuleInitStrict,
  validateRustTimeline as validateRustTimelineStrict,
} from './attribution-validation.mjs';
import {
  FROZEN_SELECTIONS,
  listUnexpectedPreparedFiles,
  readCorpusManifest,
  selectManifestEntries,
  selectionHash,
  summarizeSelectionInput,
  summarizeSelection,
  verifyPreparedCorpus,
} from './corpus.mjs';
import {
  admitFormalHost,
  admitFormalHostAfterChild,
  assertNoPagingDelta,
  virtualMemoryCounters,
} from './host-policy.mjs';
import {
  CANONICAL_EVIDENCE_PATHS,
  validateOutputAgainstGolden,
  verifyFormalEvidence,
} from './evidence-verifier.mjs';
import { captureHarnessSourceManifest } from './harness-provenance.mjs';
import { captureVueToolchainProvenance } from './toolchain-provenance.mjs';
import {
  ATTRIBUTION_DISTRIBUTION_SHA256,
  ATTRIBUTION_NATIVE_BINDING_SHA256,
  ATTRIBUTION_SOURCE_COMMIT,
  BASELINE_POOL_ENVIRONMENT,
  EXPECTED_DISTRIBUTION_SHA256,
  EXPECTED_NATIVE_BINDING_SHA256,
  LIFECYCLE_BASELINE_DISTRIBUTION_SHA256,
  LIFECYCLE_BASELINE_NATIVE_BINDING_SHA256,
  LIFECYCLE_BASELINE_SOURCE_COMMIT,
  RUNTIME_SOURCE_COMMIT,
  assertLocalExecution,
  assertRuntimeStable,
  inspectRuntimeProvenance,
} from './provenance.mjs';

assertLocalExecution();
const matrixPath = process.argv[2];
if (!matrixPath) throw new Error('expected a matrix JSON path');
const outputPath = process.argv[3];
const validateOnly = process.argv.includes('--validate-only');
const matrix = JSON.parse(await readFile(matrixPath, 'utf8'));
validateMatrix(matrix);
const harnessSourceManifest = await captureHarnessSourceManifest();
const vueToolchain = await captureVueToolchainProvenance();

const repositoryRoot = nodePath.resolve(import.meta.dirname, '../../../..');
const runtimePackageRoot = nodePath.resolve(
  process.argv[4] ?? nodePath.join(repositoryRoot, 'packages/rolldown'),
);
const runtimeRepositoryRoot = nodePath.resolve(runtimePackageRoot, '../..');
const performanceLane = matrix.lane === 'wall-screen' || matrix.lane === 'wall-confirm';
const formalAttributionLane = matrix.lane === 'instrumented-attribution';
const attributionLane = formalAttributionLane || matrix.lane === 'attribution-contract-smoke';
const evidenceCreationLane = matrix.lane === 'correctness-smoke';
const fixtureStatus = git(repositoryRoot, ['status', '--short']);
if (
  !validateOnly &&
  (performanceLane || formalAttributionLane || evidenceCreationLane) &&
  fixtureStatus
) {
  throw new Error(
    'formal Vue scale timing, attribution, and correctness evidence require a clean fixture worktree',
  );
}
const runtime = await inspectRuntimeProvenance(runtimeRepositoryRoot, runtimePackageRoot, {
  requireClean: !validateOnly && (performanceLane || formalAttributionLane || evidenceCreationLane),
  expectedPin: matrix.runtimePin,
});
const manifestPath = nodePath.join(import.meta.dirname, 'corpus-manifest.json');
const corpusDirectory = nodePath.join(import.meta.dirname, '.corpus');
const resolvedCorpusDirectory = await realpath(corpusDirectory);
const manifest = await readCorpusManifest(manifestPath);
await verifyPreparedCorpus({ corpusDirectory, manifest });
const unexpectedCorpusFiles = await listUnexpectedPreparedFiles(corpusDirectory, manifest);
if (unexpectedCorpusFiles.length !== 0) {
  throw new Error(`prepared Vue corpus has unexpected files: ${unexpectedCorpusFiles.join(', ')}`);
}
const formalEvidence =
  !validateOnly && (performanceLane || formalAttributionLane)
    ? await verifyFormalEvidence({
        repositoryRoot,
        harnessSourceManifest,
        vueToolchain,
        manifest,
        corpusDirectory: resolvedCorpusDirectory,
      })
    : undefined;
if (formalEvidence && matrix.lane === 'wall-confirm') {
  await validateGeneratedMatrixPins(matrix, {
    harnessSourceManifest,
    vueToolchain,
    manifest,
    runtime,
    formalEvidence,
  });
}
if (validateOnly) {
  await assertRuntimeStable(runtimeRepositoryRoot, runtimePackageRoot, runtime);
  console.log(
    JSON.stringify({
      validatedOnly: true,
      lane: matrix.lane,
      cases: matrix.cases.length,
      runtimePin: matrix.runtimePin,
      harnessAggregateSha256: harnessSourceManifest.aggregateSha256,
      vueToolchain,
      corpusAggregateSha256: manifest.summary.aggregateSha256,
      maximumFrozenScale: Math.max(...Object.keys(FROZEN_SELECTIONS).map(Number)),
      fixtureWorktreeClean: fixtureStatus === '',
    }),
  );
  process.exit(0);
}

const runs = [];
const admissionFailures = [];
const caseSelections = [];
const hostAdmissions = [];
let sequence = 0;
const startedAt = new Date().toISOString();

for (const definition of matrix.cases) {
  const {
    name,
    componentCount,
    variants,
    repeats,
    rotationOffset = 0,
    instrumentation,
    auditSources,
  } = definition;
  if (caseSelections.some((selection) => selection.name === name)) {
    throw new Error(`duplicate matrix case name: ${name}`);
  }
  const selectedEntries = selectManifestEntries(manifest, componentCount);
  const selectedHash = selectionHash(selectedEntries);
  if (selectedHash !== FROZEN_SELECTIONS[componentCount]) {
    throw new Error(`frozen selection hash mismatch for ${name}`);
  }
  const selectionSummary = summarizeSelection(selectedEntries);
  const selectionInput = await summarizeSelectionInput(selectedEntries, corpusDirectory);
  const evidenceGolden = formalEvidence?.goldens?.[componentCount];
  if (
    evidenceGolden &&
    (JSON.stringify(evidenceGolden.selection) !== JSON.stringify(selectionSummary) ||
      JSON.stringify(evidenceGolden.input) !== JSON.stringify(selectionInput))
  ) {
    throw new Error(`formal Vue selection differs from correctness evidence at ${componentCount}`);
  }
  if (formalEvidence && !evidenceGolden) {
    throw new Error(`formal Vue scale has no correctness golden: ${componentCount}`);
  }
  caseSelections.push({ name, componentCount, ...selectionSummary, input: selectionInput });

  const caseDirectory = nodePath.join(
    import.meta.dirname,
    '.results',
    'case-work',
    String(componentCount),
  );
  await rm(caseDirectory, { recursive: true, force: true });
  await mkdir(caseDirectory, { recursive: true });
  const caseRunStart = runs.length;
  try {
    const entryPath = nodePath.join(caseDirectory, 'entry.js');
    const selectionPath = nodePath.join(caseDirectory, 'selection.json');
    await writeFile(selectionPath, `${JSON.stringify(selectedEntries)}\n`);
    await writeFile(
      entryPath,
      `${selectedEntries
        .map(
          (entry, index) =>
            `export { default as component_${String(index).padStart(4, '0')} } from ${JSON.stringify(nodePath.join(corpusDirectory, entry.sourceKey))};`,
        )
        .join('\n')}\n`,
    );
    const caseOptions = {
      componentCount,
      instrumentation,
      auditSources,
      collectPerformance:
        matrix.lane !== 'correctness-smoke' && matrix.lane !== 'attribution-contract-smoke',
      _corpusDirectory: corpusDirectory,
      _resolvedCorpusDirectory: resolvedCorpusDirectory,
      _entryPath: entryPath,
      _selectionPath: selectionPath,
      _selectedSourceBytes: selectionSummary.bytes,
      _selectedInputSha256: selectionInput.aggregateSha256,
      _sourceAuditExactOnceSha256: selectionInput.exactOnceSha256,
      _selectionHash: selectedHash,
    };
    let admitted = true;
    for (let index = 0; index < repeats && admitted; index++) {
      const offset = (rotationOffset + index) % variants.length;
      const order = [...variants.slice(offset), ...variants.slice(0, offset)];
      for (const variant of order) {
        const admission = performanceLane ? await admitFormalHost() : undefined;
        if (admission) hostAdmissions.push({ name, variant, index, ...admission });
        const run = execute({ name, caseOptions, variant, index, admission });
        runs.push({ sequence: sequence++, ...run });
        if (run.admissionFailure) {
          admissionFailures.push({ name, variant, index, ...run.admissionFailure });
          admitted = false;
          break;
        }
      }
    }
    if (!admitted) continue;
    for (const field of [
      'outputRawCodeHash',
      'outputCodeHash',
      'outputRawMapHash',
      'outputMapHash',
      'outputCodeBytes',
      'outputMapBytes',
      'outputChunkCount',
      'outputAssetCount',
      'totalExports',
    ]) {
      const values = new Set(runs.slice(caseRunStart).map((run) => run[field]));
      if (values.size !== 1) {
        throw new Error(`${name} produced different ${field} values: ${[...values].join(', ')}`);
      }
    }
  } finally {
    await rm(caseDirectory, { recursive: true, force: true });
  }
}

await assertRuntimeStable(runtimeRepositoryRoot, runtimePackageRoot, runtime);
if (
  JSON.stringify(await captureHarnessSourceManifest()) !== JSON.stringify(harnessSourceManifest)
) {
  throw new Error('Vue scale harness sources changed during matrix execution');
}
if (git(repositoryRoot, ['status', '--short']) !== fixtureStatus) {
  throw new Error('fixture worktree changed during Vue scale matrix');
}
const report = {
  schema: 1,
  measurementClass:
    matrix.lane === 'correctness-smoke'
      ? 'untimed correctness; not performance evidence'
      : matrix.lane === 'attribution-contract-smoke'
        ? 'untimed attribution contract validation; not performance evidence'
        : performanceLane
          ? 'formal local wall evidence subject to host gates'
          : 'instrumented attribution; wall values are not performance evidence',
  startedAt,
  finishedAt: new Date().toISOString(),
  runtime,
  harnessSourceManifest,
  vueToolchain,
  evidence: formalEvidence
    ? { admission: formalEvidence.admission, correctness: formalEvidence.correctness }
    : undefined,
  fixture: {
    repositoryRoot,
    commit: git(repositoryRoot, ['rev-parse', 'HEAD']),
    worktreeStatus: fixtureStatus,
  },
  host: {
    platform: platform(),
    release: release(),
    architecture: process.arch,
    cpuModel: cpus()[0]?.model,
    logicalCpuCount: cpus().length,
    totalMemoryBytes: totalmem(),
  },
  hostAdmissions,
  admitted: admissionFailures.length === 0,
  admissionFailures,
  corpus: {
    compiler: manifest.compiler,
    repositories: manifest.repositories,
    eligibility: manifest.eligibility,
    summary: manifest.summary,
    selections: manifest.selections,
  },
  matrix,
  executionEnvironment: {
    inheritedNodeOptions: null,
    childNodeOptions: `--import=${nodePath.join(import.meta.dirname, 'register-loader.mjs')}`,
  },
  caseSelections,
  runs,
};
const serialized = `${JSON.stringify(report, null, 2)}\n`;
if (outputPath) {
  await mkdir(nodePath.dirname(nodePath.resolve(outputPath)), { recursive: true });
  await writeFile(outputPath, serialized);
  console.log(
    JSON.stringify({
      outputPath,
      lane: matrix.lane,
      cases: matrix.cases.length,
      runs: runs.length,
      startedAt,
      finishedAt: report.finishedAt,
    }),
  );
} else {
  process.stdout.write(serialized);
}

function execute({ name, caseOptions, variant, index, admission }) {
  const options = { ...caseOptions, variant };
  const environment = { ...process.env, ...BASELINE_POOL_ENVIRONMENT };
  environment.ROLLDOWN_RESEARCH_PACKAGE_ROOT = runtimePackageRoot;
  const loaderOption = `--import=${nodePath.join(import.meta.dirname, 'register-loader.mjs')}`;
  environment.NODE_OPTIONS = loaderOption;
  delete environment.ROLLDOWN_PARALLEL_PLUGIN_WORKERS;
  delete environment.ROLLDOWN_PARALLEL_PLUGIN_METRICS;
  const workerMatch = /^worker-(\d+)$/.exec(variant);
  if (workerMatch) {
    environment.ROLLDOWN_PARALLEL_PLUGIN_WORKERS = workerMatch[1];
  }
  if (options.instrumentation && attributionLane) {
    environment.ROLLDOWN_PARALLEL_PLUGIN_METRICS = 'json';
  } else if (options.instrumentation && workerMatch) {
    environment.ROLLDOWN_PARALLEL_PLUGIN_METRICS = 'json';
  }
  const childArguments = [
    process.execPath,
    '--expose-gc',
    nodePath.join(import.meta.dirname, 'run-case.mjs'),
    JSON.stringify(options),
  ];
  const timed = options.collectPerformance;
  const beforeVm = timed ? virtualMemoryCounters() : undefined;
  const result = timed
    ? spawnSync('/usr/bin/time', ['-l', ...childArguments], {
        encoding: 'utf8',
        env: environment,
        maxBuffer: 64 * 1024 * 1024,
      })
    : spawnSync(childArguments[0], childArguments.slice(1), {
        encoding: 'utf8',
        env: environment,
        maxBuffer: 64 * 1024 * 1024,
      });
  const afterVm = timed ? virtualMemoryCounters() : undefined;
  if (result.error) {
    throw new Error(
      `failed to spawn Vue scale child for ${name}/${variant}: ${result.error.code ?? result.error.message}`,
    );
  }
  if (result.status !== 0) {
    if (matrix.lane === 'correctness-smoke') {
      return {
        name,
        variant,
        index,
        admissionFailure: {
          status: result.status,
          signal: result.signal,
          stderrBytes: Buffer.byteLength(result.stderr),
          stderrSha256: createHash('sha256').update(result.stderr).digest('hex'),
          stderr: result.stderr,
        },
      };
    }
    throw new Error(
      `Vue scale child failed for ${name}/${variant} with status ${result.status}:\n${result.stderr}`,
    );
  }
  const pagingDelta = timed ? assertNoPagingDelta(beforeVm, afterVm) : undefined;
  const postHostAdmission = performanceLane ? admitFormalHostAfterChild() : undefined;
  const peakRssMatch = timed ? result.stderr.match(/(\d+)\s+maximum resident set size/) : undefined;
  if (timed && !peakRssMatch) throw new Error('failed to parse child peak RSS');
  const rustMatches = [
    ...result.stderr.matchAll(/^\[rolldown-parallel-plugin-metrics\] (\{.*\})$/gm),
  ];
  const lifecycle = [
    ...result.stderr.matchAll(
      /^\[rolldown-parallel-plugin-(?:init|termination)-metrics\] (\{.*\})$/gm,
    ),
  ].map((match) => JSON.parse(match[1]));
  const moduleInit = [
    ...result.stderr.matchAll(/^\[rolldown-parallel-plugin-module-init-metrics\] (\{.*\})$/gm),
  ].map((match) => JSON.parse(match[1]));
  const expectedRustMetrics = options.instrumentation && workerMatch ? 1 : 0;
  if (rustMatches.length !== expectedRustMetrics || lifecycle.length !== expectedRustMetrics * 2) {
    throw new Error(`unexpected instrumentation lines for ${name}/${variant}`);
  }
  const expectedModuleInitMetrics = attributionLane && options.instrumentation ? 1 : 0;
  if (moduleInit.length !== expectedModuleInitMetrics) {
    throw new Error(`unexpected binding module-initialization metrics for ${name}/${variant}`);
  }
  const child = JSON.parse(result.stdout);
  const rustMetrics = rustMatches[0] ? JSON.parse(rustMatches[0][1]) : undefined;
  const initializationMetrics = lifecycle.find(
    ({ kind }) => kind === 'rolldown_parallel_plugin_init_metrics',
  );
  const terminationMetrics = lifecycle.find(
    ({ kind }) => kind === 'rolldown_parallel_plugin_termination_metrics',
  );
  validateRun(
    child,
    rustMetrics,
    initializationMetrics,
    terminationMetrics,
    moduleInit[0],
    workerMatch ? Number(workerMatch[1]) : 0,
    options,
  );
  if (formalEvidence) {
    validateOutputAgainstGolden(child, formalEvidence.goldens[options.componentCount]);
  }
  return {
    name,
    variant,
    index,
    configuredPools: {
      tokio: Number(environment.ROLLDOWN_WORKER_THREADS),
      rayon: Number(environment.RAYON_NUM_THREADS),
      blocking: Number(environment.ROLLDOWN_MAX_BLOCKING_THREADS),
      javascriptWorkers: workerMatch ? Number(workerMatch[1]) : 0,
    },
    hostAdmission: admission,
    postHostAdmission,
    pagingDelta,
    peakRssBytes: peakRssMatch ? Number(peakRssMatch[1]) : undefined,
    ...child,
    rustMetrics,
    initializationMetrics,
    terminationMetrics,
    bindingModuleInitMetrics: moduleInit[0],
  };
}

function validateRun(run, rust, initialization, termination, moduleInit, workerCount, options) {
  if (run.totalExports !== run.componentCount) throw new Error('output export count mismatch');
  if (run.outputChunkCount < 1 || run.outputMapBytes < 1) {
    throw new Error('Vue scale output code or source map is missing');
  }
  if (run.auditSources) {
    if (
      !run.sourceAudit ||
      run.sourceAudit.distinctIds !== run.componentCount ||
      run.sourceAudit.calls !== run.componentCount ||
      run.sourceAudit.inputBytes !== run.selectedSourceBytes ||
      run.sourceAudit.inputAggregateSha256 !== options._selectedInputSha256 ||
      run.sourceAudit.exactOnceSha256 !== options._sourceAuditExactOnceSha256
    ) {
      throw new Error('exact-source audit failed');
    }
  } else if (run.sourceAudit) {
    throw new Error('source audit was emitted while disabled');
  }
  if (!run.instrumentation) {
    if (
      run.jsMetrics ||
      run.clockAnchors ||
      run.transformTimeline ||
      rust ||
      initialization ||
      termination
    ) {
      throw new Error('uninstrumented Vue scale run emitted metrics');
    }
    if (moduleInit) throw new Error('uninstrumented Vue scale run emitted module-init metrics');
    return;
  }
  const js = run.jsMetrics;
  if (!js) throw new Error('instrumented Vue scale run omitted JavaScript metrics');
  if (
    js.handlerCalls !== run.expectedMatchingHandlerCalls ||
    js.handlerInputCodeBytes !== run.selectedSourceBytes ||
    js.handlerActive !== 0 ||
    js.factoryCalls !== Math.max(1, workerCount) ||
    js.buildStartCalls !== Math.max(1, workerCount) ||
    js.maxHandlerActive > Math.max(1, workerCount) ||
    js.perWorkerCalls.length !== Math.max(1, workerCount) ||
    js.perWorkerCalls.some((calls) => calls < 1) ||
    js.perWorkerCalls.reduce((total, value) => total + value, 0) !== js.handlerCalls
  ) {
    throw new Error('JavaScript Vue scale metrics failed validation');
  }
  const expectedMask = ((1n << BigInt(Math.max(1, workerCount))) - 1n).toString(16);
  if (js.workerMask !== expectedMask) throw new Error('worker factory mask mismatch');
  if (
    run.transformTimeline?.clock?.source !== 'process.hrtime.bigint()' ||
    run.transformTimeline.records.length !== run.componentCount
  ) {
    throw new Error('Vue transform kernel timeline is missing or incomplete');
  }
  for (const anchor of [run.clockAnchors?.beforePlugin, run.clockAnchors?.afterBuild]) {
    if (
      !anchor ||
      anchor.epochAfterMs < anchor.epochBeforeMs ||
      anchor.epochBracketWidthMs !== anchor.epochAfterMs - anchor.epochBeforeMs ||
      anchor.estimateUncertaintyMs !== anchor.epochBracketWidthMs / 2 ||
      BigInt(anchor.hrtimeNs) <= 0n
    ) {
      throw new Error('invalid hrtime-to-epoch clock anchor');
    }
  }
  for (const [ordinal, record] of run.transformTimeline.records.entries()) {
    const startedAt = BigInt(record.kernelStartedAtNs);
    const finishedAt = BigInt(record.kernelFinishedAtNs);
    if (
      record.ordinal !== ordinal ||
      record.calls !== 1 ||
      record.workerNumber < 0 ||
      record.workerNumber >= Math.max(1, workerCount) ||
      startedAt <= 0n ||
      finishedAt < startedAt ||
      BigInt(record.kernelDurationNs) !== finishedAt - startedAt
    ) {
      throw new Error(`invalid Vue transform kernel timeline record at ordinal ${ordinal}`);
    }
  }
  const timelineCallsByWorker = Array.from({ length: Math.max(1, workerCount) }, () => 0);
  for (const record of run.transformTimeline.records) timelineCallsByWorker[record.workerNumber]++;
  if (JSON.stringify(timelineCallsByWorker) !== JSON.stringify(js.perWorkerCalls)) {
    throw new Error('Vue transform timeline and per-worker counters disagree');
  }
  if (attributionLane) validateBindingModuleInitStrict(moduleInit, BASELINE_POOL_ENVIRONMENT);
  else if (moduleInit) throw new Error('non-attribution run emitted binding module-init metrics');
  if (workerCount === 0) {
    if (js.maxHandlerActive !== 1) throw new Error('ordinary handler concurrency is not one');
    return;
  }
  if (!rust || !initialization || !termination) {
    throw new Error('parallel Vue scale instrumentation is incomplete');
  }
  if (
    initialization.workerCount !== workerCount ||
    initialization.workers.length !== workerCount ||
    initialization.pluginCount !== 1 ||
    termination.workerCount !== workerCount ||
    rust.workerCount !== workerCount
  ) {
    throw new Error('worker lifecycle count mismatch');
  }
  if (
    rust.wrapperCalls !== rust.permitAcquiredCalls ||
    rust.wrapperCalls !== rust.completedWrapperCalls ||
    rust.valueResults !== js.handlerCalls ||
    rust.nullResults !== rust.wrapperCalls - js.handlerCalls ||
    rust.returnedCodeBytes !== js.handlerReturnedCodeBytes ||
    rust.wrapperInputCodeBytes < js.handlerInputCodeBytes ||
    rust.errorResults !== 0 ||
    rust.cancelledBeforeAcquire !== 0 ||
    rust.cancelledDuringService !== 0 ||
    rust.wrapperCalls !== run.componentCount + 3 ||
    rust.permitQueuePending.current !== 0 ||
    rust.wrapperOutstanding.current !== 0 ||
    rust.permitInFlight.current !== 0 ||
    rust.permitInFlight.max > workerCount
  ) {
    throw new Error('Rust Vue scale metrics failed validation');
  }
  if (attributionLane) {
    validateAttributionLifecycleStrict(initialization, termination, workerCount);
    validateRustTimelineStrict(rust, run, options, workerCount);
  }
}

async function validateGeneratedMatrixPins(
  matrixValue,
  {
    harnessSourceManifest,
    vueToolchain: toolchain,
    manifest: corpusManifest,
    runtime: runtimeValue,
    formalEvidence: evidence,
  },
) {
  const expectedLock = {
    harnessAggregateSha256: harnessSourceManifest.aggregateSha256,
    vueToolchain: toolchain,
    corpusAggregateSha256: corpusManifest.summary.aggregateSha256,
    runtimePin: runtimeValue.runtimePin,
    evidence: { admission: evidence.admission, correctness: evidence.correctness },
  };
  if (JSON.stringify(matrixValue.provenanceLock) !== JSON.stringify(expectedLock)) {
    throw new Error(
      'generated Vue confirmation matrix provenance differs from the current evidence',
    );
  }
  const generatedFrom = matrixValue.generatedFrom;
  const sourcePath = generatedFrom?.path ?? generatedFrom?.priorConfirmationPath;
  const sourceSha256 = generatedFrom?.sha256 ?? generatedFrom?.priorConfirmationSha256;
  if (typeof sourcePath !== 'string' || !/^[a-f0-9]{64}$/.test(sourceSha256 ?? '')) {
    throw new Error('generated Vue confirmation matrix omits its source report pin');
  }
  const sourceContent = await readFile(sourcePath);
  if (createHash('sha256').update(sourceContent).digest('hex') !== sourceSha256) {
    throw new Error('generated Vue confirmation source report differs from its pinned hash');
  }
}

function validateMatrix(value) {
  const lanes = new Set([
    'correctness-smoke',
    'wall-screen',
    'wall-confirm',
    'instrumented-attribution',
    'attribution-contract-smoke',
  ]);
  if (
    value.schema !== 1 ||
    !lanes.has(value.lane) ||
    value.bindingProfile !== 'release' ||
    !Array.isArray(value.cases) ||
    JSON.stringify(value.configuredPools) !== JSON.stringify({ tokio: 18, rayon: 12, blocking: 4 })
  ) {
    throw new Error('invalid frozen Vue scale matrix header');
  }
  if (
    ['wall-screen', 'wall-confirm', 'instrumented-attribution'].includes(value.lane) &&
    JSON.stringify(value.requiredEvidence) !== JSON.stringify(CANONICAL_EVIDENCE_PATHS)
  ) {
    throw new Error('formal Vue matrices must consume the canonical committed evidence pointers');
  }
  if (value.lane === 'wall-confirm') {
    const sourceHash = value.generatedFrom?.sha256 ?? value.generatedFrom?.priorConfirmationSha256;
    const sourcePath = value.generatedFrom?.path ?? value.generatedFrom?.priorConfirmationPath;
    if (
      !/^[a-f0-9]{64}$/.test(value.provenanceLock?.harnessAggregateSha256 ?? '') ||
      !/^[a-f0-9]{64}$/.test(value.provenanceLock?.corpusAggregateSha256 ?? '') ||
      typeof value.provenanceLock?.vueToolchain !== 'object' ||
      typeof value.provenanceLock?.runtimePin !== 'object' ||
      typeof value.provenanceLock?.evidence?.admission !== 'object' ||
      typeof value.provenanceLock?.evidence?.correctness !== 'object' ||
      typeof sourcePath !== 'string' ||
      !/^[a-f0-9]{64}$/.test(sourceHash ?? '')
    ) {
      throw new Error('generated Vue confirmation matrix omits provenance or source-report pins');
    }
  }
  const pin = value.runtimePin;
  if (
    !pin ||
    !['historical-0aa', 'lifecycle-corrected-baseline', 'instrumented-research'].includes(
      pin.kind,
    ) ||
    !/^[a-f0-9]{40}$/.test(pin.sourceCommit) ||
    !/^[a-f0-9]{64}$/.test(pin.nativeBindingSha256) ||
    !/^[a-f0-9]{64}$/.test(pin.distributionSha256)
  ) {
    throw new Error(
      'matrix runtimePin must contain a concrete source commit, native binding hash, and distribution hash',
    );
  }
  if (
    pin.kind === 'historical-0aa' &&
    (pin.sourceCommit !== RUNTIME_SOURCE_COMMIT ||
      pin.nativeBindingSha256 !== EXPECTED_NATIVE_BINDING_SHA256 ||
      pin.distributionSha256 !== EXPECTED_DISTRIBUTION_SHA256)
  ) {
    throw new Error('historical-0aa matrix does not match the retained historical artifacts');
  }
  if (
    pin.kind === 'lifecycle-corrected-baseline' &&
    (pin.sourceCommit !== LIFECYCLE_BASELINE_SOURCE_COMMIT ||
      pin.nativeBindingSha256 !== LIFECYCLE_BASELINE_NATIVE_BINDING_SHA256 ||
      pin.distributionSha256 !== LIFECYCLE_BASELINE_DISTRIBUTION_SHA256)
  ) {
    throw new Error('lifecycle-corrected matrix does not match the frozen baseline artifacts');
  }
  if (
    pin.kind === 'instrumented-research' &&
    (pin.sourceCommit !== ATTRIBUTION_SOURCE_COMMIT ||
      pin.nativeBindingSha256 !== ATTRIBUTION_NATIVE_BINDING_SHA256 ||
      pin.distributionSha256 !== ATTRIBUTION_DISTRIBUTION_SHA256)
  ) {
    throw new Error('instrumented matrix does not match the frozen attribution artifacts');
  }
  if (
    (value.lane === 'instrumented-attribution' || value.lane === 'attribution-contract-smoke') &&
    pin.kind !== 'instrumented-research'
  ) {
    throw new Error('instrumented attribution requires an explicit instrumented-research pin');
  }
  if (
    value.lane !== 'instrumented-attribution' &&
    value.lane !== 'attribution-contract-smoke' &&
    pin.kind !== 'lifecycle-corrected-baseline'
  ) {
    throw new Error('correctness and wall lanes require the lifecycle-corrected baseline pin');
  }
  const expectedBaseScales = Object.keys(FROZEN_SELECTIONS).map(Number);
  const expectedAllVariants = [
    'ordinary',
    'worker-1',
    'worker-2',
    'worker-3',
    'worker-4',
    'worker-5',
    'worker-6',
    'worker-7',
    'worker-8',
  ];
  for (const [definitionIndex, definition] of value.cases.entries()) {
    if (
      typeof definition.name !== 'string' ||
      !Object.hasOwn(FROZEN_SELECTIONS, definition.componentCount) ||
      !Array.isArray(definition.variants) ||
      definition.variants.length === 0 ||
      new Set(definition.variants).size !== definition.variants.length ||
      definition.variants.some(
        (variant) => variant !== 'ordinary' && !/^worker-[1-8]$/.test(variant),
      ) ||
      !Number.isSafeInteger(definition.repeats) ||
      definition.repeats < 1 ||
      !Number.isSafeInteger(definition.rotationOffset ?? 0) ||
      typeof definition.instrumentation !== 'boolean' ||
      typeof definition.auditSources !== 'boolean'
    ) {
      throw new Error(`invalid Vue scale matrix case: ${JSON.stringify(definition)}`);
    }
    if (
      value.lane === 'correctness-smoke' &&
      (definition.repeats !== 1 ||
        definition.rotationOffset !== 0 ||
        !definition.instrumentation ||
        !definition.auditSources)
    ) {
      throw new Error('correctness smoke must be one audited instrumented full-corpus pass');
    }
    if (
      (value.lane === 'wall-screen' || value.lane === 'wall-confirm') &&
      (definition.instrumentation || definition.auditSources)
    ) {
      throw new Error('wall matrices must disable instrumentation and source auditing');
    }
    if (value.lane === 'wall-screen') {
      if (
        definition.repeats !== 1 ||
        definition.rotationOffset !== definitionIndex ||
        JSON.stringify(definition.variants) !== JSON.stringify(expectedAllVariants)
      ) {
        throw new Error('wall screen must contain one ordinary plus worker-1..8 pass');
      }
    }
    if (
      value.lane === 'instrumented-attribution' &&
      (!definition.instrumentation || !definition.auditSources)
    ) {
      throw new Error('instrumented attribution must enable metrics and exact-source audit');
    }
    if (
      value.lane === 'instrumented-attribution' &&
      (definition.repeats !== 1 ||
        definition.rotationOffset !== definitionIndex ||
        JSON.stringify(definition.variants) !== JSON.stringify(expectedAllVariants))
    ) {
      throw new Error('instrumented attribution must cover ordinary plus worker-1..8 once');
    }
    if (
      value.lane === 'attribution-contract-smoke' &&
      (definition.componentCount !== Math.min(...expectedBaseScales) ||
        definition.repeats !== 1 ||
        definition.rotationOffset !== 0 ||
        !definition.instrumentation ||
        !definition.auditSources ||
        JSON.stringify(definition.variants) !== JSON.stringify(['ordinary', 'worker-4']))
    ) {
      throw new Error('attribution contract smoke must be one audited ordinary/worker-4 pass');
    }
  }
  if (
    (value.lane === 'wall-screen' || value.lane === 'instrumented-attribution') &&
    JSON.stringify(value.cases.map(({ componentCount }) => componentCount)) !==
      JSON.stringify(expectedBaseScales)
  ) {
    throw new Error(`${value.lane} must cover every frozen scale exactly once`);
  }
  if (
    value.lane === 'correctness-smoke' &&
    (JSON.stringify(value.cases.map(({ componentCount }) => componentCount)) !==
      JSON.stringify(expectedBaseScales) ||
      value.cases
        .slice(0, -1)
        .some(({ variants }) => JSON.stringify(variants) !== JSON.stringify(['ordinary'])) ||
      JSON.stringify(value.cases.at(-1).variants) !==
        JSON.stringify(['ordinary', 'worker-1', 'worker-4', 'worker-8']))
  ) {
    throw new Error(
      'correctness smoke must cover ordinary once at every scale and worker-1/4/8 at full scale',
    );
  }
  if (value.lane === 'attribution-contract-smoke' && value.cases.length !== 1) {
    throw new Error('attribution contract smoke must contain exactly one case');
  }
}

function git(root, arguments_) {
  const result = spawnSync('git', ['-C', root, ...arguments_], { encoding: 'utf8' });
  if (result.status !== 0) throw new Error(`git ${arguments_.join(' ')} failed`);
  return result.stdout.trim();
}
