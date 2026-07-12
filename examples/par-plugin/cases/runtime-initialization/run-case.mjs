import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { readFile, readdir, stat } from 'node:fs/promises';
import nodePath from 'node:path';
import { pathToFileURL } from 'node:url';
import { performance } from 'node:perf_hooks';
import { Worker, isMainThread } from 'node:worker_threads';
import { getHeapStatistics } from 'node:v8';
import { createRequire } from 'node:module';
import { INITIALIZATION_TIMEOUTS } from './admission.mjs';

if (!isMainThread) throw new Error('runtime initialization case must run on the main thread');
if (process.version !== 'v24.18.0') {
  throw new Error(
    `runtime initialization attribution requires Node.js v24.18.0, got ${process.version}`,
  );
}
for (const name of [
  'NODE_OPTIONS',
  'NODE_COMPILE_CACHE',
  'NODE_COMPILE_CACHE_PORTABLE',
  'NODE_DISABLE_COMPILE_CACHE',
]) {
  if (typeof process.env[name] === 'string' && process.env[name].trim() !== '') {
    throw new Error(`runtime initialization attribution rejects inherited ${name}`);
  }
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
const samples = [];
let samplerFinished = false;
let sampler;
let workersTerminated = false;
let processAtAllReady;
let workerResourcesAfterAllReady;
let processAfterWorkerResourceCapture;
let workerLocalSnapshots;
let processBeforeTermination;
let terminationStartedAt;
let terminationFinishedAt;
let processAfterTerminationBeforePostGc;
let processAfterPostGc;

try {
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
    readyPromises.push(waitForWorkerReady(worker, workerIndex, online, ready));
  }

  sampler = (async () => {
    while (!samplerFinished) {
      samples.push(captureProcessSample(state));
      await new Promise((resolve) => setTimeout(resolve, options.sampleIntervalMs));
    }
    samples.push(captureProcessSample(state));
  })();

  await Promise.all(readyPromises);
  processAtAllReady = captureProcessSnapshot();
  samplerFinished = true;
  await sampler;
  workerResourcesAfterAllReady = await withTimeout(
    Promise.all(
      workers.map(async (worker, workerIndex) => ({
        workerIndex,
        cpuUsageMicros: await worker.cpuUsage(),
        heapStatistics: await worker.getHeapStatistics(),
        eventLoopUtilization: worker.performance.eventLoopUtilization(),
      })),
    ),
    INITIALIZATION_TIMEOUTS.workerSnapshotMs,
    'worker all-ready resource capture',
  );
  processAfterWorkerResourceCapture = captureProcessSnapshot();

  workerLocalSnapshots = await Promise.all(
    workers.map((worker, workerIndex) => waitForWorkerSnapshot(worker, workerIndex)),
  );
  processBeforeTermination = captureProcessSnapshot();
  terminationStartedAt = timestamp();
  await withTimeout(
    Promise.all(workers.map((worker) => worker.terminate())),
    INITIALIZATION_TIMEOUTS.workerTerminationMs,
    'worker termination',
  );
  workersTerminated = true;
  terminationFinishedAt = timestamp();
  processAfterTerminationBeforePostGc = captureProcessSnapshot();
  globalThis.gc?.();
  globalThis.gc?.();
  processAfterPostGc = captureProcessSnapshot();
} finally {
  samplerFinished = true;
  if (sampler) await sampler;
  if (!workersTerminated && workers.length > 0) {
    try {
      await withTimeout(
        Promise.allSettled(workers.map((worker) => worker.terminate())),
        INITIALIZATION_TIMEOUTS.workerTerminationMs,
        'failure cleanup worker termination',
      );
    } catch {
      for (const worker of workers) worker.unref();
    }
  }
}

const processCpu = subtractCpu(
  processAfterPostGc.cpuUsageMicros,
  processBeforePreload.cpuUsageMicros,
);
const mainCpu = subtractCpu(
  processAfterPostGc.mainThreadCpuUsageMicros,
  processBeforePreload.mainThreadCpuUsageMicros,
);
const workerCpuAfterAllReady = workerResourcesAfterAllReady.reduce(
  (total, resource) => ({
    user: total.user + resource.cpuUsageMicros.user,
    system: total.system + resource.cpuUsageMicros.system,
  }),
  { user: 0, system: 0 },
);
const workerCpuBeforeTermination = workerLocalSnapshots.reduce(
  (total, resource) => ({
    user: total.user + resource.cpuUsageMicros.user,
    system: total.system + resource.cpuUsageMicros.system,
  }),
  { user: 0, system: 0 },
);
const processCpuTotal = processCpu.user + processCpu.system;
const measuredJsCpuTotal =
  mainCpu.user +
  mainCpu.system +
  workerCpuBeforeTermination.user +
  workerCpuBeforeTermination.system;
const result = {
  schemaVersion: 2,
  kind: 'rolldown-runtime-initialization-case',
  measurementClass:
    'instrumented initialization attribution; elapsed values are not wall benchmark evidence',
  options,
  runtime: {
    node: process.version,
    nodeBinary: process.execPath,
    nodeEnv: process.env.NODE_ENV ?? null,
    packageRoot,
    bindingPath,
    bindingBytes: bindingStat.size,
    bindingSha256,
    packageEntryBytes: packageEntry.byteLength,
    packageEntrySha256: sha256(packageEntry),
    workerSourceSha256: sha256(workerSource),
    configuredPools: {
      tokio: Number(process.env.ROLLDOWN_WORKER_THREADS),
      rayon: Number(process.env.RAYON_NUM_THREADS),
      blocking: Number(process.env.ROLLDOWN_MAX_BLOCKING_THREADS),
    },
    moduleCompileCache: {
      enabled: false,
      reason:
        'NODE_COMPILE_CACHE and NODE_DISABLE_COMPILE_CACHE are both rejected for the unchanged-runtime baseline',
    },
  },
  timeline: {
    clock: { timeOriginEpochMs: performance.timeOrigin },
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
      processCpu:
        'whole process from before optional preload through worker termination and parent post-GC',
      mainCpu: 'Node.js main thread over the same interval as processCpu',
      workerCpuAfterAllReady:
        'sum of Worker.cpuUsage snapshots captured after the immediate all-ready process snapshot',
      workerCpuBeforeTermination:
        'sum of worker-local thread CPU snapshots after worker GC and before termination',
      residualCpu:
        'whole-process CPU minus measured Node.js main and pre-termination worker thread CPU; includes worker termination, Rust, Node runtime, native addon, helper threads, and measurement error',
    },
    processBeforePreload,
    processBeforeWorkers,
    processAtAllReady,
    processAfterWorkerResourceCapture,
    processBeforeTermination,
    processAfterTerminationBeforePostGc,
    processAfterPostGc,
    workerResourcesAfterAllReady,
    workerLocalSnapshots,
    processCpuMicros: processCpu,
    mainCpuMicros: mainCpu,
    workerCpuAfterAllReadyMicros: workerCpuAfterAllReady,
    workerCpuBeforeTerminationMicros: workerCpuBeforeTermination,
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

function subtractCpu(after, before) {
  return { user: after.user - before.user, system: after.system - before.system };
}

function waitForWorkerReady(worker, workerIndex, online, ready) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(
      () => finish(new Error(`worker ${workerIndex} did not become ready in time`)),
      INITIALIZATION_TIMEOUTS.workerReadyMs,
    );
    const onOnline = () => online.set(workerIndex, timestamp());
    const onMessage = (message) => {
      if (message.type === 'ready' && message.workerIndex === workerIndex) {
        ready.set(workerIndex, { receivedAt: timestamp(), ...message });
        finish();
      }
    };
    const onError = (error) => finish(error);
    const onExit = (code) => finish(new Error(`worker ${workerIndex} exited ${code} before ready`));
    const finish = (error) => {
      clearTimeout(timer);
      worker.off('online', onOnline);
      worker.off('message', onMessage);
      worker.off('error', onError);
      worker.off('exit', onExit);
      if (error) reject(error);
      else resolve();
    };
    worker.once('online', onOnline);
    worker.on('message', onMessage);
    worker.once('error', onError);
    worker.once('exit', onExit);
  });
}

function waitForWorkerSnapshot(worker, workerIndex) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(
      () => finish(new Error(`worker ${workerIndex} did not return its snapshot in time`)),
      INITIALIZATION_TIMEOUTS.workerSnapshotMs,
    );
    const onMessage = (message) => {
      if (message.type === 'snapshot' && message.workerIndex === workerIndex)
        finish(undefined, message);
    };
    const onError = (error) => finish(error);
    const onExit = (code) =>
      finish(new Error(`worker ${workerIndex} exited ${code} before its snapshot`));
    const finish = (error, value) => {
      clearTimeout(timer);
      worker.off('message', onMessage);
      worker.off('error', onError);
      worker.off('exit', onExit);
      if (error) reject(error);
      else resolve(value);
    };
    worker.on('message', onMessage);
    worker.once('error', onError);
    worker.once('exit', onExit);
    worker.postMessage('snapshot-post-gc');
  });
}

function withTimeout(promise, timeoutMs, label) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(
      () => reject(new Error(`${label} timed out after ${timeoutMs} ms`)),
      timeoutMs,
    );
    promise.then(
      (value) => {
        clearTimeout(timer);
        resolve(value);
      },
      (error) => {
        clearTimeout(timer);
        reject(error);
      },
    );
  });
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
