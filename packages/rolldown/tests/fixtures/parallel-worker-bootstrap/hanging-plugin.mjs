import { defineParallelPluginImplementation } from 'rolldown/parallelPlugin';

export default defineParallelPluginImplementation(async ({ siblingEntered }, { threadNumber }) => {
  const state = new Int32Array(siblingEntered);
  if (threadNumber === 0) {
    Atomics.wait(state, 0, 0, 5_000);
    if (Atomics.load(state, 0) !== 1) {
      throw new Error('hanging sibling did not enter its initializer');
    }
    throw new Error('sentinel parallel bootstrap failure');
  }
  Atomics.store(state, 0, 1);
  Atomics.notify(state, 0);
  await new Promise(() => {});
});
