import { defineParallelPluginImplementation } from 'rolldown/parallelPlugin';

export default defineParallelPluginImplementation(({ state, failRender }) => ({
  name: 'parallel-build-close-lifecycle',
  buildStart() {
    Atomics.add(state, 0, 1);
  },
  renderStart() {
    if (failRender) {
      throw new Error('parallel render failure');
    }
  },
  closeBundle() {
    Atomics.add(state, 1, 1);
  },
}));
