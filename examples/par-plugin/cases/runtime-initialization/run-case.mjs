import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { readFile, readdir, stat } from 'node:fs/promises';
import nodePath from 'node:path';
import { pathToFileURL } from 'node:url';
import { performance } from 'node:perf_hooks';
import { Worker, isMainThread } from 'node:worker_threads';
import { getHeapStatistics } from 'node:v8';
import { createRequire } from 'node:module';

if (!isMainThread) throw new Error('runtime initialization case must run on the main thread');
if (process.version !== 'v24.18.0') {
  throw new Error(
    `runtime initialization attribution requires Node.js v24.18.0, got ${process.version}`,
  );
}
const options = JSON.parse(process.argv[2] ?? '{}');
validateOptions(options);

const repositoryRoot = nodePath.resolve(import.meta.dirname, '../../../..');
const packageRoot = nodePath.resolve(
  process.env.ROLLDOWN_RESEARCH_PACKAGE_ROOT ?? nodePath.join(repositoryRoot, 'packages/rolldown'),
);
const distributionRoot = nodePath.join(packageRoot, 'dist');
const bindingNames = (await readdir(distributionRoot)).filter((name) =>
  /^rolldown-binding\..+\.node$/.test(name),
);
if (bindingNames.length !== 1)
  throw new Error(`expected one native binding, got ${bindingNames.length}`);
const bindingPath = nodePath.join(distributionRoot, bindingNames[0]);
const packageEntryPath = nodePath.join(distributionRoot, 'index.mjs');
const workerPath = nodePath.join(import.meta.dirname, 'worker.mjs');
const [bindingStat, bindingSha256, packageEntry, workerSource] = await Promise.all([
  stat(bindingPath),
  hashFile(bindingPath),
  readFile(packageEntryPath),
  readFile(workerPath),
]);

const processStartedAt = timestamp();
const processCpuStartedAt = process.cpuUsage();
const mainCpuStartedAt = process.threadCpuUsage();
const processBeforePreload = captureProcessSnapshot();
let preloadTimeline;
if (options.parentPreload !== 'none') {
  const startedAt = timestamp();
  if (options.parentPreload === 'binding') {
    createRequire(import.meta.url)(bindingPath);
  } else {
    await import(pathToFileURL(packageEntryPath).href);
  }
  const finishedAt = timestamp();
  preloadTimeline = { mode: options.parentPreload, startedAt, finishedAt };
}
const processBeforeWorkers = captureProcessSnapshot();
const stateBuffer = new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT * options.workerCount);
const state = new Int32Array(stateBuffer);
const workers = [];
const readyPromises = [];
const online = new Map();
const ready = new Map();
const constructorTimeline = [];
const workerUrl = pathToFileURL(workerPath);

for (let workerIndex = 0; workerIndex < options.workerCount; workerIndex++) {
  const constructorStartedAt = timestamp();
  const worker = new Worker(workerUrl, {
    workerData: {
      workerIndex,
      mode: options.mode,
      stateBuffer,
      bindingPath,
      packageEntryUrl: pathToFileURL(packageEntryPath).href,
    },
  });
  const constructorReturnedAt = timestamp();
  constructorTimeline.push({ workerIndex, constructorStartedAt, constructorReturnedAt });
  workers.push(worker);
  readyPromises.push(
    new Promise((resolve, reject) => {
      worker.once('online', () => online.set(workerIndex, timestamp()));
      worker.on('message', (message) => {
        if (message.type === 'ready') {
          ready.set(workerIndex, { receivedAt: timestamp(), ...message });
          resolve();
        }
      });
      worker.once('error', reject);
      worker.once('exit', (code) => {
        if (!ready.has(workerIndex))
          reject(new Error(`worker ${workerIndex} exited ${code} before ready`));
      });
    }),
  );
}

const samples = [];
let samplerFinished = false;
const sampler = (async () => {
  while (!samplerFinished) {
    samples.push(captureProcessSample(state));
    await new Promise((resolve) => setTimeout(resolve, options.sampleIntervalMs));
  }
  samples.push(captureProcessSample(state));
})();

await Promise.all(readyPromises);
samplerFinished = true;
await sampler;
const workerResourcesAtReady = await Promise.all(
  workers.map(async (worker, workerIndex) => ({
    workerIndex,
    cpuUsageMicros: await worker.cpuUsage(),
    heapStatistics: await worker.getHeapStatistics(),
    eventLoopUtilization: worker.performance.eventLoopUtilization(),
  })),
);
const processAtReady = captureProcessSnapshot();

const snapshotPromises = workers.map(
  (worker, workerIndex) =>
    new Promise((resolve, reject) => {
      const listener = (message) => {
        if (message.type === 'snapshot' && message.workerIndex === workerIndex) {
          worker.off('message', listener);
          resolve(message);
        }
      };
      worker.on('message', listener);
      worker.once('error', reject);
      worker.postMessage('snapshot');
    }),
);
const workerLocalSnapshots = await Promise.all(snapshotPromises);
const terminationStartedAt = timestamp();
await Promise.all(workers.map((worker) => worker.terminate()));
const terminationFinishedAt = timestamp();
const processAfterTermination = captureProcessSnapshot();

const processCpu = process.cpuUsage(processCpuStartedAt);
const mainCpu = subtractThreadCpu(process.threadCpuUsage(), mainCpuStartedAt);
const workerCpu = workerResourcesAtReady.reduce(
  (total, resource) => ({
    user: total.user + resource.cpuUsageMicros.user,
    system: total.system + resource.cpuUsageMicros.system,
  }),
  { user: 0, system: 0 },
);
const processCpuTotal = processCpu.user + processCpu.system;
const measuredJsCpuTotal = mainCpu.user + mainCpu.system + workerCpu.user + workerCpu.system;
const result = {
  schemaVersion: 1,
  kind: 'rolldown-runtime-initialization-case',
  measurementClass:
    'instrumented initialization attribution; elapsed values are not wall benchmark evidence',
  options,
  runtime: {
    node: process.version,
    packageRoot,
    bindingPath,
    bindingBytes: bindingStat.size,
    bindingSha256,
    packageEntrySha256: sha256(packageEntry),
    workerSourceSha256: sha256(workerSource),
  },
  timeline: {
    processStartedAt,
    preload: preloadTimeline,
    constructors: constructorTimeline,
    online: Object.fromEntries(online),
    ready: [...ready.values()].sort((left, right) => left.workerIndex - right.workerIndex),
    terminationStartedAt,
    terminationFinishedAt,
  },
  resources: {
    scope: {
      rss: 'whole process; controlled differences are not direct worker ownership',
      processCpu: 'whole process from before optional preload through termination',
      mainCpu: 'Node.js main thread only',
      workerCpu: 'sum of Worker.cpuUsage snapshots at pool ready',
      residualCpu:
        'whole-process CPU minus measured Node.js main and worker thread CPU; includes Rust, Node runtime, native addon, helper threads, and measurement error',
    },
    processBeforePreload,
    processBeforeWorkers,
    processAtReady,
    processAfterTermination,
    workerResourcesAtReady,
    workerLocalSnapshots,
    processCpuMicros: processCpu,
    mainCpuMicros: mainCpu,
    workerCpuMicros: workerCpu,
    residualCpuMicros: processCpuTotal - measuredJsCpuTotal,
    samples,
    peakSampledRssBytes: Math.max(...samples.map((sample) => sample.memoryUsageBytes.rss)),
    peakSampledThreadCount: options.sampleOsThreads
      ? Math.max(...samples.map((sample) => sample.osThreadCount ?? 0))
      : undefined,
  },
};
process.stdout.write(`${JSON.stringify(result)}\n`);

function timestamp() {
  const monotonicMs = performance.now();
  return { monotonicMs, epochMs: performance.timeOrigin + monotonicMs };
}

function captureProcessSnapshot() {
  return {
    capturedAt: timestamp(),
    cpuUsageMicros: process.cpuUsage(),
    mainThreadCpuUsageMicros: process.threadCpuUsage(),
    memoryUsageBytes: process.memoryUsage(),
    mainIsolateHeapStatistics: getHeapStatistics(),
  };
}

function captureProcessSample(state) {
  return {
    capturedAt: timestamp(),
    state: [...state],
    memoryUsageBytes: process.memoryUsage(),
    osThreadCount: options.sampleOsThreads ? readOsThreadCount() : undefined,
  };
}

function readOsThreadCount() {
  if (process.platform !== 'darwin') return undefined;
  const result = spawnSync('/bin/ps', ['-M', '-p', String(process.pid)], {
    encoding: 'utf8',
  });
  if (result.status !== 0) return undefined;
  const lines = result.stdout.trim().split('\n');
  return Math.max(0, lines.length - 1);
}

function subtractThreadCpu(after, before) {
  return { user: after.user - before.user, system: after.system - before.system };
}

function sha256(value) {
  return createHash('sha256').update(value).digest('hex');
}

function hashFile(path) {
  const result = spawnSync('/usr/bin/shasum', ['-a', '256', path], { encoding: 'utf8' });
  if (result.status !== 0) throw new Error(`shasum failed for ${path}: ${result.stderr}`);
  const digest = result.stdout.trim().split(/\s+/)[0];
  if (!/^[0-9a-f]{64}$/.test(digest)) throw new Error(`invalid shasum output for ${path}`);
  return digest;
}

function validateOptions(value) {
  if (!['empty', 'binding', 'package'].includes(value.mode))
    throw new Error('mode must be empty, binding, or package');
  if (!Number.isSafeInteger(value.workerCount) || value.workerCount < 1 || value.workerCount > 8) {
    throw new Error('workerCount must be an integer from 1 through 8');
  }
  if (!['none', 'binding', 'package'].includes(value.parentPreload)) {
    throw new Error('parentPreload must be none, binding, or package');
  }
  if (
    !Number.isSafeInteger(value.sampleIntervalMs) ||
    value.sampleIntervalMs < 1 ||
    value.sampleIntervalMs > 100
  ) {
    throw new Error('sampleIntervalMs must be an integer from 1 through 100');
  }
  if (typeof value.sampleOsThreads !== 'boolean')
    throw new Error('sampleOsThreads must be boolean');
}
