import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { mkdir, mkdtemp, readFile, readdir, rm, stat, writeFile } from 'node:fs/promises';
import { cpus, platform, release, totalmem, tmpdir } from 'node:os';
import nodePath from 'node:path';
import {
  readCorpusManifest,
  selectManifestEntries,
  selectionHash,
  verifyPreparedCorpus,
} from './corpus.mjs';

if (process.version !== 'v24.18.0') {
  throw new Error(`Svelte matrix requires Node.js v24.18.0, got ${process.version}`);
}
const matrixPath = process.argv[2];
if (!matrixPath) throw new Error('expected a matrix JSON path');
const outputPath = process.argv[3];
const matrix = JSON.parse(await readFile(matrixPath, 'utf8'));
if (!Array.isArray(matrix.cases)) throw new Error('matrix.cases must be an array');
if (matrix.bindingProfile !== 'release') {
  throw new Error('matrix.bindingProfile must be "release"');
}
const repositoryRoot = nodePath.resolve(import.meta.dirname, '../../../..');
const bindingDirectory = nodePath.join(repositoryRoot, 'packages/rolldown/src');
const bindingFileNames = (await readdir(bindingDirectory)).filter((name) =>
  /^rolldown-binding\..+\.node$/.test(name),
);
if (bindingFileNames.length !== 1) {
  throw new Error(`expected one local native binding, got ${bindingFileNames.length}`);
}
const bindingPath = nodePath.join(bindingDirectory, bindingFileNames[0]);
const bindingContent = await readFile(bindingPath);
const bindingStat = await stat(bindingPath);
const manifestPath = nodePath.join(import.meta.dirname, 'corpus-manifest.json');
const corpusDirectory = nodePath.join(import.meta.dirname, '.corpus');
const manifest = await readCorpusManifest(manifestPath);
await verifyPreparedCorpus({ corpusDirectory, manifest });

const gitCommit = spawnSync('git', ['-C', repositoryRoot, 'rev-parse', 'HEAD'], {
  encoding: 'utf8',
});
if (gitCommit.status !== 0) throw new Error('failed to identify the Rolldown commit');
const gitStatus = spawnSync('git', ['-C', repositoryRoot, 'status', '--short'], {
  encoding: 'utf8',
});
if (gitStatus.status !== 0) throw new Error('failed to inspect the Rolldown worktree');

const runs = [];
const caseSelections = [];
let sequence = 0;
const startedAt = new Date().toISOString();
const execute = (name, caseOptions, variant, index, warmup) => {
  const options = { ...caseOptions, variant };
  const environment = { ...process.env };
  delete environment.ROLLDOWN_PARALLEL_PLUGIN_WORKERS;
  delete environment.ROLLDOWN_PARALLEL_PLUGIN_METRICS;
  const workerMatch = /^worker-(\d+)$/.exec(variant);
  if (workerMatch) {
    environment.ROLLDOWN_PARALLEL_PLUGIN_WORKERS = workerMatch[1];
    if (options.instrumentation) environment.ROLLDOWN_PARALLEL_PLUGIN_METRICS = 'json';
  }
  const result = spawnSync(
    '/usr/bin/time',
    [
      '-l',
      process.execPath,
      '--expose-gc',
      nodePath.join(import.meta.dirname, 'run-case.mjs'),
      JSON.stringify(options),
    ],
    { encoding: 'utf8', env: environment },
  );
  if (result.status !== 0) {
    throw new Error(
      `Svelte transform child failed with status ${result.status}:\n${result.stderr}`,
    );
  }
  if (warmup) return;

  const peakRssMatch = result.stderr.match(/(\d+)\s+maximum resident set size/);
  if (!peakRssMatch) throw new Error('failed to parse maximum resident set size');
  const rustMetricsMatches = [
    ...result.stderr.matchAll(/^\[rolldown-parallel-plugin-metrics\] (\{.*\})$/gm),
  ];
  const lifecycleMetrics = [
    ...result.stderr.matchAll(/^\[rolldown-parallel-plugin-init-metrics\] (\{.*\})$/gm),
  ].map((match) => JSON.parse(match[1]));
  const expectedRustMetrics = options.instrumentation && workerMatch ? 1 : 0;
  if (rustMetricsMatches.length !== expectedRustMetrics) {
    throw new Error(
      `expected ${expectedRustMetrics} Rust metrics lines for ${name}/${variant}, got ${rustMetricsMatches.length}`,
    );
  }
  if (lifecycleMetrics.length !== expectedRustMetrics * 2) {
    throw new Error(`unexpected lifecycle metric count for ${name}/${variant}`);
  }
  const child = JSON.parse(result.stdout);
  const rustMetrics = rustMetricsMatches[0] ? JSON.parse(rustMetricsMatches[0][1]) : undefined;
  const initializationMetrics = lifecycleMetrics.find(
    ({ kind }) => kind === 'rolldown_parallel_plugin_init_metrics',
  );
  const terminationMetrics = lifecycleMetrics.find(
    ({ kind }) => kind === 'rolldown_parallel_plugin_termination_metrics',
  );
  validateRun(
    child,
    rustMetrics,
    initializationMetrics,
    terminationMetrics,
    workerMatch ? Number(workerMatch[1]) : 0,
  );
  runs.push({
    name,
    index,
    sequence: sequence++,
    peakRssBytes: Number(peakRssMatch[1]),
    ...child,
    rustMetrics,
    initializationMetrics,
    terminationMetrics,
  });
};

for (const definition of matrix.cases) {
  const { name, variants, warmups = 1, repeats = 1, componentCount, ...caseOptions } = definition;
  if (caseSelections.some((selection) => selection.name === name)) {
    throw new Error(`duplicate matrix case name: ${name}`);
  }
  const selectedEntries = selectManifestEntries(manifest, componentCount);
  caseSelections.push({
    name,
    componentCount,
    selectionHash: selectionHash(selectedEntries),
    sourceBytes: selectedEntries.reduce((total, entry) => total + entry.bytes, 0),
    sourceLines: selectedEntries.reduce((total, entry) => total + entry.lines, 0),
    typeScriptFiles: selectedEntries.filter((entry) => entry.typeScript).length,
    runeFiles: selectedEntries.filter((entry) => entry.runes).length,
    uniqueContents: new Set(selectedEntries.map((entry) => entry.sha256)).size,
  });
  const caseDirectory = await mkdtemp(nodePath.join(tmpdir(), 'rolldown-parallel-svelte-case-'));
  const caseRunStart = runs.length;
  try {
    const entryPath = nodePath.join(caseDirectory, 'entry.js');
    await writeFile(
      entryPath,
      `${selectedEntries
        .map(
          (entry, index) =>
            `export { default as component_${String(index).padStart(4, '0')} } from ${JSON.stringify(nodePath.join(corpusDirectory, entry.path))};`,
        )
        .join('\n')}\n`,
    );
    const executionOptions = {
      ...caseOptions,
      componentCount,
      _corpusDirectory: corpusDirectory,
      _entryPath: entryPath,
      _selectedSourceBytes: selectedEntries.reduce((total, entry) => total + entry.bytes, 0),
      _selectionHash: selectionHash(selectedEntries),
    };
    for (let index = 0; index < warmups; index++) {
      for (const variant of variants) execute(name, executionOptions, variant, index, true);
    }
    for (let index = 0; index < repeats; index++) {
      const offset = index % variants.length;
      const order = [...variants.slice(offset), ...variants.slice(0, offset)];
      for (const variant of order) execute(name, executionOptions, variant, index, false);
    }
    for (const hashField of [
      'outputRawCodeHash',
      'outputCodeHash',
      'outputRawMapHash',
      'outputMapHash',
    ]) {
      const hashes = new Set(runs.slice(caseRunStart).map((run) => run[hashField]));
      if (hashes.size !== 1) {
        throw new Error(
          `${name} produced different ${hashField} values: ${[...hashes].join(', ')}`,
        );
      }
    }
  } finally {
    await rm(caseDirectory, { recursive: true, force: true });
  }
}

const report = {
  schema: 1,
  startedAt,
  finishedAt: new Date().toISOString(),
  node: process.version,
  nodeBinary: process.execPath,
  svelteVersion: '5.56.4',
  rolldownCommit: gitCommit.stdout.trim(),
  rolldownWorktreeStatus: gitStatus.stdout.trim(),
  nativeBinding: {
    path: nodePath.relative(repositoryRoot, bindingPath),
    bytes: bindingStat.size,
    sha256: createHash('sha256').update(bindingContent).digest('hex'),
    profileClaim: matrix.bindingProfile,
    profileVerification:
      'The byte hash pins the artifact; the report cannot infer its Cargo profile.',
  },
  corpus: {
    upstream: manifest.upstream,
    selection: manifest.selection,
    summary: manifest.summary,
  },
  host: {
    platform: platform(),
    release: release(),
    architecture: process.arch,
    cpuModel: cpus()[0]?.model,
    logicalCpuCount: cpus().length,
    totalMemoryBytes: totalmem(),
  },
  matrix,
  caseSelections,
  runs,
};
const serializedReport = `${JSON.stringify(report, null, 2)}\n`;
if (outputPath) {
  await mkdir(nodePath.dirname(nodePath.resolve(outputPath)), { recursive: true });
  await writeFile(outputPath, serializedReport);
  console.log(
    JSON.stringify({
      outputPath,
      cases: matrix.cases.length,
      runs: runs.length,
      startedAt,
      finishedAt: report.finishedAt,
    }),
  );
} else {
  process.stdout.write(serializedReport);
}

function validateRun(run, rustMetrics, initializationMetrics, terminationMetrics, workerCount) {
  if (!run.instrumentation) {
    if (run.jsMetrics || rustMetrics || initializationMetrics || terminationMetrics) {
      throw new Error('uninstrumented run emitted metrics');
    }
    return;
  }
  const js = run.jsMetrics;
  if (!js) throw new Error('instrumented run did not emit JavaScript metrics');
  if (js.handlerCalls !== run.expectedMatchingHandlerCalls)
    throw new Error('handler call mismatch');
  if (js.handlerInputCodeBytes !== run.selectedSourceBytes) throw new Error('input byte mismatch');
  if (js.perWorkerCalls.reduce((total, value) => total + value, 0) !== js.handlerCalls) {
    throw new Error('per-worker handler calls do not sum to total calls');
  }
  if (js.factoryCalls !== Math.max(1, workerCount)) throw new Error('factory call mismatch');
  if (js.handlerActive !== 0 || js.errors !== 0 || js.warnings !== 0) {
    throw new Error('unexpected final JavaScript metric state');
  }
  if (js.maxHandlerActive > Math.max(1, workerCount)) {
    throw new Error('handler concurrency exceeds available instances');
  }
  const expectedWorkerMask = ((1n << BigInt(Math.max(1, workerCount))) - 1n).toString(16);
  if (js.workerMask !== expectedWorkerMask) throw new Error('worker factory mask mismatch');
  if (workerCount === 0) {
    if (js.maxHandlerActive !== 1) throw new Error('ordinary handler concurrency is not one');
    return;
  }
  if (!rustMetrics || !initializationMetrics || !terminationMetrics) {
    throw new Error('parallel instrumentation is incomplete');
  }
  if (
    initializationMetrics.workerCount !== workerCount ||
    initializationMetrics.workers.length !== workerCount ||
    terminationMetrics.workerCount !== workerCount ||
    initializationMetrics.pluginCount !== 1
  ) {
    throw new Error('lifecycle worker count mismatch');
  }
  if (rustMetrics.workerCount !== workerCount) throw new Error('Rust worker count mismatch');
  if (
    rustMetrics.wrapperCalls !== rustMetrics.permitAcquiredCalls ||
    rustMetrics.wrapperCalls !== rustMetrics.completedWrapperCalls ||
    rustMetrics.valueResults !== js.handlerCalls ||
    rustMetrics.errorResults !== 0 ||
    rustMetrics.cancelledBeforeAcquire !== 0 ||
    rustMetrics.cancelledDuringService !== 0
  ) {
    throw new Error('unexpected Rust wrapper counts');
  }
  if (rustMetrics.nullResults !== rustMetrics.wrapperCalls - js.handlerCalls) {
    throw new Error('Rust null count does not explain filter misses');
  }
  if (rustMetrics.returnedCodeBytes !== js.handlerReturnedCodeBytes) {
    throw new Error('Rust and JavaScript returned byte counts differ');
  }
  if (
    rustMetrics.permitQueuePending.current !== 0 ||
    rustMetrics.wrapperOutstanding.current !== 0 ||
    rustMetrics.permitInFlight.current !== 0 ||
    rustMetrics.permitInFlight.max > workerCount
  ) {
    throw new Error('Rust permit counters are invalid');
  }
}
