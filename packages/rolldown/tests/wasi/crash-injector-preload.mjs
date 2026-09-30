// Main-thread preload for worker-crash-latch.mjs (`node --import`). Sends every
// WASI pool worker (the loader's `wasi-worker.mjs`) through
// crash-injector-worker.mjs, which makes one wasm import throw inside the
// worker. Nothing else in the process changes.
import { fileURLToPath } from 'node:url';
import workerThreads from 'node:worker_threads';

const injector = fileURLToPath(new URL('./crash-injector-worker.mjs', import.meta.url));
const OriginalWorker = workerThreads.Worker;

workerThreads.Worker = class extends OriginalWorker {
  constructor(filename, options = {}) {
    const file = filename instanceof URL ? fileURLToPath(filename) : String(filename);
    if (file.endsWith('wasi-worker.mjs')) {
      options = { ...options, workerData: { ...options.workerData, __crashInjectorTarget: file } };
      filename = injector;
    }
    super(filename, options);
  }
};
