export const COUNTER = {
  factoryCalls: 0,
  componentCalls: 1,
  moduleCalls: 2,
  active: 3,
  maxActive: 4,
  componentNsTotal: 5,
  componentNsMax: 6,
  moduleNsTotal: 7,
  moduleNsMax: 8,
  inputCodeBytes: 9,
  returnedCodeBytes: 10,
  returnedMapBytes: 11,
  warnings: 12,
  errors: 13,
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
    componentCalls: read(COUNTER.componentCalls),
    moduleCalls: read(COUNTER.moduleCalls),
    handlerCalls: read(COUNTER.componentCalls) + read(COUNTER.moduleCalls),
    handlerActive: read(COUNTER.active),
    maxHandlerActive: read(COUNTER.maxActive),
    componentNsTotal: read(COUNTER.componentNsTotal),
    componentNsMax: read(COUNTER.componentNsMax),
    moduleNsTotal: read(COUNTER.moduleNsTotal),
    moduleNsMax: read(COUNTER.moduleNsMax),
    handlerInputCodeBytes: read(COUNTER.inputCodeBytes),
    handlerReturnedCodeBytes: read(COUNTER.returnedCodeBytes),
    handlerReturnedMapBytes: read(COUNTER.returnedMapBytes),
    warnings: read(COUNTER.warnings),
    errors: read(COUNTER.errors),
    workerMask: BigInt.asUintN(64, Atomics.load(counters, COUNTER.workerMask)).toString(16),
    perWorkerCalls,
  };
}
