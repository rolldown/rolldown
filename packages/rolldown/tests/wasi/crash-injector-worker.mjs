// Pool-worker entry for worker-crash-latch.mjs. Wraps the wasm import named by
// ROLLDOWN_TEST_CRASH_IMPORT (`<module>.<name>`) so its ROLLDOWN_TEST_CRASH_AT-th
// call in this worker throws a WebAssembly.RuntimeError: the worker's wasm
// thread dies mid-task and takes the same path as a real trap (wasi-threads
// error handler -> emnapi reports the crash on the main thread). Then it runs
// the real `wasi-worker.mjs`.
import { appendFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import { threadId, workerData } from 'node:worker_threads';

const target = process.env.ROLLDOWN_TEST_CRASH_IMPORT;
const crashAt = Number(process.env.ROLLDOWN_TEST_CRASH_AT ?? 1);
const crashLog = process.env.ROLLDOWN_TEST_CRASH_LOG;
let calls = 0;

const OriginalInstance = WebAssembly.Instance;
function Instance(module, imports) {
  if (target) {
    const dot = target.indexOf('.');
    const namespace = imports?.[target.slice(0, dot)];
    const name = target.slice(dot + 1);
    const original = namespace?.[name];
    if (typeof original === 'function') {
      namespace[name] = function (...args) {
        if (++calls === crashAt) {
          if (crashLog) appendFileSync(crashLog, `t${threadId} CRASH in ${target} call ${calls}\n`);
          throw new WebAssembly.RuntimeError(`forced worker crash in ${target} (test)`);
        }
        return original.apply(this, args);
      };
    }
  }
  return new OriginalInstance(module, imports);
}
Instance.prototype = OriginalInstance.prototype;
WebAssembly.Instance = Instance;

await import(pathToFileURL(workerData.__crashInjectorTarget).href);
