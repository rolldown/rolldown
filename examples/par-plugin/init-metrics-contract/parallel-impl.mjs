import { defineParallelPluginImplementation } from '../../../packages/rolldown/dist/parallel-plugin.mjs';

export default defineParallelPluginImplementation((options) => {
  const counters = new Int32Array(options.metricsBuffer);
  Atomics.add(counters, 0, 1);
  return {
    name: 'initialization-contract',
    buildStart() {
      Atomics.add(counters, 1, 1);
    },
    transform: {
      filter: { id: /input\.js$/ },
      handler(code) {
        Atomics.add(counters, 2, 1);
        return `${code}\n/* initialization-contract */`;
      },
    },
    buildEnd() {
      Atomics.add(counters, 3, 1);
    },
    closeBundle() {
      Atomics.add(counters, 4, 1);
    },
  };
});
