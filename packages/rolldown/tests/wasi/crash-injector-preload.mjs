// Main-thread preload for worker-crash-latch.mjs (`node --import`). Sends every
// WASI pool worker (the loader's `wasi-worker.mjs`) through
// crash-injector-worker.mjs, which makes one wasm import throw inside the
// worker. Nothing else in the process changes.
//
// With ROLLDOWN_TEST_CRASH_ARMED=1 the workers also share a control buffer with
// this thread, published as globalThis[Symbol.for('rolldown.test.crashControl')]:
// the test arms the crash from here (see crash-injector-worker.mjs). The loader's
// own crash flag (`workerData.crashFlag`, which a pool worker raises as its wasm
// thread dies) is published as globalThis[Symbol.for('rolldown.test.loaderCrashFlag')].
import { fileURLToPath } from 'node:url';
import workerThreads from 'node:worker_threads';

const injector = fileURLToPath(new URL('./crash-injector-worker.mjs', import.meta.url));
const OriginalWorker = workerThreads.Worker;
const control =
  process.env.ROLLDOWN_TEST_CRASH_ARMED === '1'
    ? new Int32Array(new SharedArrayBuffer(8))
    : undefined;
if (control) {
  globalThis[Symbol.for('rolldown.test.crashControl')] = control;
}

workerThreads.Worker = class extends OriginalWorker {
  constructor(filename, options = {}) {
    const file = filename instanceof URL ? fileURLToPath(filename) : String(filename);
    if (file.endsWith('wasi-worker.mjs')) {
      if (control && options.workerData?.crashFlag instanceof Int32Array) {
        globalThis[Symbol.for('rolldown.test.loaderCrashFlag')] ??= options.workerData.crashFlag;
      }
      options = {
        ...options,
        workerData: {
          ...options.workerData,
          __crashInjectorTarget: file,
          __crashInjectorControl: control,
        },
      };
      filename = injector;
    }
    super(filename, options);
  }
};
