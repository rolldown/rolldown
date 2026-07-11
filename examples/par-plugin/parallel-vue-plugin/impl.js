import { defineParallelPluginImplementation } from 'rolldown/parallelPlugin';
import Vue from 'unplugin-vue/rolldown';
import { COUNTER, MAX_WORKERS } from './metrics.js';

const updateMax = (counters, index, candidate) => {
  let current = Atomics.load(counters, index);
  while (candidate > current) {
    const previous = Atomics.compareExchange(counters, index, current, candidate);
    if (previous === current) return;
    current = previous;
  }
};

const hookHandler = (hook) => (typeof hook === 'function' ? hook : hook?.handler);

export const createVueTransformPlugin = (options, threadNumber = 0) => {
  const factoryStartedAt = options.metricsBuffer ? process.hrtime.bigint() : 0n;
  const counters = options.metricsBuffer ? new BigInt64Array(options.metricsBuffer) : undefined;
  if (!Number.isInteger(threadNumber) || threadNumber < 0 || threadNumber >= MAX_WORKERS) {
    throw new Error(`invalid parallel-vue thread number: ${threadNumber}`);
  }

  const vue = Vue({
    root: options.root,
    isProduction: true,
    sourceMap: false,
    inlineTemplate: true,
  });
  const originalBuildStart = hookHandler(vue.buildStart);
  const originalTransform = hookHandler(vue.transform);
  if (!originalBuildStart || !originalTransform) {
    throw new Error('unplugin-vue did not expose the expected buildStart and transform hooks');
  }

  if (counters) {
    Atomics.add(counters, COUNTER.factoryCalls, 1n);
    Atomics.or(counters, COUNTER.workerMask, 1n << BigInt(threadNumber));
    const elapsed = process.hrtime.bigint() - factoryStartedAt;
    Atomics.add(counters, COUNTER.factoryNsTotal, elapsed);
    updateMax(counters, COUNTER.factoryNsMax, elapsed);
  }

  const buildStart = counters
    ? async function (...args) {
        const startedAt = process.hrtime.bigint();
        Atomics.add(counters, COUNTER.buildStartCalls, 1n);
        try {
          return await originalBuildStart.call(this, ...args);
        } finally {
          const elapsed = process.hrtime.bigint() - startedAt;
          Atomics.add(counters, COUNTER.buildStartNsTotal, elapsed);
          updateMax(counters, COUNTER.buildStartNsMax, elapsed);
        }
      }
    : function (...args) {
        return originalBuildStart.call(this, ...args);
      };

  const transform = counters
    ? async function (code, id, ...args) {
        const startedAt = process.hrtime.bigint();
        const active = Atomics.add(counters, COUNTER.active, 1n) + 1n;
        updateMax(counters, COUNTER.maxActive, active);
        Atomics.add(counters, COUNTER.calls, 1n);
        Atomics.add(counters, COUNTER.perWorkerCallsStart + threadNumber, 1n);
        Atomics.add(counters, COUNTER.inputCodeBytes, BigInt(Buffer.byteLength(code)));
        try {
          const result = await originalTransform.call(this, code, id, ...args);
          const returnedCode = typeof result === 'string' ? result : result?.code;
          const returnedMap = typeof result === 'object' ? result?.map : undefined;
          if (typeof returnedCode === 'string') {
            Atomics.add(
              counters,
              COUNTER.returnedCodeBytes,
              BigInt(Buffer.byteLength(returnedCode)),
            );
          }
          if (returnedMap) {
            Atomics.add(
              counters,
              COUNTER.returnedMapBytes,
              BigInt(Buffer.byteLength(JSON.stringify(returnedMap))),
            );
          }
          return result;
        } finally {
          const elapsed = process.hrtime.bigint() - startedAt;
          Atomics.add(counters, COUNTER.serviceNsTotal, elapsed);
          updateMax(counters, COUNTER.serviceNsMax, elapsed);
          Atomics.sub(counters, COUNTER.active, 1n);
        }
      }
    : function (code, id, ...args) {
        return originalTransform.call(this, code, id, ...args);
      };

  return {
    name: 'parallel-vue-transform',
    buildStart,
    transform: {
      filter: { id: { include: [/\.vue$/] } },
      handler: transform,
    },
  };
};

export const vueTransformPlugin = (options) => createVueTransformPlugin(options);

export default defineParallelPluginImplementation((options, context) =>
  createVueTransformPlugin(options, context.threadNumber),
);
