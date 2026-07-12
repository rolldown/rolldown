import { spawnSync } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { cpus, platform, release, totalmem } from 'node:os';
import nodePath from 'node:path';
import {
  admitFormalHost,
  assertNoPagingDelta,
  virtualMemoryCounters,
} from '../vue-scale/host-policy.mjs';
import { assertLocalExecution, BASELINE_POOL_ENVIRONMENT } from '../vue-scale/provenance.mjs';

assertLocalExecution();
const matrixPath = process.argv[2];
const outputPath = process.argv[3];
if (!matrixPath) throw new Error('expected a matrix JSON path');
const matrix = JSON.parse(await readFile(matrixPath, 'utf8'));
validateMatrix(matrix);

const formal = matrix.lane === 'formal-attribution';
const variants = matrix.cases.flatMap((definition) =>
  definition.workerCounts.map((workerCount) => ({ ...definition, workerCount })),
);
const runs = [];
const hostAdmissions = [];
let sequence = 0;
for (let block = 0; block < matrix.repeats; block++) {
  const offset = block % variants.length;
  const order = [...variants.slice(offset), ...variants.slice(0, offset)];
  for (const variant of order) {
    const admission = formal ? await admitFormalHost() : undefined;
    if (admission)
      hostAdmissions.push({
        block,
        name: variant.name,
        workerCount: variant.workerCount,
        ...admission,
      });
    runs.push({ sequence: sequence++, block, ...execute(variant, admission) });
  }
}

const report = {
  schemaVersion: 1,
  kind: 'rolldown-runtime-initialization-matrix',
  measurementClass: formal
    ? 'formal local initialization attribution; instrumented elapsed values are not wall benchmark evidence'
    : 'untimed correctness smoke; not performance evidence',
  createdAt: new Date().toISOString(),
  host: {
    platform: platform(),
    release: release(),
    architecture: process.arch,
    cpuModel: cpus()[0]?.model,
    logicalCpuCount: cpus().length,
    totalMemoryBytes: totalmem(),
  },
  hostAdmissions,
  matrix,
  runs,
};
const serialized = `${JSON.stringify(report, null, 2)}\n`;
if (outputPath) {
  await mkdir(nodePath.dirname(nodePath.resolve(outputPath)), { recursive: true });
  await writeFile(outputPath, serialized);
  console.log(JSON.stringify({ outputPath, runs: runs.length, lane: matrix.lane }));
} else {
  process.stdout.write(serialized);
}

function execute(variant, admission) {
  const environment = {
    ...process.env,
    ...BASELINE_POOL_ENVIRONMENT,
    ROLLDOWN_PARALLEL_PLUGIN_METRICS: 'json',
  };
  const options = {
    mode: variant.mode,
    workerCount: variant.workerCount,
    parentPreload: variant.parentPreload,
    sampleIntervalMs: matrix.sampleIntervalMs,
    sampleOsThreads: matrix.sampleOsThreads,
  };
  const childArguments = [
    process.execPath,
    '--expose-gc',
    nodePath.join(import.meta.dirname, 'run-case.mjs'),
    JSON.stringify(options),
  ];
  const beforeVm = formal ? virtualMemoryCounters() : undefined;
  const result = formal
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
  const afterVm = formal ? virtualMemoryCounters() : undefined;
  if (result.error) {
    throw new Error(
      `${variant.name}/worker-${variant.workerCount} failed to execute: ${result.error.code ?? result.error.message}`,
    );
  }
  if (result.status !== 0) {
    throw new Error(
      `${variant.name}/worker-${variant.workerCount} failed (${result.status}):\n${result.stderr}`,
    );
  }
  const child = JSON.parse(result.stdout);
  if (
    child.runtime.bindingSha256 !== matrix.runtime.bindingSha256 ||
    child.runtime.packageEntrySha256 !== matrix.runtime.packageEntrySha256
  ) {
    throw new Error(`${variant.name}/worker-${variant.workerCount} runtime provenance mismatch`);
  }
  const moduleInit = [
    ...result.stderr.matchAll(/^\[rolldown-parallel-plugin-module-init-metrics\] (\{.*\})$/gm),
  ].map((match) => JSON.parse(match[1]));
  const importsNativeLibrary = variant.parentPreload !== 'none' || variant.mode !== 'empty';
  const expectedModuleInit = importsNativeLibrary ? 1 : 0;
  if (moduleInit.length !== expectedModuleInit) {
    throw new Error(
      `${variant.name}/worker-${variant.workerCount} expected ${expectedModuleInit} module init records, got ${moduleInit.length}`,
    );
  }
  const ordinals = moduleInit
    .map((record) => record.invocationOrdinal)
    .sort((left, right) => left - right);
  if (ordinals.some((ordinal, index) => ordinal !== index + 1)) {
    throw new Error(
      `${variant.name}/worker-${variant.workerCount} module init ordinals are incomplete`,
    );
  }
  if (
    moduleInit.some(
      (record) =>
        record.kind !== 'rolldown_binding_module_init_metrics' ||
        record.configuredTokioWorkerThreads !== 18 ||
        record.configuredTokioMaxBlockingThreads !== 4,
    )
  ) {
    throw new Error(`${variant.name}/worker-${variant.workerCount} module init schema mismatch`);
  }
  const peakRssMatch = formal
    ? result.stderr.match(/(\d+)\s+maximum resident set size/)
    : undefined;
  if (formal && !peakRssMatch) throw new Error('failed to parse child peak RSS');
  return {
    name: variant.name,
    mode: variant.mode,
    parentPreload: variant.parentPreload,
    workerCount: variant.workerCount,
    hostAdmission: admission,
    pagingDelta: formal ? assertNoPagingDelta(beforeVm, afterVm) : undefined,
    peakRssBytes: peakRssMatch ? Number(peakRssMatch[1]) : undefined,
    child,
    moduleInit,
  };
}

function validateMatrix(value) {
  if (
    value.schema !== 1 ||
    !['correctness-smoke', 'formal-attribution'].includes(value.lane) ||
    value.bindingProfile !== 'release' ||
    JSON.stringify(value.configuredPools) !==
      JSON.stringify({ tokio: 18, rayon: 12, blocking: 4 }) ||
    !Number.isSafeInteger(value.sampleIntervalMs) ||
    value.sampleIntervalMs < 1 ||
    value.sampleIntervalMs > 100 ||
    typeof value.sampleOsThreads !== 'boolean' ||
    !Number.isSafeInteger(value.repeats) ||
    value.repeats < 1 ||
    !/^[0-9a-f]{64}$/.test(value.runtime?.bindingSha256) ||
    !/^[0-9a-f]{64}$/.test(value.runtime?.packageEntrySha256) ||
    !Array.isArray(value.cases) ||
    value.cases.length === 0
  ) {
    throw new Error('invalid runtime initialization matrix header or unresolved runtime hash');
  }
  for (const definition of value.cases) {
    if (
      typeof definition.name !== 'string' ||
      !['none', 'binding', 'package'].includes(definition.parentPreload) ||
      !['empty', 'binding', 'package'].includes(definition.mode) ||
      !Array.isArray(definition.workerCounts) ||
      definition.workerCounts.length === 0 ||
      new Set(definition.workerCounts).size !== definition.workerCounts.length ||
      definition.workerCounts.some(
        (count) => !Number.isSafeInteger(count) || count < 1 || count > 8,
      )
    ) {
      throw new Error(`invalid runtime initialization case: ${JSON.stringify(definition)}`);
    }
  }
}
