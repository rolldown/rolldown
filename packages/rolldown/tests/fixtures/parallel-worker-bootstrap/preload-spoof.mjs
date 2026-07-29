import { appendFileSync } from 'node:fs';
import { isMainThread, workerData } from 'node:worker_threads';

if (!isMainThread && workerData?.threadNumber === 1) {
  workerData.controlPort.postMessage({ type: 'ready' });
  workerData.controlPort.postMessage({ type: 'success' });

  const markerPath = process.env.ROLLDOWN_PRELOAD_BOUNDARY_MARKER;
  if (!markerPath) {
    throw new Error('Missing preload boundary marker path');
  }
  appendFileSync(markerPath, 'preload-completed\n');
}
