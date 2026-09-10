import { defineParallelPluginImplementation } from 'rolldown/parallelPlugin';
import { threadId } from 'node:worker_threads';

export default defineParallelPluginImplementation(({ mode, state }, { threadNumber }) => {
  Atomics.add(state, 0, 1);
  if (threadNumber === 0) {
    Atomics.store(state, 3, threadId);
    const interval = setInterval(() => {
      if (Atomics.load(state, 2) === 0) return;
      clearInterval(interval);
      Atomics.add(state, 1, 1);
      Atomics.notify(state, 1);
      if (mode === 'exit') {
        process.exit(23);
      }
      throw new Error('delayed parallel-plugin worker fault');
    }, 1);
  }

  return {
    name: 'parallel-worker-supervision-fixture',
    buildStart() {},
  };
});
