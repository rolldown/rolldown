import { existsSync } from 'node:fs';
import { setTimeout as delay } from 'node:timers/promises';
import { defineParallelPluginImplementation } from 'rolldown/parallelPlugin';
import { COUNTER, MAX_WORKERS } from './metrics.js';

const updateMax = (counters, index, candidate) => {
  let current = Atomics.load(counters, index);
  while (candidate > current) {
    const previous = Atomics.compareExchange(counters, index, current, candidate);
    if (previous === current) return;
    current = previous;
  }
};

const exactPadding = (byteLength) => {
  if (byteLength <= 0) return '';
  if (byteLength < 4) return ' '.repeat(byteLength);
  return `/*${'p'.repeat(byteLength - 4)}*/`;
};

const checksumWork = (input, iterations, syncFsProbes, probePath, counters) => {
  let checksum = 2166136261;
  for (let index = 0; index < iterations; index++) {
    checksum = Math.imul(checksum ^ input.charCodeAt(index % input.length), 16777619);
  }
  for (let index = 0; index < syncFsProbes; index++) {
    checksum = Math.imul(checksum ^ Number(existsSync(probePath)), 16777619);
  }
  if (counters && syncFsProbes > 0) {
    Atomics.add(counters, COUNTER.syncFsCalls, BigInt(syncFsProbes));
  }
  return (checksum >>> 0).toString(16).padStart(8, '0');
};

export const createControlledHookPlugin = (options, threadNumber = 0) => {
  const factoryStartedAt = process.hrtime.bigint();
  const counters = options.metricsBuffer ? new BigInt64Array(options.metricsBuffer) : undefined;
  const {
    hook,
    graphShape,
    moduleCount,
    workIterations,
    syncFsProbes,
    asyncDelayMs,
    probePath,
    resultPaddingBytes,
  } = options;
  const resultPadding = exactPadding(resultPaddingBytes);

  if (!Number.isInteger(threadNumber) || threadNumber < 0 || threadNumber >= MAX_WORKERS) {
    throw new Error(`invalid controlled-hooks thread number: ${threadNumber}`);
  }
  if (hook !== 'resolveId' && hook !== 'load') {
    throw new Error(`invalid controlled hook: ${hook}`);
  }

  if (counters) {
    Atomics.add(counters, COUNTER.factoryCalls, 1n);
    Atomics.or(counters, COUNTER.workerMask, 1n << BigInt(threadNumber));
    const elapsed = process.hrtime.bigint() - factoryStartedAt;
    Atomics.add(counters, COUNTER.factoryNsTotal, elapsed);
    updateMax(counters, COUNTER.factoryNsMax, elapsed);
  }

  const enterHandler = (inputBytes) => {
    const startedAt = counters ? process.hrtime.bigint() : 0n;
    if (counters) {
      const active = Atomics.add(counters, COUNTER.active, 1n) + 1n;
      updateMax(counters, COUNTER.maxActive, active);
      Atomics.add(counters, COUNTER.calls, 1n);
      Atomics.add(counters, COUNTER.perWorkerCallsStart + threadNumber, 1n);
      Atomics.add(counters, COUNTER.inputBytes, BigInt(inputBytes));
    }
    return startedAt;
  };

  const leaveHandler = (startedAt, returnedBytes) => {
    if (!counters) return;
    Atomics.add(counters, COUNTER.returnedBytes, BigInt(returnedBytes));
    const elapsed = process.hrtime.bigint() - startedAt;
    Atomics.add(counters, COUNTER.serviceNsTotal, elapsed);
    updateMax(counters, COUNTER.serviceNsMax, elapsed);
    Atomics.sub(counters, COUNTER.active, 1n);
  };

  if (hook === 'resolveId') {
    return {
      name: 'controlled-resolve-id',
      resolveId: {
        filter: { id: { include: [/^controlled-resolve:/] } },
        handler(specifier, importer) {
          const startedAt = enterHandler(
            Buffer.byteLength(specifier) + (importer ? Buffer.byteLength(importer) : 0),
          );
          let returnedBytes = 0;
          try {
            const index = Number(specifier.slice('controlled-resolve:'.length));
            if (!Number.isSafeInteger(index) || index < 0 || index >= moduleCount) {
              throw new Error(`invalid controlled resolve specifier: ${specifier}`);
            }
            const checksum = checksumWork(
              specifier,
              workIterations,
              syncFsProbes,
              probePath,
              counters,
            );
            const id = `\0controlled-resolved:${graphShape}:${moduleCount}:${index}:${checksum}`;
            returnedBytes = Buffer.byteLength(id);
            return id;
          } finally {
            leaveHandler(startedAt, returnedBytes);
          }
        },
      },
    };
  }

  return {
    name: 'controlled-load',
    load: {
      filter: {
        id: { include: [new RegExp(`^${String.fromCharCode(0)}controlled-load:`)] },
      },
      async handler(id) {
        const startedAt = enterHandler(Buffer.byteLength(id));
        let returnedBytes = 0;
        try {
          if (asyncDelayMs > 0) {
            if (counters) Atomics.add(counters, COUNTER.asyncDelayCalls, 1n);
            await delay(asyncDelayMs);
          }
          const index = Number(id.slice('\0controlled-load:'.length));
          if (!Number.isSafeInteger(index) || index < 0 || index >= moduleCount) {
            throw new Error(`invalid controlled load id: ${id}`);
          }
          const checksum = checksumWork(id, workIterations, syncFsProbes, probePath, counters);
          const nextImport =
            graphShape === 'chain' && index + 1 < moduleCount
              ? `import 'controlled-load:${index + 1}';\n`
              : '';
          const code = `${nextImport}globalThis.__controlledLoad = (globalThis.__controlledLoad || 0) + ${index} + 0x${checksum};\n${resultPadding}`;
          returnedBytes = Buffer.byteLength(code);
          return { code, map: null };
        } finally {
          leaveHandler(startedAt, returnedBytes);
        }
      },
    },
  };
};

export const controlledHookPlugin = (options) => createControlledHookPlugin(options);

export default defineParallelPluginImplementation((options, context) =>
  createControlledHookPlugin(options, context.threadNumber),
);
