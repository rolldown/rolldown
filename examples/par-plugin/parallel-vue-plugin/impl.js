import { defineParallelPluginImplementation } from 'rolldown/parallelPlugin';
import { createRequire } from 'node:module';
import Vue from 'unplugin-vue/rolldown';
import { COUNTER, MAX_WORKERS, TIMELINE_FIELD, TIMELINE_STRIDE } from './metrics.js';

// Resolve through Vue's Node entrypoint so the adapter pins its own compiler while
// preserving Vue's TypeScript filesystem registration side effect.
const compiler = createRequire(import.meta.url)('vue/compiler-sfc');

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
  const timeline = options.timelineBuffer ? new BigInt64Array(options.timelineBuffer) : undefined;
  if (!Number.isInteger(threadNumber) || threadNumber < 0 || threadNumber >= MAX_WORKERS) {
    throw new Error(`invalid parallel-vue thread number: ${threadNumber}`);
  }
  if (Boolean(timeline) !== Boolean(options.sourceOrdinals) || (timeline && !counters)) {
    throw new Error('Vue transform timeline requires metrics and sourceOrdinals together');
  }

  const vue = Vue({
    root: options.root,
    compiler,
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
        const ordinal = timeline ? options.sourceOrdinals[id] : undefined;
        if (timeline && !Number.isSafeInteger(ordinal)) {
          throw new Error(`missing Vue transform timeline ordinal for ${id}`);
        }
        const timelineOffset = timeline ? ordinal * TIMELINE_STRIDE : undefined;
        if (timeline) {
          const calls = Atomics.add(timeline, timelineOffset + TIMELINE_FIELD.calls, 1n) + 1n;
          if (calls !== 1n) throw new Error(`duplicate Vue transform timeline entry for ${id}`);
          Atomics.store(
            timeline,
            timelineOffset + TIMELINE_FIELD.workerNumber,
            BigInt(threadNumber + 1),
          );
          Atomics.store(timeline, timelineOffset + TIMELINE_FIELD.kernelStartedAtNs, startedAt);
        }
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
          const finishedAt = process.hrtime.bigint();
          if (timeline) {
            Atomics.store(timeline, timelineOffset + TIMELINE_FIELD.kernelFinishedAtNs, finishedAt);
          }
          const elapsed = finishedAt - startedAt;
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
