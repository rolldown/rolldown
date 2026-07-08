import { createRequire } from 'node:module';
import { parentPort, workerData } from 'node:worker_threads';

const require = createRequire(import.meta.url);
const binding = require(workerData.bindingPath);
const capabilities = binding.getRuntimeCapabilities();
const probe = binding.__rolldownTestRetainSchedulerWaker;
const registerTaskHost = binding.registerCurrentThreadTaskHost;

if (typeof probe !== 'function') {
  parentPort.postMessage({
    type: 'unsupported',
    error: 'The async-runtime binding was built without the worker teardown regression probe',
  });
} else if (capabilities.flavor === 'CurrentThread' && typeof registerTaskHost !== 'function') {
  parentPort.postMessage({
    type: 'unsupported',
    error: 'The async-runtime binding does not expose the CurrentThread task-host contract',
  });
} else {
  if (capabilities.flavor === 'CurrentThread') {
    registerTaskHost();
  }
  probe(workerData.paths.armed, workerData.paths.release, workerData.paths.completed);
  parentPort.postMessage({ type: 'started', backend: capabilities.backend });
  setInterval(() => {}, 1_000);
}
