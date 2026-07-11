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

export function createMetricsBuffer() {
  return new SharedArrayBuffer(BigInt64Array.BYTES_PER_ELEMENT * COUNTER_LENGTH);
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
