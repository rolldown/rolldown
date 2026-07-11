import { realpathSync } from 'node:fs';
import nodePath from 'node:path';
import { compile, VERSION } from 'svelte/compiler';
import { COUNTER, MAX_WORKERS } from './metrics.js';

export const SVELTE_VERSION = VERSION;

const updateMax = (counters, index, candidate) => {
  let current = Atomics.load(counters, index);
  while (candidate > current) {
    const previous = Atomics.compareExchange(counters, index, current, candidate);
    if (previous === current) return;
    current = previous;
  }
};

const normalizedFilename = (corpusDirectory, id) =>
  nodePath.relative(corpusDirectory, id).split(nodePath.sep).join('/');

const normalizeWarning = (warning, id) => ({
  code: `SVELTE_${warning.code.toUpperCase()}`,
  message: warning.message,
  id,
  loc: warning.start
    ? {
        file: id,
        line: warning.start.line,
        column: warning.start.column,
      }
    : undefined,
  frame: warning.frame,
});

const compileComponent = (code, id, corpusDirectory, context) => {
  const result = compile(code, {
    filename: normalizedFilename(corpusDirectory, id),
    generate: 'client',
    dev: false,
    css: 'injected',
    discloseVersion: false,
  });
  for (const warning of result.warnings) context.warn(normalizeWarning(warning, id));
  const map = JSON.parse(result.js.map.toString());
  return {
    transformResult: { code: result.js.code, map },
    mapBytes: Buffer.byteLength(JSON.stringify(map)),
    warningCount: result.warnings.length,
  };
};

export const createSvelteTransformPlugin = (options, threadNumber = 0) => {
  if (!Number.isInteger(threadNumber) || threadNumber < 0 || threadNumber >= MAX_WORKERS) {
    throw new Error(`invalid Svelte transform thread number: ${threadNumber}`);
  }
  if (typeof options.corpusDirectory !== 'string' || options.corpusDirectory.length === 0) {
    throw new Error('corpusDirectory must be a non-empty string');
  }
  const corpusDirectory = realpathSync(options.corpusDirectory);
  const counters = options.metricsBuffer ? new BigInt64Array(options.metricsBuffer) : undefined;
  if (counters) {
    Atomics.add(counters, COUNTER.factoryCalls, 1n);
    Atomics.or(counters, COUNTER.workerMask, 1n << BigInt(threadNumber));
  }

  if (!counters) {
    return {
      name: 'svelte-transform',
      transform: {
        filter: { id: { include: [/\.svelte$/] } },
        handler(code, id) {
          return compileComponent(code, id, corpusDirectory, this).transformResult;
        },
      },
    };
  }

  return {
    name: 'svelte-transform',
    transform: {
      filter: { id: { include: [/\.svelte$/] } },
      handler(code, id) {
        const startedAt = process.hrtime.bigint();
        const active = Atomics.add(counters, COUNTER.active, 1n) + 1n;
        updateMax(counters, COUNTER.maxActive, active);
        Atomics.add(counters, COUNTER.calls, 1n);
        Atomics.add(counters, COUNTER.perWorkerCallsStart + threadNumber, 1n);
        Atomics.add(counters, COUNTER.inputCodeBytes, BigInt(Buffer.byteLength(code)));
        try {
          const result = compileComponent(code, id, corpusDirectory, this);
          Atomics.add(
            counters,
            COUNTER.returnedCodeBytes,
            BigInt(Buffer.byteLength(result.transformResult.code)),
          );
          Atomics.add(counters, COUNTER.returnedMapBytes, BigInt(result.mapBytes));
          Atomics.add(counters, COUNTER.warnings, BigInt(result.warningCount));
          return result.transformResult;
        } catch (error) {
          Atomics.add(counters, COUNTER.errors, 1n);
          throw error;
        } finally {
          const elapsed = process.hrtime.bigint() - startedAt;
          Atomics.add(counters, COUNTER.serviceNsTotal, elapsed);
          updateMax(counters, COUNTER.serviceNsMax, elapsed);
          Atomics.sub(counters, COUNTER.active, 1n);
        }
      },
    },
  };
};

export const svelteTransformPlugin = (options) => createSvelteTransformPlugin(options);
