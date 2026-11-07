// Pool-worker entry for pool-worker-preload.mjs (load-failure case). The wasm
// instantiate in this worker throws, so a Worker the loader preloaded fails while
// it loads, the way a real load failure does; then the real `wasi-worker.mjs`
// runs unchanged.
import { pathToFileURL } from 'node:url';
import { workerData } from 'node:worker_threads';

WebAssembly.Instance = function () {
  throw new Error('forced pool worker load failure (test)');
};

await import(pathToFileURL(workerData.__poolPreloadTarget).href);
