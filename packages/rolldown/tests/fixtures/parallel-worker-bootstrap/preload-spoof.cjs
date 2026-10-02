// `--require` is the one inherited preload hook a classic eval worker still
// runs, so it observes `workerData` before the bootstrap hides its control
// port. The marker file records that a parallel-plugin worker replayed it.
const { appendFileSync } = require('node:fs');
const { isMainThread, workerData } = require('node:worker_threads');

if (!isMainThread && workerData?.threadNumber === 1) {
  const markerPath = process.env.ROLLDOWN_PRELOAD_BOUNDARY_MARKER;
  if (!markerPath) {
    throw new Error('Missing preload boundary marker path');
  }
  appendFileSync(markerPath, 'preload-completed\n');

  workerData.controlPort?.postMessage({ type: 'ready' });
  workerData.controlPort?.postMessage({ type: 'success' });
}
