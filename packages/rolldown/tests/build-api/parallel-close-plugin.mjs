import { defineParallelPluginImplementation } from 'rolldown/parallelPlugin';

export default defineParallelPluginImplementation(({ state }) => ({
  name: 'parallel-build-close-lifecycle',
  buildStart() {
    Atomics.add(state, 0, 1);
  },
  closeBundle() {
    Atomics.add(state, 1, 1);
  },
}));
