export const COUNTER = {
  factoryCalls: 0,
  calls: 1,
  active: 2,
  maxActive: 3,
  serviceNsTotal: 4,
  serviceNsMax: 5,
  inputBytes: 6,
  returnedBytes: 7,
  workerMask: 8,
  factoryNsTotal: 9,
  factoryNsMax: 10,
  syncFsCalls: 11,
  asyncDelayCalls: 12,
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
    handlerCalls: read(COUNTER.calls),
    handlerActive: read(COUNTER.active),
    maxHandlerActive: read(COUNTER.maxActive),
    handlerNsTotal: read(COUNTER.serviceNsTotal),
    handlerNsMax: read(COUNTER.serviceNsMax),
    handlerInputBytes: read(COUNTER.inputBytes),
    handlerReturnedBytes: read(COUNTER.returnedBytes),
    syncFsCalls: read(COUNTER.syncFsCalls),
    asyncDelayCalls: read(COUNTER.asyncDelayCalls),
    workerMask: BigInt.asUintN(64, Atomics.load(counters, COUNTER.workerMask)).toString(16),
    factoryNsTotal: read(COUNTER.factoryNsTotal),
    factoryNsMax: read(COUNTER.factoryNsMax),
    perWorkerCalls,
  };
}
