import { existsSync } from 'node:fs';
import { createRequire } from 'node:module';
import { parentPort, workerData } from 'node:worker_threads';

import { getRuntimeCapabilities } from 'rolldown/experimental';

const require = createRequire(import.meta.url);
const binding = require(workerData.bindingPath);
const capabilities = getRuntimeCapabilities();
const probe = binding.__rolldownTestRetainSchedulerWaker;

if (typeof probe !== 'function') {
  parentPort.postMessage({
    type: 'unsupported',
    error: 'The async-runtime binding was built without the worker teardown regression probe',
  });
} else {
  const armingKeepalive = setInterval(() => {
    if (existsSync(workerData.paths.armed)) {
      clearInterval(armingKeepalive);
    }
  }, 5);
  probe(workerData.paths.armed, workerData.paths.release, workerData.paths.completed);
  parentPort.postMessage({
    type: 'started',
    backend: capabilities.backend,
    flavor: capabilities.flavor,
  });
}
