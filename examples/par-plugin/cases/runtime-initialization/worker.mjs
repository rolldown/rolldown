import { createRequire } from 'node:module';
import { parentPort, workerData } from 'node:worker_threads';
import { performance } from 'node:perf_hooks';
import { getHeapStatistics } from 'node:v8';

const state = new Int32Array(workerData.stateBuffer);
const timestamp = () => {
  const monotonicMs = performance.now();
  return { monotonicMs, epochMs: performance.timeOrigin + monotonicMs };
};
const entryAt = timestamp();
Atomics.store(state, workerData.workerIndex, 1);
Atomics.notify(state, workerData.workerIndex);

const importStartedAt = timestamp();
if (workerData.mode === 'binding') {
  createRequire(import.meta.url)(workerData.bindingPath);
} else if (workerData.mode === 'package') {
  await import(workerData.packageEntryUrl);
} else if (workerData.mode !== 'empty') {
  throw new Error(`unknown initialization probe mode: ${workerData.mode}`);
}
const importFinishedAt = timestamp();
Atomics.store(state, workerData.workerIndex, 2);
Atomics.notify(state, workerData.workerIndex);

parentPort.postMessage({
  type: 'ready',
  workerIndex: workerData.workerIndex,
  clock: { timeOriginEpochMs: performance.timeOrigin },
  timeline: { entryAt, importStartedAt, importFinishedAt },
  cpuUsageMicros: process.threadCpuUsage(),
  heapStatistics: getHeapStatistics(),
  eventLoopUtilization: performance.eventLoopUtilization(),
});

parentPort.on('message', (message) => {
  if (message === 'snapshot-post-gc') {
    globalThis.gc?.();
    globalThis.gc?.();
    parentPort.postMessage({
      type: 'snapshot',
      workerIndex: workerData.workerIndex,
      capturedAt: timestamp(),
      postGc: typeof globalThis.gc === 'function',
      cpuUsageMicros: process.threadCpuUsage(),
      heapStatistics: getHeapStatistics(),
      eventLoopUtilization: performance.eventLoopUtilization(),
    });
  }
});
