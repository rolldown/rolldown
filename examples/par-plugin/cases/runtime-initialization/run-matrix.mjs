import { spawn } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import { cpus, platform, release, totalmem } from 'node:os';
import nodePath from 'node:path';
import {
  admitFormalHost,
  admitFormalHostAfterChild,
  assertNoPagingDelta,
  virtualMemoryCounters,
} from '../vue-scale/host-policy.mjs';
import { assertLocalExecution, BASELINE_POOL_ENVIRONMENT } from '../vue-scale/provenance.mjs';
import {
  captureInitializationHarnessProvenance,
  inspectAttributionRuntime,
  verifyCurrentHarnessProvenance,
} from './provenance.mjs';
import {
  flattenInitializationVariants,
  initializationWorkerSourceSha256,
  INITIALIZATION_TIMEOUTS,
  orderInitializationVariants,
  parseMacOsPeakRss,
  validateInitializationCase,
  validateInitializationMatrix,
  validateModuleInitRecords,
} from './admission.mjs';
import { writeArtifactAtomically } from './artifact-io.mjs';

assertLocalExecution();
for (const name of [
  'NODE_COMPILE_CACHE',
  'NODE_COMPILE_CACHE_PORTABLE',
  'NODE_DISABLE_COMPILE_CACHE',
]) {
  if (typeof process.env[name] === 'string' && process.env[name].trim() !== '') {
    throw new Error(`runtime initialization matrix rejects inherited ${name}`);
  }
}
const validateOnly = process.argv.includes('--validate-only');
const positional = process.argv.slice(2).filter((value) => value !== '--validate-only');
const [matrixPath, outputPath] = positional;
if (!matrixPath) throw new Error('expected a matrix JSON path');
const matrix = validateInitializationMatrix(JSON.parse(await readFile(matrixPath, 'utf8')));
if (validateOnly) {
  console.log(JSON.stringify({ valid: true, matrixPath, lane: matrix.lane }));
  process.exit(0);
}

const formal = matrix.lane === 'formal-attribution';
const repositoryRoot = nodePath.resolve(import.meta.dirname, '../../../..');
if (
  formal &&
  (!outputPath ||
    !nodePath
      .resolve(outputPath)
      .startsWith(nodePath.join(import.meta.dirname, '.results') + nodePath.sep))
) {
  throw new Error('formal initialization raw output must be written below .results');
}
const packageRoot = nodePath.resolve(
  process.env.ROLLDOWN_RESEARCH_PACKAGE_ROOT ?? nodePath.join(repositoryRoot, 'packages/rolldown'),
);
const harnessProvenance = await captureInitializationHarnessProvenance({ requireClean: formal });
const workerSourceSha256 = initializationWorkerSourceSha256(harnessProvenance);
const runtimeProvenance = await inspectAttributionRuntime(packageRoot, matrix.runtime);
const variants = flattenInitializationVariants(matrix);
const runs = [];
const hostAdmissions = [];
let sequence = 0;
for (let block = 0; block < matrix.repeats; block++) {
  const order = orderInitializationVariants(variants, block);
  for (const variant of order) {
    const admission = formal ? await admitFormalHost() : undefined;
    if (admission)
      hostAdmissions.push({
        block,
        name: variant.name,
        workerCount: variant.workerCount,
        ...admission,
      });
    runs.push({ sequence: sequence++, block, ...(await execute(variant, admission)) });
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
  executionEnvironment: {
    inheritedNodeOptions: null,
    inheritedNodeCompileCache: null,
    inheritedNodeCompileCachePortable: null,
    inheritedNodeDisableCompileCache: null,
    childNodeEnv: 'production',
    childPoolEnvironment: BASELINE_POOL_ENVIRONMENT,
    exposeGcByArgument: true,
    timeouts: INITIALIZATION_TIMEOUTS,
    rotation: 'paired-block offset with odd blocks reversed',
  },
  hostAdmissions,
  harnessProvenance,
  runtimeProvenance,
  matrix,
  runs,
};
verifyCurrentHarnessProvenance(
  harnessProvenance,
  await captureInitializationHarnessProvenance({ requireClean: formal }),
);
const finalRuntimeProvenance = await inspectAttributionRuntime(packageRoot, matrix.runtime);
if (JSON.stringify(finalRuntimeProvenance) !== JSON.stringify(runtimeProvenance)) {
  throw new Error('initialization attribution runtime changed during the matrix');
}
const serialized = `${JSON.stringify(report, null, 2)}\n`;
if (outputPath) {
  await writeArtifactAtomically(outputPath, serialized);
  console.log(JSON.stringify({ outputPath, runs: runs.length, lane: matrix.lane }));
} else {
  process.stdout.write(serialized);
}

async function execute(variant, admission) {
  const environment = {
    ...process.env,
    ...BASELINE_POOL_ENVIRONMENT,
    NODE_ENV: 'production',
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
    ? await spawnCaptured('/usr/bin/time', ['-l', ...childArguments], environment)
    : await spawnCaptured(childArguments[0], childArguments.slice(1), environment);
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
  validateInitializationCase(child, options, matrix.runtime, workerSourceSha256);
  const moduleInit = [
    ...result.stderr.matchAll(/^\[rolldown-parallel-plugin-module-init-metrics\] (\{.*\})$/gm),
  ].map((match) => JSON.parse(match[1]));
  validateModuleInitRecords(moduleInit, options);
  const peakRssBytes = parseMacOsPeakRss(result.stderr, { required: formal });
  const postHostAdmission = formal ? admitFormalHostAfterChild() : undefined;
  return {
    name: variant.name,
    mode: variant.mode,
    parentPreload: variant.parentPreload,
    workerCount: variant.workerCount,
    hostAdmission: admission,
    postHostAdmission,
    pagingDelta: formal ? assertNoPagingDelta(beforeVm, afterVm) : undefined,
    peakRssBytes,
    child,
    moduleInit,
  };
}

function spawnCaptured(command, arguments_, environment) {
  const maximumBytes = 64 * 1024 * 1024;
  return new Promise((resolve) => {
    const child = spawn(command, arguments_, {
      detached: true,
      env: environment,
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    const stdout = [];
    const stderr = [];
    let stdoutBytes = 0;
    let stderrBytes = 0;
    let error;
    let settled = false;
    const killGroup = () => {
      try {
        process.kill(-child.pid, 'SIGKILL');
      } catch {
        child.kill('SIGKILL');
      }
    };
    const timer = setTimeout(() => {
      error = Object.assign(
        new Error(
          `initialization child timed out after ${INITIALIZATION_TIMEOUTS.childProcessMs} ms`,
        ),
        { code: 'ETIMEDOUT' },
      );
      killGroup();
    }, INITIALIZATION_TIMEOUTS.childProcessMs);
    const collect = (chunks, chunk, stream) => {
      const bytes = Buffer.byteLength(chunk);
      if (stream === 'stdout') stdoutBytes += bytes;
      else stderrBytes += bytes;
      if (stdoutBytes > maximumBytes || stderrBytes > maximumBytes) {
        error = Object.assign(
          new Error(`initialization child exceeded ${maximumBytes} output bytes`),
          {
            code: 'ENOBUFS',
          },
        );
        killGroup();
        return;
      }
      chunks.push(chunk);
    };
    child.stdout.setEncoding('utf8');
    child.stderr.setEncoding('utf8');
    child.stdout.on('data', (chunk) => collect(stdout, chunk, 'stdout'));
    child.stderr.on('data', (chunk) => collect(stderr, chunk, 'stderr'));
    child.once('error', (spawnError) => {
      error = spawnError;
    });
    child.once('close', (status, signal) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      resolve({
        status,
        signal,
        stdout: stdout.join(''),
        stderr: stderr.join(''),
        error,
      });
    });
  });
}
