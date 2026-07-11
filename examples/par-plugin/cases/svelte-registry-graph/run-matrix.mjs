import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { mkdir, readFile, readdir, stat, writeFile } from 'node:fs/promises';
import { cpus, platform, release, totalmem } from 'node:os';
import nodePath from 'node:path';
import { readSourceManifest, verifyGraphCorpus } from './graph-corpus.mjs';
import { hashRolldownDistribution } from './provenance.mjs';

if (process.version !== 'v24.18.0') {
  throw new Error(`registry graph matrix requires Node.js v24.18.0, got ${process.version}`);
}
const matrixPath = process.argv[2];
if (!matrixPath) throw new Error('expected a matrix JSON path');
const outputPath = process.argv[3];
const matrix = JSON.parse(await readFile(matrixPath, 'utf8'));
if (!Array.isArray(matrix.cases) || matrix.bindingProfile !== 'release') {
  throw new Error('matrix requires cases and bindingProfile "release"');
}
const manifest = await readSourceManifest(
  nodePath.join(import.meta.dirname, 'source-manifest.json'),
);
const expected = JSON.parse(
  await readFile(nodePath.join(import.meta.dirname, 'expected-graph.json'), 'utf8'),
);
const corpusDirectory = nodePath.join(import.meta.dirname, '.graph-corpus');
await verifyGraphCorpus({ corpusDirectory, manifest });
const entryPaths = manifest.entryPaths.map((path) => nodePath.join(corpusDirectory, path));

const repositoryRoot = nodePath.resolve(import.meta.dirname, '../../../..');
const nativeBindingSourceCommit = '54fd0e24112505443044a4bba5c41d1f4d9ba2aa';
const git = (arguments_) => {
  const result = spawnSync('git', ['-C', repositoryRoot, ...arguments_], { encoding: 'utf8' });
  if (result.status !== 0) throw new Error(`git ${arguments_.join(' ')} failed`);
  return result.stdout.trim();
};
const fixtureCommit = git(['rev-parse', 'HEAD']);
const initialWorktreeStatus = git(['status', '--short']);
if (initialWorktreeStatus)
  throw new Error('formal registry graph matrices require a clean worktree');
const nonFixtureChangesSinceBinding = git([
  'diff',
  '--name-only',
  `${nativeBindingSourceCommit}..${fixtureCommit}`,
])
  .split('\n')
  .filter(Boolean)
  .filter((path) => !path.startsWith('examples/par-plugin/') && path !== 'pnpm-lock.yaml');
if (nonFixtureChangesSinceBinding.length !== 0) {
  throw new Error(
    `binding source attribution is invalid: ${nonFixtureChangesSinceBinding.join(', ')}`,
  );
}
const bindingDirectory = nodePath.join(repositoryRoot, 'packages/rolldown/src');
const bindingNames = (await readdir(bindingDirectory)).filter((name) =>
  /^rolldown-binding\..+\.node$/.test(name),
);
if (bindingNames.length !== 1) throw new Error('expected one local native binding');
const bindingPath = nodePath.join(bindingDirectory, bindingNames[0]);
const bindingContent = await readFile(bindingPath);
const bindingStat = await stat(bindingPath);
const rolldownDistribution = await hashRolldownDistribution(repositoryRoot);
const nodeBinaryContent = await readFile(process.execPath);
const nodeBinaryStat = await stat(process.execPath);
const runs = [];
let sequence = 0;
const startedAt = new Date().toISOString();

const execute = (name, caseOptions, variant, index, warmup) => {
  const options = {
    ...caseOptions,
    variant,
    _corpusDirectory: corpusDirectory,
    _entryPaths: entryPaths,
  };
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
    throw new Error(`registry graph child failed for ${name}/${variant}:\n${result.stderr}`);
  }
  if (warmup) return;
  const peakRssMatch = result.stderr.match(/(\d+)\s+maximum resident set size/);
  if (!peakRssMatch) throw new Error('failed to parse child peak RSS');
  const rustMatches = [
    ...result.stderr.matchAll(/^\[rolldown-parallel-plugin-metrics\] (\{.*\})$/gm),
  ];
  const lifecycle = [
    ...result.stderr.matchAll(
      /^\[rolldown-parallel-plugin-(?:init|termination)-metrics\] (\{.*\})$/gm,
    ),
  ].map((match) => JSON.parse(match[1]));
  const expectedRustMetrics = options.instrumentation && workerMatch ? 1 : 0;
  if (rustMatches.length !== expectedRustMetrics || lifecycle.length !== expectedRustMetrics * 2) {
    throw new Error(`unexpected instrumentation lines for ${name}/${variant}`);
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
  const { name, variants, warmups = 1, repeats = 1, ...caseOptions } = definition;
  const caseStart = runs.length;
  for (let index = 0; index < warmups; index++) {
    for (const variant of variants) execute(name, caseOptions, variant, index, true);
  }
  for (let index = 0; index < repeats; index++) {
    const offset = index % variants.length;
    const order = [...variants.slice(offset), ...variants.slice(0, offset)];
    for (const variant of order) execute(name, caseOptions, variant, index, false);
  }
  for (const field of ['graphHash', 'outputCodeHash', 'outputMapHash']) {
    const values = new Set(runs.slice(caseStart).map((run) => run[field]));
    if (values.size !== 1) throw new Error(`${name} produced different ${field} values`);
  }
}

const report = {
  schema: 1,
  startedAt,
  finishedAt: new Date().toISOString(),
  node: process.version,
  nodeBinary: process.execPath,
  nodeArtifact: {
    bytes: nodeBinaryStat.size,
    sha256: createHash('sha256').update(nodeBinaryContent).digest('hex'),
  },
  svelteVersion: '5.56.4',
  rolldownCommit: fixtureCommit,
  rolldownWorktreeStatus: initialWorktreeStatus,
  nativeBinding: {
    path: nodePath.relative(repositoryRoot, bindingPath),
    bytes: bindingStat.size,
    sha256: createHash('sha256').update(bindingContent).digest('hex'),
    sourceCommit: nativeBindingSourceCommit,
    profileClaim: matrix.bindingProfile,
    profileVerification:
      'The byte hash pins the artifact; the report cannot infer its Cargo profile.',
  },
  rolldownDistribution,
  host: {
    platform: platform(),
    release: release(),
    architecture: process.arch,
    cpuModel: cpus()[0]?.model,
    logicalCpuCount: cpus().length,
    totalMemoryBytes: totalmem(),
  },
  sourceManifest: manifest,
  expectedGraph: expected,
  matrix,
  runs,
};
const finalWorktreeStatus = git(['status', '--short']);
if (finalWorktreeStatus) throw new Error('worktree changed during registry graph matrix');
if (
  JSON.stringify(await hashRolldownDistribution(repositoryRoot)) !==
  JSON.stringify(rolldownDistribution)
) {
  throw new Error('Rolldown distribution changed during registry graph matrix');
}
const serialized = `${JSON.stringify(report, null, 2)}\n`;
if (outputPath) {
  await mkdir(nodePath.dirname(nodePath.resolve(outputPath)), { recursive: true });
  await writeFile(outputPath, serialized);
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
  process.stdout.write(serialized);
}

function validateRun(run, rust, initialization, termination, workerCount) {
  for (const field of [
    'localModuleCount',
    'componentModuleCount',
    'svelteModuleCount',
    'typeScriptModuleCount',
    'graphSourceBytes',
    'graphHash',
    'outputChunkCount',
    'outputAssetCount',
    'nullMapChunkCount',
    'outputCodeBytes',
    'outputMapBytes',
    'totalExports',
    'outputCodeHash',
    'outputMapHash',
  ]) {
    if (run[field] !== expected[field]) throw new Error(`${field} differs from ordinary proof`);
  }
  for (const field of [
    'localModulePaths',
    'resolverTelemetry',
    'bareExternalIds',
    'appVirtualExternals',
    'workspacePackageExternals',
    'svelteRuntimeExternals',
    'thirdPartyBareExternals',
    'bareExternalPackages',
    'logs',
  ]) {
    if (JSON.stringify(run[field]) !== JSON.stringify(expected[field])) {
      throw new Error(`${field} differs from ordinary proof`);
    }
  }
  if (
    run.entryCount !== expected.entryPaths.length ||
    run.externalizedImports.length !== expected.externalizedImportCount ||
    run.projectLocalExternalCount !== 0
  ) {
    throw new Error('entry or externalization gate failed');
  }
  if (!run.instrumentation) {
    if (run.jsMetrics || rust || initialization || termination) {
      throw new Error('uninstrumented graph run emitted metrics');
    }
    return;
  }
  const js = run.jsMetrics;
  if (!js) throw new Error('instrumented graph run omitted JavaScript metrics');
  if (
    js.componentCalls !== expected.componentCalls ||
    js.moduleCalls !== expected.moduleCalls ||
    js.handlerInputCodeBytes !== expected.expectedTransformInputBytes ||
    js.handlerActive !== 0 ||
    js.warnings !== expected.logs.length ||
    js.errors !== 0 ||
    js.factoryCalls !== Math.max(1, workerCount) ||
    js.maxHandlerActive > Math.max(1, workerCount) ||
    js.perWorkerCalls.reduce((total, value) => total + value, 0) !== js.handlerCalls
  ) {
    throw new Error('JavaScript graph metrics failed validation');
  }
  const expectedMask = ((1n << BigInt(Math.max(1, workerCount))) - 1n).toString(16);
  if (js.workerMask !== expectedMask) throw new Error('worker mask differs');
  if (workerCount === 0) {
    if (js.maxHandlerActive !== 1) throw new Error('ordinary handler concurrency differs');
    return;
  }
  if (!rust || !initialization || !termination) throw new Error('worker metrics are incomplete');
  if (
    initialization.workerCount !== workerCount ||
    initialization.workers.length !== workerCount ||
    initialization.pluginCount !== 1 ||
    termination.workerCount !== workerCount ||
    rust.workerCount !== workerCount
  ) {
    throw new Error('worker lifecycle count differs');
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
    rust.permitQueuePending.current !== 0 ||
    rust.wrapperOutstanding.current !== 0 ||
    rust.permitInFlight.current !== 0 ||
    rust.permitInFlight.max > workerCount
  ) {
    throw new Error('Rust graph metrics failed validation');
  }
}
