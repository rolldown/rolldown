import { defineParallelPluginImplementation } from 'rolldown/parallelPlugin';

export default defineParallelPluginImplementation(({ state }) => ({
  name: 'parallel-close-lifecycle',
  async transform(code) {
    Atomics.add(state, 0, 1);
    Atomics.notify(state, 0);
    while (Atomics.load(state, 1) === 0) {
      await Atomics.waitAsync(state, 1, 0).value;
    }
    Atomics.add(state, 2, 1);
    Atomics.notify(state, 2);
    return { code };
  },
}));
