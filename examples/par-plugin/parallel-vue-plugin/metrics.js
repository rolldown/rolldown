export const COUNTER = {
  factoryCalls: 0,
  factoryNsTotal: 1,
  factoryNsMax: 2,
  buildStartCalls: 3,
  buildStartNsTotal: 4,
  buildStartNsMax: 5,
  calls: 6,
  active: 7,
  maxActive: 8,
  serviceNsTotal: 9,
  serviceNsMax: 10,
  inputCodeBytes: 11,
  returnedCodeBytes: 12,
  returnedMapBytes: 13,
  workerMask: 14,
  perWorkerCallsStart: 16,
};

export const MAX_WORKERS = 64;
export const COUNTER_LENGTH = COUNTER.perWorkerCallsStart + MAX_WORKERS;

export const TIMELINE_FIELD = {
  calls: 0,
  workerNumber: 1,
  kernelStartedAtNs: 2,
  kernelFinishedAtNs: 3,
};
export const TIMELINE_STRIDE = 4;

export function createMetricsBuffer() {
  return new SharedArrayBuffer(BigInt64Array.BYTES_PER_ELEMENT * COUNTER_LENGTH);
}

export function createTransformTimelineBuffer(sourceCount) {
  if (!Number.isSafeInteger(sourceCount) || sourceCount < 1) {
    throw new Error('sourceCount must be a positive safe integer');
  }
  return new SharedArrayBuffer(BigInt64Array.BYTES_PER_ELEMENT * TIMELINE_STRIDE * sourceCount);
}

export function readTransformTimeline(buffer, sourceKeys) {
  if (!buffer) return undefined;
  if (!Array.isArray(sourceKeys)) throw new Error('sourceKeys must be an array');
  const values = new BigInt64Array(buffer);
  if (values.length !== sourceKeys.length * TIMELINE_STRIDE) {
    throw new Error('Vue transform timeline buffer length mismatch');
  }
  return {
    clock: {
      source: 'process.hrtime.bigint()',
      unit: 'nanoseconds',
      epoch: 'arbitrary monotonic epoch shared by Node.js worker threads in this process',
      alignment:
        'run-case clockAnchors bracket the same hrtime clock with Date.now() before plugin setup and after build',
    },
    records: sourceKeys.map((sourceKey, ordinal) => {
      const offset = ordinal * TIMELINE_STRIDE;
      const calls = Number(Atomics.load(values, offset + TIMELINE_FIELD.calls));
      const encodedWorkerNumber = Number(
        Atomics.load(values, offset + TIMELINE_FIELD.workerNumber),
      );
      const kernelStartedAtNs = Atomics.load(values, offset + TIMELINE_FIELD.kernelStartedAtNs);
      const kernelFinishedAtNs = Atomics.load(values, offset + TIMELINE_FIELD.kernelFinishedAtNs);
      return {
        ordinal,
        sourceKey,
        calls,
        workerNumber: encodedWorkerNumber - 1,
        kernelStartedAtNs: kernelStartedAtNs.toString(),
        kernelFinishedAtNs: kernelFinishedAtNs.toString(),
        kernelDurationNs: (kernelFinishedAtNs - kernelStartedAtNs).toString(),
      };
    }),
  };
}

export function readMetrics(buffer) {
  if (!buffer) return undefined;
  const counters = new BigInt64Array(buffer);
  const read = (index) => Number(Atomics.load(counters, index));
  const perWorkerCalls = Array.from({ length: MAX_WORKERS }, (_, index) =>
    read(COUNTER.perWorkerCallsStart + index),
  );
  while (perWorkerCalls.at(-1) === 0) perWorkerCalls.pop();
  return {
    factoryCalls: read(COUNTER.factoryCalls),
    factoryNsTotal: read(COUNTER.factoryNsTotal),
    factoryNsMax: read(COUNTER.factoryNsMax),
    buildStartCalls: read(COUNTER.buildStartCalls),
    buildStartNsTotal: read(COUNTER.buildStartNsTotal),
    buildStartNsMax: read(COUNTER.buildStartNsMax),
    handlerCalls: read(COUNTER.calls),
    handlerActive: read(COUNTER.active),
    maxHandlerActive: read(COUNTER.maxActive),
    handlerNsTotal: read(COUNTER.serviceNsTotal),
    handlerNsMax: read(COUNTER.serviceNsMax),
    handlerInputCodeBytes: read(COUNTER.inputCodeBytes),
    handlerReturnedCodeBytes: read(COUNTER.returnedCodeBytes),
    handlerReturnedMapBytes: read(COUNTER.returnedMapBytes),
    workerMask: BigInt.asUintN(64, Atomics.load(counters, COUNTER.workerMask)).toString(16),
    perWorkerCalls,
  };
}
