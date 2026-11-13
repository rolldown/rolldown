// Pool-worker entry for worker-crash-latch.mjs. Wraps the wasm import named by
// ROLLDOWN_TEST_CRASH_IMPORT (`<module>.<name>`) so that one of its calls throws
// a WebAssembly.RuntimeError: the worker's wasm thread dies mid-task and takes
// the same path as a real trap (wasi-threads error handler -> emnapi reports the
// crash on the main thread). Then it runs the real `wasi-worker.mjs`.
//
// Which call throws:
// - by default, the ROLLDOWN_TEST_CRASH_AT-th call in this worker;
// - with ROLLDOWN_TEST_CRASH_ARMED=1, the first call in any worker after the
//   main thread armed the shared control buffer (control[0]: 0 idle, 1 armed,
//   2 fired; only the worker that moves it from 1 to 2 throws). Until then every
//   `fd_read` adds one to control[1], so the main thread can see the builds
//   reading files before it arms.
import { appendFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import { threadId, workerData } from 'node:worker_threads';

const target = process.env.ROLLDOWN_TEST_CRASH_IMPORT;
const crashAt = Number(process.env.ROLLDOWN_TEST_CRASH_AT ?? 1);
const crashLog = process.env.ROLLDOWN_TEST_CRASH_LOG;
const control = workerData.__crashInjectorControl;
const PROGRESS_IMPORT = 'wasi_snapshot_preview1.fd_read';
let calls = 0;

function wrapImport(imports, spec, wrap) {
  const dot = spec.indexOf('.');
  const namespace = imports?.[spec.slice(0, dot)];
  const name = spec.slice(dot + 1);
  const original = namespace?.[name];
  if (typeof original === 'function') {
    namespace[name] = wrap(original);
  }
}

function crash() {
  if (crashLog) appendFileSync(crashLog, `t${threadId} CRASH in ${target} call ${calls}\n`);
  throw new WebAssembly.RuntimeError(`forced worker crash in ${target} (test)`);
}

const OriginalInstance = WebAssembly.Instance;
function Instance(module, imports) {
  if (control instanceof Int32Array) {
    wrapImport(
      imports,
      PROGRESS_IMPORT,
      (original) =>
        function (...args) {
          Atomics.add(control, 1, 1);
          return original.apply(this, args);
        },
    );
  }
  if (target) {
    wrapImport(
      imports,
      target,
      (original) =>
        function (...args) {
          ++calls;
          if (control instanceof Int32Array) {
            if (Atomics.load(control, 0) === 1 && Atomics.compareExchange(control, 0, 1, 2) === 1) {
              crash();
            }
          } else if (calls === crashAt) {
            crash();
          }
          return original.apply(this, args);
        },
    );
  }
  return new OriginalInstance(module, imports);
}
Instance.prototype = OriginalInstance.prototype;
WebAssembly.Instance = Instance;

await import(pathToFileURL(workerData.__crashInjectorTarget).href);
