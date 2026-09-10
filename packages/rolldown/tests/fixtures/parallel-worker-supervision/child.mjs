import { Worker } from 'node:worker_threads';
import nodePath from 'node:path';
import { rolldown } from 'rolldown';
import { defineParallelPlugin } from 'rolldown/experimental';

const mode = process.argv[2];
if (mode !== 'error' && mode !== 'exit') {
  throw new Error(`Unexpected worker fault mode: ${mode}`);
}
const keepAlive = setInterval(() => {}, 1_000);
const state = new Int32Array(new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT * 4));

let targetWorkerExitCount = 0;
let terminateCalls = 0;
const workerThreadIds = new WeakMap();
const originalEmit = Worker.prototype.emit;
const originalTerminate = Worker.prototype.terminate;
Worker.prototype.emit = function (event, ...args) {
  if (event === 'online') workerThreadIds.set(this, this.threadId);
  const result = Reflect.apply(originalEmit, this, [event, ...args]);
  if (event === 'exit' && workerThreadIds.get(this) === Atomics.load(state, 3)) {
    targetWorkerExitCount += 1;
  }
  return result;
};
Worker.prototype.terminate = function () {
  terminateCalls += 1;
  return Reflect.apply(originalTerminate, this, []);
};

await main().finally(() => {
  clearInterval(keepAlive);
  Worker.prototype.emit = originalEmit;
  Worker.prototype.terminate = originalTerminate;
});

async function main() {
  const plugin = defineParallelPlugin(nodePath.join(import.meta.dirname, 'fault-plugin.mjs'));
  const bundle = await rolldown({
    cwd: import.meta.dirname,
    input: 'input.js',
    plugins: [plugin({ mode, state })],
  });

  await bundle.generate();
  Atomics.store(state, 2, 1);
  Atomics.notify(state, 2);
  await waitFor(() => Atomics.load(state, 1) === 1 && targetWorkerExitCount === 1);

  const firstCloseError = await bundle.close().catch((error) => error);
  if (!(firstCloseError instanceof Error)) {
    throw new Error('The first close did not report the delayed worker fault');
  }
  const firstCloseErrors = collectErrorMessages(firstCloseError);
  const terminateCallsAfterFirstClose = terminateCalls;
  await bundle.close();
  const terminateCallsAfterRetry = terminateCalls;

  console.log(
    JSON.stringify({
      firstCloseErrors,
      terminateCallsAfterFirstClose,
      terminateCallsAfterRetry,
      workerCount: Atomics.load(state, 0),
    }),
  );
}

function collectErrorMessages(error) {
  const messages = [error.message];
  if (error instanceof AggregateError) {
    for (const nested of error.errors) {
      messages.push(...(nested instanceof Error ? collectErrorMessages(nested) : [String(nested)]));
    }
  }
  return messages;
}

async function waitFor(predicate) {
  const deadline = Date.now() + 10_000;
  while (!predicate()) {
    if (Date.now() >= deadline) {
      throw new Error(
        `Timed out waiting for worker faults: ${JSON.stringify({
          faultCount: Atomics.load(state, 1),
          workerCount: Atomics.load(state, 0),
          targetWorkerExitCount,
          targetWorkerThreadId: Atomics.load(state, 3),
        })}`,
      );
    }
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
}
