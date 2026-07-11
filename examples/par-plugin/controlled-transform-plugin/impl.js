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
  return `/*${'x'.repeat(byteLength - 4)}*/`;
};

export const createControlledTransformPlugin = (options, threadNumber = 0) => {
  const factoryStartedAt = process.hrtime.bigint();
  const counters = options.metricsBuffer ? new BigInt64Array(options.metricsBuffer) : undefined;
  const workIterations = options.workIterations;
  const resultPadding = exactPadding(options.resultPaddingBytes);

  if (!Number.isInteger(threadNumber) || threadNumber < 0 || threadNumber >= MAX_WORKERS) {
    throw new Error(`invalid controlled-transform thread number: ${threadNumber}`);
  }

  if (counters) {
    Atomics.add(counters, COUNTER.factoryCalls, 1n);
    Atomics.or(counters, COUNTER.workerMask, 1n << BigInt(threadNumber));
    const elapsed = process.hrtime.bigint() - factoryStartedAt;
    Atomics.add(counters, COUNTER.factoryNsTotal, elapsed);
    updateMax(counters, COUNTER.factoryNsMax, elapsed);
  }

  return {
    name: 'controlled-transform',
    transform: {
      filter: { id: { include: [/\.controlled\.js$/] } },
      handler(code) {
        const startedAt = counters ? process.hrtime.bigint() : 0n;
        if (counters) {
          const active = Atomics.add(counters, COUNTER.active, 1n) + 1n;
          updateMax(counters, COUNTER.maxActive, active);
          Atomics.add(counters, COUNTER.calls, 1n);
          Atomics.add(counters, COUNTER.perWorkerCallsStart + threadNumber, 1n);
          Atomics.add(counters, COUNTER.inputCodeBytes, BigInt(Buffer.byteLength(code)));
        }

        try {
          let checksum = 2166136261;
          for (let index = 0; index < workIterations; index++) {
            checksum = Math.imul(checksum ^ code.charCodeAt(index % code.length), 16777619);
          }
          const checksumHex = (checksum >>> 0).toString(16).padStart(8, '0');
          const checksumSuffix = workIterations === 0 ? '' : `/*work:${checksumHex}*/`;
          const result = `${code}${resultPadding}${checksumSuffix}`;
          if (counters) {
            Atomics.add(counters, COUNTER.returnedCodeBytes, BigInt(Buffer.byteLength(result)));
          }
          return { code: result, map: null };
        } finally {
          if (counters) {
            const elapsed = process.hrtime.bigint() - startedAt;
            Atomics.add(counters, COUNTER.serviceNsTotal, elapsed);
            updateMax(counters, COUNTER.serviceNsMax, elapsed);
            Atomics.sub(counters, COUNTER.active, 1n);
          }
        }
      },
    },
  };
};

export const controlledTransformPlugin = (options) => createControlledTransformPlugin(options);

export default defineParallelPluginImplementation((options, context) =>
  createControlledTransformPlugin(options, context.threadNumber),
);
