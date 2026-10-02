import { createRequire } from 'node:module';
import { readdirSync } from 'node:fs';
import { Worker } from 'node:worker_threads';
import { fileURLToPath } from 'node:url';

import { rolldown } from 'rolldown';
import { getRuntimeCapabilities } from 'rolldown/experimental';

const require = createRequire(import.meta.url);
const bindingDir = fileURLToPath(new URL('../../../dist/', import.meta.url));
const bindingFiles = readdirSync(bindingDir).filter(
  (name) => name.startsWith('rolldown-binding.') && name.endsWith('.node'),
);
if (bindingFiles.length !== 1) {
  throw new Error(`Expected one native Rolldown binding, found ${bindingFiles.join(', ')}`);
}
const binding = require(
  fileURLToPath(new URL(`../../../dist/${bindingFiles[0]}`, import.meta.url)),
);
const stopRuntime = binding.__rolldownTestStopAsyncRuntime;
const startRuntime = binding.__rolldownTestStartAsyncRuntime;
if (typeof stopRuntime !== 'function' || typeof startRuntime !== 'function') {
  throw new Error('The async-runtime binding was built without scheduler lifecycle test probes');
}

const capabilities = getRuntimeCapabilities();
let mainBundle;
let replacementBundle;
let worker;
let runtimeStopped = false;
let mainBundleGenerations = 0;
let replacementBundleGenerations = 0;
let retiredSchedulerState;
let workerExternalSideEffectsEntered = false;
let workerNormalLoadEntered = false;

try {
  mainBundle = await rolldown(createInputOptions('main'));
  await mainBundle.generate({ format: 'esm' });
  mainBundleGenerations += 1;

  worker = new Worker(new URL('./loader-cancellation-worker.mjs', import.meta.url));
  const workerState = await waitForWorkerLoads(worker);
  workerExternalSideEffectsEntered = workerState.externalSideEffectsEntered;
  workerNormalLoadEntered = workerState.normalLoadEntered;
  await withTimeout(worker.terminate(), 'worker termination');
  worker = undefined;

  await mainBundle.generate({ format: 'esm' });
  mainBundleGenerations += 1;
  await mainBundle.close();
  mainBundle = undefined;

  stopRuntime();
  runtimeStopped = true;
  retiredSchedulerState = binding.getAsyncRuntimeMetrics();
  for (const key of ['queuedRunnables', 'activeRunnables', 'activeBlockingTasks']) {
    if (retiredSchedulerState[key] !== 0) {
      throw new Error(
        `Scheduler ${key} did not retire after worker cancellation: ${JSON.stringify(
          retiredSchedulerState,
        )}`,
      );
    }
  }
  startRuntime();
  runtimeStopped = false;

  replacementBundle = await rolldown(createInputOptions('replacement'));
  await replacementBundle.generate({ format: 'esm' });
  replacementBundleGenerations += 1;
  await replacementBundle.close();
  replacementBundle = undefined;

  console.log(
    JSON.stringify({
      backend: capabilities.backend,
      flavor: capabilities.flavor,
      mainBundleGenerations,
      replacementBundleGenerations,
      retiredSchedulerState: {
        activeBlockingTasks: retiredSchedulerState.activeBlockingTasks,
        activeRunnables: retiredSchedulerState.activeRunnables,
        queuedRunnables: retiredSchedulerState.queuedRunnables,
      },
      workerExternalSideEffectsEntered,
      workerNormalLoadEntered,
    }),
  );
} finally {
  await worker?.terminate().catch(() => {});
  await mainBundle?.close().catch(() => {});
  await replacementBundle?.close().catch(() => {});
  if (runtimeStopped) {
    startRuntime();
  }
}

function createInputOptions(name) {
  const publicId = `virtual:${name}`;
  const resolvedId = `\0${publicId}`;
  return {
    input: publicId,
    plugins: [
      {
        name: `worker-termination-${name}`,
        resolveId(id) {
          if (id === publicId) {
            return resolvedId;
          }
        },
        load(id) {
          if (id === resolvedId) {
            return `export const value = ${JSON.stringify(name)};`;
          }
        },
      },
    ],
  };
}

function waitForWorkerLoads(target) {
  return withTimeout(
    new Promise((resolve, reject) => {
      const pending = new Set(['external-side-effects-entered', 'normal-load-entered']);
      const cleanup = () => {
        target.off('message', onMessage);
        target.off('error', onError);
        target.off('exit', onExit);
      };
      const onMessage = (message) => {
        if (!pending.delete(message)) {
          cleanup();
          reject(new Error(`Unexpected worker response: ${JSON.stringify(message)}`));
          return;
        }
        if (pending.size === 0) {
          cleanup();
          resolve({
            externalSideEffectsEntered: true,
            normalLoadEntered: true,
          });
        }
      };
      const onError = (error) => {
        cleanup();
        reject(error);
      };
      const onExit = (code) => {
        cleanup();
        reject(
          new Error(
            `Worker exited before entering every loader callback with code ${code}; pending=${[
              ...pending,
            ].join(',')}`,
          ),
        );
      };
      target.on('message', onMessage);
      target.once('error', onError);
      target.once('exit', onExit);
    }),
    'worker load hook',
  );
}

function withTimeout(promise, label) {
  let timer;
  const timeout = new Promise((_, reject) => {
    timer = setTimeout(() => reject(new Error(`Timed out waiting for ${label}`)), 10_000);
    timer.unref();
  });
  return Promise.race([promise, timeout]).finally(() => clearTimeout(timer));
}
